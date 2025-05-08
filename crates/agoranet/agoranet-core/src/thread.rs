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
    storage: Arc<S>,
}

impl<S: AsyncStorage + Send + Sync + 'static> AgoraThread<S> {
    /// Creates a new thread instance.
    pub fn new(id: Cid, storage: Arc<S>) -> Self {
        Self {
            id,
            message_cids: Arc::new(RwLock::new(Vec::new())),
            storage,
        }
    }

    /// Checks if an anchor should be created based on message count.
    async fn should_anchor(&self) -> Result<bool, AgoraError> {
        // TODO: Make configurable via ThreadConfig or similar
        const ANCHOR_EVERY: usize = 25;
        let guard = self.message_cids.read().await;
        // Only anchor if there are messages and the count hits the threshold
        Ok(!guard.is_empty() && guard.len() % ANCHOR_EVERY == 0)
    }

    /// Creates and persists a `ThreadAnchor` IPLD object pointing to the given tail message CID.
    async fn anchor_now(&self, tail_cid: &Cid) -> Result<Cid, AgoraError> {
        let anchor = ThreadAnchor {
            tail: tail_cid.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        log::debug!("Creating anchor for thread {} pointing to tail {}", self.id, tail_cid);
        // Use put_ipld from AsyncStorage trait
        self.storage.put_ipld(&anchor).await
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
        // 1. Persist the message using put_ipld, get its CID
        let cid: Cid = self.storage.put_ipld(&msg).await?;

        // 2. Add the CID to the in-memory list
        let anchor_cid = {
            let mut guard = self.message_cids.write().await;
            guard.push(cid.clone());
            
            // 3. Check if anchoring is needed and perform it (within the write lock scope is fine)
            if self.should_anchor().await? {
                log::info!("Anchor triggered automatically for thread {} at message {}", self.id, cid);
                Some(self.anchor_now(&cid).await?)
            } else {
                None
            }
        }; // Release write lock here

        Ok((cid, anchor_cid))
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