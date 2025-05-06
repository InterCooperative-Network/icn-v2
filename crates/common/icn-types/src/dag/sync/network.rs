use crate::cid::Cid;
use crate::dag::{DagNode, DagStore};
use crate::dag::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};
use crate::dag::sync::transport::{DAGSyncTransport, TransportConfig};
use crate::identity::Did;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Policy for how DAG synchronization should be performed
#[derive(Debug, Clone)]
pub struct SyncPolicy {
    /// Minimum number of peers required for quorum verification
    pub min_quorum: usize,
    /// Set of authorized DIDs that can provide valid DAG nodes
    pub authorized_dids: Option<HashSet<Did>>,
    /// Rate limit for sync operations (nodes per minute)
    pub rate_limit: Option<usize>,
    /// Maximum bundle size in number of nodes
    pub max_bundle_size: usize,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            min_quorum: 1,
            authorized_dids: None, 
            rate_limit: None,
            max_bundle_size: 1000,
        }
    }
}

/// DAG sync service implementation that uses a network transport
pub struct NetworkDagSyncService<T: DAGSyncTransport, D: DagStore> {
    /// The underlying transport
    transport: T,
    /// The local DAG store
    store: Arc<D>,
    /// Connected peers
    peers: Arc<RwLock<HashMap<String, FederationPeer>>>,
    /// Federation ID
    federation_id: String,
    /// Local DID
    local_did: Option<Did>,
    /// Sync policy
    policy: SyncPolicy,
}

impl<T: DAGSyncTransport, D: DagStore> NetworkDagSyncService<T, D> {
    /// Create a new network DAG sync service with the given transport and store
    pub fn new(transport: T, store: Arc<D>, federation_id: String, local_did: Option<Did>) -> Self {
        Self {
            transport,
            store,
            peers: Arc::new(RwLock::new(HashMap::new())),
            federation_id,
            local_did,
            policy: SyncPolicy::default(),
        }
    }

    /// Set the sync policy
    pub fn with_policy(mut self, policy: SyncPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Start background sync tasks
    pub async fn start_background_sync(&self) -> Result<(), SyncError> {
        // Clone what we need for the background task
        let transport_clone = self.transport.clone();
        let store_clone = self.store.clone();
        let federation_id = self.federation_id.clone();
        
        // Spawn a task to receive bundles
        tokio::spawn(async move {
            loop {
                match transport_clone.receive_bundles().await {
                    Ok((peer_id, bundle)) => {
                        // Process the bundle
                        if bundle.federation_id == federation_id {
                            for node in &bundle.nodes {
                                // Store the node
                                if let Err(e) = store_clone.put(node).await {
                                    eprintln!("Failed to store node: {:?}", e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Error receiving bundle: {:?}", e);
                        // Add delay to avoid spinning on errors
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Verify that a set of nodes meets the sync policy requirements
    async fn verify_against_policy(&self, nodes: &[DagNode]) -> VerificationResult {
        // Check if we have authorized DIDs configured
        if let Some(ref authorized_dids) = self.policy.authorized_dids {
            for node in nodes {
                // Check if the node has a valid signature
                if let Some(ref auth) = node.auth {
                    // Check if the DID is authorized
                    if !authorized_dids.contains(&auth.did) {
                        return VerificationResult::Rejected {
                            reason: format!("Node from unauthorized DID: {}", auth.did),
                        };
                    }
                    
                    // In a real implementation, we would also verify the signature here
                } else {
                    // Node has no auth, reject if we require authorized DIDs
                    return VerificationResult::Rejected {
                        reason: "Node missing required auth signature".to_string(),
                    };
                }
            }
        }
        
        // All checks passed
        VerificationResult::Verified
    }
}

#[async_trait]
impl<T: DAGSyncTransport + Clone + Send + Sync + 'static, D: DagStore + Send + Sync + 'static> DAGSyncService for NetworkDagSyncService<T, D> {
    async fn offer_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        // Check if we're connected to the peer
        if !self.transport.is_connected(peer_id).await? {
            return Err(SyncError::PeerNotFound(peer_id.to_string()));
        }
        
        // Send the offer and get the requested CIDs
        self.transport.send_offer(peer_id, cids).await
    }

    async fn accept_offer(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        // Check local store for which CIDs we don't have
        let mut needed = HashSet::new();
        for cid in cids {
            if !self.store.exists(cid).await? {
                needed.insert(cid.clone());
            }
        }

        // Apply bundle size limit from policy
        if needed.len() > self.policy.max_bundle_size {
            // Truncate to max size - in practice we'd want to prioritize or paginate
            needed = needed.into_iter().take(self.policy.max_bundle_size).collect();
        }
        
        Ok(needed)
    }

    async fn fetch_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError> {
        // Check if we're connected to the peer
        if !self.transport.is_connected(peer_id).await? {
            return Err(SyncError::PeerNotFound(peer_id.to_string()));
        }
        
        // Apply policy size limit if needed
        let cids_to_fetch = if cids.len() > self.policy.max_bundle_size {
            &cids[0..self.policy.max_bundle_size]
        } else {
            cids
        };
        
        // Request the nodes
        self.transport.request_nodes(peer_id, cids_to_fetch).await
    }

    async fn verify_nodes(&self, nodes: &[DagNode]) -> VerificationResult {
        // Verify against our policy
        self.verify_against_policy(nodes).await
    }

    async fn broadcast_nodes(&self, nodes: &[DagNode]) -> Result<(), SyncError> {
        // Create a bundle with the nodes
        let bundle = DAGSyncBundle {
            nodes: nodes.to_vec(),
            federation_id: self.federation_id.clone(),
            source_peer: Some(self.transport.local_peer_id()),
            timestamp: chrono::Utc::now(),
        };
        
        // Get connected peers
        let peers = self.peers.read().unwrap();
        
        // Broadcast to all peers (in a real implementation, we might be more selective)
        for (peer_id, _) in peers.iter() {
            if let Err(e) = self.transport.send_bundle(peer_id, bundle.clone()).await {
                eprintln!("Failed to send bundle to {}: {:?}", peer_id, e);
            }
        }
        
        Ok(())
    }

    async fn connect_peer(&self, peer: &FederationPeer) -> Result<(), SyncError> {
        // Check if peer is already connected
        if self.transport.is_connected(&peer.id).await? {
            return Ok(());
        }
        
        // Connect to the peer
        self.transport.connect(peer).await?;
        
        // Add to known peers
        if let Ok(mut peers) = self.peers.write() {
            peers.insert(peer.id.clone(), peer.clone());
        }
        
        Ok(())
    }

    async fn discover_peers(&self) -> Result<Vec<FederationPeer>, SyncError> {
        // Discover peers through the transport
        self.transport.discover_peers().await
    }
}

// Add clone implementation for the transport
impl<T: DAGSyncTransport + Clone, D: DagStore> Clone for NetworkDagSyncService<T, D> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            store: self.store.clone(),
            peers: self.peers.clone(),
            federation_id: self.federation_id.clone(),
            local_did: self.local_did.clone(),
            policy: self.policy.clone(),
        }
    }
} 