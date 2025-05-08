#![doc = "Defines the AgoraThread structure and associated operations."]

//! Module for managing AgoraNet threads and their lifecycles.

use std::sync::Arc;
use tokio::sync::RwLock;
use icn_core_types::Cid;
use async_trait::async_trait;

use crate::error::AgoraError;
use crate::message::{Message, ProposalBody, ThreadAnchor};
use crate::storage::AsyncStorage;

/// Represents an AgoraNet discussion thread or proposal lifecycle.
/// Internally, it manages an ordered log of message CIDs and handles DAG anchoring.
#[derive(Debug)]
pub struct AgoraThread<S: AsyncStorage + Send + Sync + 'static> {
    /// Unique identifier for this thread (e.g., CID of the genesis message or derived).
    pub id: Cid,
    // Store message CIDs in order, messages themselves are in AsyncStorage.
    message_cids: Arc<RwLock<Vec<Cid>>>,
    // Keep track of the last anchor CID for linking
    last_anchor_cid: Arc<RwLock<Option<Cid>>>,
    // Keep track of message index where the last anchor occurred
    last_anchor_index: Arc<RwLock<usize>>,
    storage: Arc<S>,
}

impl<S: AsyncStorage + Send + Sync + 'static> AgoraThread<S> {
    /// Creates a new thread instance.
    pub fn new(id: Cid, storage: Arc<S>) -> Self {
        Self {
            id,
            message_cids: Arc::new(RwLock::new(Vec::new())),
            last_anchor_cid: Arc::new(RwLock::new(None)),
            last_anchor_index: Arc::new(RwLock::new(0)),
            storage,
        }
    }

    /// Checks if an anchor should be created based on message count.
    async fn should_anchor(&self) -> Result<bool, AgoraError> {
        const ANCHOR_EVERY: usize = 25; // Keep consistent with test
        let messages = self.message_cids.read().await;
        let last_anchor_idx = *self.last_anchor_index.read().await;
        let current_idx = messages.len();
        // Anchor if enough messages passed since last anchor OR if it's the first message
        // (We might want a specific genesis anchor later)
        Ok(!messages.is_empty() && (current_idx - last_anchor_idx >= ANCHOR_EVERY))
    }

    /// Creates and persists a `ThreadAnchor` IPLD object.
    async fn anchor_now(&self, tail_cid: &Cid) -> Result<Cid, AgoraError> {
        let prev_anchor = self.last_anchor_cid.read().await.clone();
        let merkle_root: [u8; 32] = {
            // Placeholder: Hash the tail CID for now.
            // TODO: Implement actual Merkle root calculation over message CIDs since last anchor.
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(tail_cid.hash().digest());
            hasher.finalize().into()
        };

        let anchor = ThreadAnchor {
            tail: tail_cid.clone(),
            merkle_root,
            timestamp: chrono::Utc::now().timestamp(),
            prev_anchor,
        };
        log::debug!("Creating anchor for thread {}: prev={:?}, tail={}, root={:?}", 
            self.id, anchor.prev_anchor, anchor.tail, hex::encode(anchor.merkle_root));
        
        let anchor_cid = self.storage.put_ipld(&anchor).await?;
        
        // Update last anchor state *after* successfully storing the new one
        {
            let msg_count = self.message_cids.read().await.len();
            *self.last_anchor_cid.write().await = Some(anchor_cid.clone());
            *self.last_anchor_index.write().await = msg_count; 
        }

        Ok(anchor_cid)
    }
}

/// Dummy struct for Proposal, representing the data associated with a proposal message body.
/// Used for type definition in the `lib.rs` re-export.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: Cid, // CID of the proposal message body
    pub content: ProposalBody,
}

/// Defines operations on an AgoraNet thread.
#[async_trait]
pub trait ThreadOperations: Send + Sync {
    /// Appends a message to the thread, persists it, and returns its CID.
    /// Optionally returns the CID of a new anchor if one was triggered.
    async fn append(&self, msg: Message) -> Result<(Cid, Option<Cid>), AgoraError>;

