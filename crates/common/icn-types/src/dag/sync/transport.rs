use async_trait::async_trait;
use crate::Cid;
use crate::dag::sync::bundle::DAGSyncBundle;
use crate::dag::sync::network::{FederationPeer, SyncError}; // Use the types defined in network.rs
use crate::dag::{DagError, DagStore};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
use tokio::sync::RwLock; 
use std::sync::Arc;
use crate::dag::Utc;

/// Configuration for a transport implementation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportConfig {
    pub timeout_secs: Option<u64>,
    // Add other relevant config fields like bind address, retry logic etc.
}

/// Transport message types for DAG sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DAGSyncMessage {
    Offer(Vec<Cid>),
    Request(Vec<Cid>),
    Bundle(DAGSyncBundle),
    // TODO: Potentially other message types like Ack, Error, etc.
}

/// Trait defining the interface for DAG sync network transport implementations
#[async_trait]
pub trait DAGSyncTransport: Send + Sync {
    /// Get the local peer ID for this transport
    fn local_peer_id(&self) -> String;

    /// Check if connected to a specific peer
    async fn is_connected(&self, peer_id: &str) -> Result<bool, SyncError>;

    /// Send an offer of CIDs to a peer, returns the CIDs they need
    async fn send_offer(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError>;

    /// Send a bundle of nodes to a peer
    async fn send_bundle(&self, peer_id: &str, bundle: DAGSyncBundle) -> Result<(), SyncError>;

    /// Receive bundles from any connected peers
    async fn receive_bundles(&mut self) -> Result<(String, DAGSyncBundle), SyncError>; // Returns (peer_id, bundle)

    /// Request specific nodes (by CID) from a peer
    async fn request_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError>;

    /// Connect to a peer
    async fn connect(&mut self, peer: &FederationPeer) -> Result<(), SyncError>;

    /// Disconnect from a peer
    async fn disconnect(&mut self, peer_id: &str) -> Result<(), SyncError>;

    /// Discover peers
    async fn discover_peers(&self) -> Result<Vec<FederationPeer>, SyncError>;

    // Add other necessary methods like configure, listen, clone etc. if needed
}

// NOTE: Concrete implementations of DAGSyncTransport will need to handle Clone
// trait if required by NetworkDagSyncService construction/usage. 


// --- Mock Implementation for Testing ---
use crate::dag::memory::MemoryDagStore;

#[derive(Clone, Default)] 
pub struct MemoryDagTransport {
    local_peer_id: String,
    // Use Tokio RwLock here
    peers_stores: Arc<RwLock<HashMap<String, Arc<RwLock<MemoryDagStore>>>>>, 
}

impl MemoryDagTransport {
    /// Create a new mock transport for a specific peer ID.
    pub fn new(local_peer_id: String) -> Self {
        Self {
            local_peer_id,
            // Use Tokio RwLock here
            peers_stores: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Helper to get a write lock on peers_stores
    // This helper itself needs to be async now
    async fn get_peers_write_lock(&self) -> tokio::sync::RwLockWriteGuard<HashMap<String, Arc<RwLock<MemoryDagStore>>>> {
        self.peers_stores.write().await
    }
    
    // Helper to get a read lock on peers_stores
    // This helper itself needs to be async now
    async fn get_peers_read_lock(&self) -> tokio::sync::RwLockReadGuard<HashMap<String, Arc<RwLock<MemoryDagStore>>>> {
        self.peers_stores.read().await
    }
    
     // Helper to get a specific peer's store Arc (read lock on outer map)
    async fn get_peer_store_arc(&self, peer_id: &str) -> Result<Arc<RwLock<MemoryDagStore>>, SyncError> {
        let stores = self.get_peers_read_lock().await; // Await the read lock
        stores.get(peer_id)
            .cloned()
            .ok_or_else(|| SyncError::PeerNotFound(peer_id.to_string()))
    }
    
    /// Special method for mock setup: registers a peer and its store.
    /// This needs to be async now because get_peers_write_lock is async.
    pub async fn add_peer(&self, peer_id: String, store: Arc<RwLock<MemoryDagStore>>) -> Result<(), SyncError> {
        println!("MockTransport [{}]: Registering store for peer {}", self.local_peer_id, peer_id);
        let mut stores = self.get_peers_write_lock().await; // Await the write lock
        stores.insert(peer_id, store);
        Ok(())
    }
}

#[async_trait]
impl DAGSyncTransport for MemoryDagTransport {
    fn local_peer_id(&self) -> String {
        self.local_peer_id.clone()
    }

    async fn is_connected(&self, peer_id: &str) -> Result<bool, SyncError> {
        // In mock, being registered means being "connected"
        Ok(self.get_peers_read_lock().await.contains_key(peer_id))
    }

    async fn connect(&mut self, peer: &FederationPeer) -> Result<(), SyncError> {
        // No-op for the mock, connection is implicit via registration
        println!("MockTransport [{}]: connect({}) called (no-op)", self.local_peer_id, peer.peer_id);
        Ok(())
    }
    
    async fn disconnect(&mut self, peer_id: &str) -> Result<(), SyncError> {
        println!("MockTransport [{}]: disconnect({}) called", self.local_peer_id, peer_id);
        let mut stores = self.get_peers_write_lock().await; // Await lock
        stores.remove(peer_id);
        Ok(())
    }

    async fn discover_peers(&self) -> Result<Vec<FederationPeer>, SyncError> {
        println!("MockTransport [{}]: discover_peers called", self.local_peer_id);
        let stores = self.get_peers_read_lock().await; // Await lock
        Ok(stores.keys().filter(|&id| *id != self.local_peer_id) // Exclude self
           .map(|id| {
            // Create placeholder FederationPeer
            FederationPeer {
                peer_id: id.clone(),
                addresses: Vec::new(), // Mock has no real addresses
                last_seen: None, 
                metadata: HashMap::new(),
            }
        }).collect())
    }
    
    async fn send_offer(&self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        println!("MockTransport [{}]: send_offer of {} cids to {}", self.local_peer_id, cids.len(), peer_id);
        let peer_store_access_arc = self.get_peer_store_arc(peer_id).await?;
        
        let mut needed = HashSet::new();
        for cid in cids {
            // Acquire lock, call get_node on the guarded MemoryDagStore, then await
            let store_guard = peer_store_access_arc.read().await;
            match (*store_guard).get_node(cid).await { 
                 Ok(_) => { /* Peer has it, they don't need it */ },
                 Err(DagError::NodeNotFound(_)) => { 
                     needed.insert(cid.clone()); 
                 },
                 Err(e) => return Err(SyncError::Storage(format!("Error checking peer {} store for {}: {}", peer_id, cid, e))),
            }
            // store_guard is dropped here, releasing the read lock for this CID
        }
        println!("MockTransport [{}]: Peer {} needs {} out of {} offered cids", self.local_peer_id, peer_id, needed.len(), cids.len());
        Ok(needed)
    }
    
    async fn request_nodes(&self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError> {
        println!("MockTransport [{}]: request_nodes for {} cids from {}", self.local_peer_id, cids.len(), peer_id);
        let peer_store_access_arc = self.get_peer_store_arc(peer_id).await?;
        
        let mut fetched_nodes = Vec::new();
        for cid in cids {
            // Acquire lock, call get_node on the guarded MemoryDagStore, then await
            let store_guard = peer_store_access_arc.read().await;
            match (*store_guard).get_node(cid).await { 
                Ok(signed_node) => fetched_nodes.push(signed_node),
                Err(DagError::NodeNotFound(_)) => {
                    eprintln!("MockTransport Error: Peer {} promised node {} but doesn't have it!", peer_id, cid);
                    return Err(SyncError::Storage(format!("Peer {} does not have requested node {}", peer_id, cid)));
                },
                Err(e) => return Err(SyncError::Storage(format!("Error fetching node {} from peer {}: {}", cid, peer_id, e))),
            }
            // store_guard is dropped here, releasing the read lock for this CID
        }
        
        Ok(DAGSyncBundle {
            nodes: fetched_nodes.into_iter().map(|sn| sn.node).collect(),
            federation_id: "mock_federation".to_string(), 
            source_peer: Some(peer_id.to_string()),
            timestamp: Some(Utc::now()),
        })
    }
    
    // --- Methods likely left unimplemented or simple Ok(()) for Mock ---
    async fn send_bundle(&self, peer_id: &str, bundle: DAGSyncBundle) -> Result<(), SyncError> {
         println!("MockTransport [{}]: send_bundle of {} nodes to {} (no-op)", self.local_peer_id, bundle.nodes.len(), peer_id);
         Ok(())
    }

    async fn receive_bundles(&mut self) -> Result<(String, DAGSyncBundle), SyncError> {
        println!("MockTransport [{}]: receive_bundles called (unsupported)", self.local_peer_id);
        Err(SyncError::Internal("receive_bundles is not implemented for MemoryDagTransport".to_string()))
    }
} 