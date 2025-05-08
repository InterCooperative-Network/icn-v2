use crate::Cid;
use crate::dag::{DagError, DagNode, DagStore};
use crate::dag::sync::network::{DAGSyncService, FederationPeer, SyncError, VerificationResult};
use crate::dag::sync::bundle::DAGSyncBundle;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

/// In-memory implementation of the DAG sync service
#[derive(Clone)]
pub struct MemoryDAGSyncService<S: DagStore + Send + Sync + 'static> {
    local_peer_id: String,
    peers: Arc<RwLock<HashMap<String, FederationPeer>>>,
    dag_store: Arc<RwLock<S>>,
    federation_id: String,
}

impl<S: DagStore + Send + Sync + 'static> MemoryDAGSyncService<S> {
    /// Create a new MemoryDAGSyncService
    pub fn new(local_peer_id: String, federation_id: String, dag_store: Arc<RwLock<S>>) -> Self {
        Self {
            local_peer_id,
            federation_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            dag_store,
        }
    }
}

#[async_trait]
impl<S: DagStore + Send + Sync + 'static> DAGSyncService for MemoryDAGSyncService<S> {
    async fn offer_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        println!("MemoryDAGSyncService: Received offer from {}", peer_id);
        let mut needed = HashSet::new();
        let store = self.dag_store.read().await;
        for cid in cids {
            match store.get_node(cid).await {
                Ok(_) => { /* Node exists, do nothing */ }
                Err(DagError::NodeNotFound(_)) => {
                    needed.insert(cid.clone());
                }
                Err(e) => {
                    println!("MemoryDAGSyncService::offer_nodes - Error checking node {}: {}", cid, e);
                    return Err(SyncError::Storage(format!("Store error checking node {}: {}", cid, e)));
                }
            }
        }
        Ok(needed)
    }

    async fn accept_offer(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        println!("MemoryDAGSyncService: Accepting offer from {} for {} cids", peer_id, cids.len());
        let mut needed = HashSet::new();
        let store = self.dag_store.read().await;
        for cid in cids {
            match store.get_node(cid).await {
                Ok(_) => { /* Node exists, do nothing */ }
                Err(DagError::NodeNotFound(_)) => {
                    needed.insert(cid.clone());
                }
                Err(e) => {
                    println!("MemoryDAGSyncService::accept_offer - Error checking node {}: {}", cid, e);
                    return Err(SyncError::Storage(format!("Store error checking node {}: {}", cid, e)));
                }
            }
        }
        Ok(needed)
    }

    async fn fetch_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError> {
        println!("MemoryDAGSyncService: Fetching nodes for {}", peer_id);
        let mut fetched_nodes = Vec::new();
        let store = self.dag_store.read().await;
        for cid in cids {
            match store.get_node(cid).await {
                Ok(signed_node) => {
                    fetched_nodes.push(signed_node.node)
                },
                Err(DagError::NodeNotFound(_)) => {
                    return Err(SyncError::Storage(format!("Node not found locally during fetch: {}", cid)));
                }
                Err(e) => {
                    return Err(SyncError::Storage(format!("Store error during fetch: {}", e)));
                }
            }
        }
        
        Ok(DAGSyncBundle {
            nodes: fetched_nodes,
            federation_id: self.federation_id.clone(),
            source_peer: Some(self.local_peer_id.clone()),
            timestamp: Some(Utc::now()),
        })
    }

    async fn verify_nodes(&self, nodes: &[DagNode]) -> VerificationResult {
        println!("MemoryDAGSyncService: Verifying {} nodes", nodes.len());
        for node in nodes {
            let fid = &node.metadata.federation_id;
            if fid != &self.federation_id {
                return VerificationResult::Rejected { reason: format!("Node federation ID {} mismatch (expected {})", fid, self.federation_id) };
            }
        }
        VerificationResult::Verified
    }

    async fn broadcast_nodes(&self, nodes: &[DagNode]) -> Result<(), SyncError> {
         println!("MemoryDAGSyncService: Broadcasting {} nodes (no-op)", nodes.len());
         Ok(())
    }

    async fn connect_peer(&self, peer: &FederationPeer) -> Result<(), SyncError> {
        println!("MemoryDAGSyncService: Connecting peer {}", peer.peer_id);
        let mut peers = self.peers.write().await;
        peers.insert(peer.peer_id.clone(), peer.clone());
        Ok(())
    }

    async fn disconnect_peer(&self, peer_id: &str) -> Result<(), SyncError> {
        println!("MemoryDAGSyncService: Disconnecting peer {}", peer_id);
        let mut peers = self.peers.write().await;
        peers.remove(peer_id);
        Ok(())
    }

    async fn discover_peers(&self) -> Result<Vec<FederationPeer>, SyncError> {
        println!("MemoryDAGSyncService: Discovering peers");
        let peers = self.peers.read().await;
        Ok(peers.values().cloned().collect())
    }
} 