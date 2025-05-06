use async_trait::async_trait;
use crate::cid::Cid;
use crate::dag::sync::bundle::DAGSyncBundle;
use crate::dag::sync::network::{FederationPeer, SyncError}; // Use the types defined in network.rs
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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