    // /// Loads a range of messages from the thread.
    // async fn load_range(&self, start_cid: Option<Cid>, limit: usize) -> Result<Vec<Arc<Message>>, AgoraError>;

    /// Triggers the DAG anchoring strategy manually based on the current tail message.
    /// Returns the CID of the new anchor if created, or None if the thread is empty.
    async fn anchor(&self) -> Result<Option<Cid>, AgoraError>;

    /// Gets a list of message CIDs starting from a specific index.
    async fn get_message_cids(&self, start_index: Option<usize>) -> Result<Vec<Cid>, AgoraError>;

    /// Retrieves a message by its CID from storage.
    async fn get_message(&self, cid: &Cid) -> Result<Option<Message>, AgoraError>;
}

#[async_trait]
impl<S: AsyncStorage + Send + Sync + 'static> ThreadOperations for AgoraThread<S> {
    /// Appends a message, stores it via IPLD, updates internal CID list, and potentially anchors.
    async fn append(&self, msg: Message) -> Result<(Cid, Option<Cid>), AgoraError> {
        let cid: Cid = self.storage.put_ipld(&msg).await?;
        let anchor_cid_opt = {
            let mut guard = self.message_cids.write().await;
            guard.push(cid.clone());
            
            // Drop guard before calling should_anchor/anchor_now to avoid deadlock
            drop(guard); 

            if self.should_anchor().await? {
                log::info!("Anchor triggered automatically for thread {} at message {}", self.id, cid);
                Some(self.anchor_now(&cid).await?)
            } else {
                None
            }
        };
        Ok((cid, anchor_cid_opt))
    }

    /// Manually trigger anchoring based on the last message.
    async fn anchor(&self) -> Result<Option<Cid>, AgoraError> {
        let tail_cid: Option<Cid> = {
            let guard = self.message_cids.read().await;
            guard.last().cloned() // Get the CID of the last message
        };
        if let Some(cid) = tail_cid {
            log::info!("Manual anchor requested for thread {} at message {}", self.id, cid);
            Ok(Some(self.anchor_now(&cid).await?))
        } else {
            log::warn!("Cannot anchor thread {} as it has no messages.", self.id);
            Ok(None) // Cannot anchor an empty thread
        }
    }

    /// Returns message CIDs starting from `start_index`.
    async fn get_message_cids(&self, start_index: Option<usize>) -> Result<Vec<Cid>, AgoraError> {
        let guard = self.message_cids.read().await;
        let begin = start_index.unwrap_or(0).min(guard.len());
        Ok(guard[begin..].to_vec())
    }

    /// Retrieves a specific message by its CID using get_ipld.
    async fn get_message(&self, cid: &Cid) -> Result<Option<Message>, AgoraError> {
        self.storage.get_ipld(cid).await
    }

    // Load range example (requires more work)
    // async fn load_range(&self, start_cid: Option<Cid>, limit: usize) -> Result<Vec<Arc<Message>>, AgoraError> {
    //     let all_cids = self.get_message_cids(None).await?;
    //     let start_idx = match start_cid {
    //         Some(start) => all_cids.iter().position(|c| c == &start).ok_or_else(|| AgoraError::InvalidInput("start_cid not found in thread".to_string()))?,
    //         None => 0,
    //     };
    //     let cids_to_load = all_cids.into_iter().skip(start_idx).take(limit).collect::<Vec<_>>();
    // 
    //     let mut messages = Vec::with_capacity(cids_to_load.len());
    //     for cid in cids_to_load {
    //         if let Some(msg) = self.get_message(&cid).await? {
    //             messages.push(Arc::new(msg));
    //         } else {
    //             // This case might indicate inconsistency between message_cids and storage
    //             log::error!("Message CID {} found in thread log but not in storage!", cid);
    //             return Err(AgoraError::Storage(format!("Message CID {} not found during range load", cid)));
    //         }
    //     }
    //     Ok(messages)
    // }
}

// --- Unit Tests --- 
// Removed inline tests - moved to tests/thread_tests.rs 