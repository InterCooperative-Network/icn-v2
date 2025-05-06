use crate::cid::Cid;
use crate::dag::sync::{DAGSyncBundle, FederationPeer, SyncError};
use crate::identity::Did;
use async_trait::async_trait;
use std::collections::HashSet;

/// Protocol identifier for libp2p DAG sync
pub const DAG_SYNC_PROTOCOL_ID: &str = "/icn/dag-sync/1.0.0";

/// Trait defining the interface for DAG sync network transport implementations
#[async_trait]
pub trait DAGSyncTransport {
    /// Connect to a peer
    async fn connect(&mut self, peer: &FederationPeer) -> Result<(), SyncError>;
    
    /// Disconnect from a peer
    async fn disconnect(&mut self, peer_id: &str) -> Result<(), SyncError>;
    
    /// Check if connected to a peer
    async fn is_connected(&self, peer_id: &str) -> Result<bool, SyncError>;
    
    /// Send a bundle to a peer
    async fn send_bundle(&mut self, peer_id: &str, bundle: DAGSyncBundle) -> Result<(), SyncError>;
    
    /// Receive bundles from peers (may be a long-running operation)
    async fn receive_bundles(&mut self) -> Result<(String, DAGSyncBundle), SyncError>;
    
    /// Send an offer of node CIDs to a peer
    async fn send_offer(&mut self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError>;
    
    /// Request specific nodes from a peer
    async fn request_nodes(&mut self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError>;
    
    /// Discover peers in the network
    async fn discover_peers(&mut self) -> Result<Vec<FederationPeer>, SyncError>;
    
    /// Get local peer ID
    fn local_peer_id(&self) -> String;
    
    /// Get local peer DID
    fn local_did(&self) -> Option<Did>;
}

/// Transport message types for DAG sync
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DAGSyncMessage {
    /// Bundle of DAG nodes
    Bundle(DAGSyncBundle),
    /// Offer of node CIDs
    Offer { cids: Vec<Cid> },
    /// Request for specific nodes
    Request { cids: Vec<Cid> },
    /// Response to an offer with the CIDs the requester wants
    OfferResponse { cids: HashSet<Cid> },
    /// Peer discovery request
    DiscoveryRequest { federation_id: String },
    /// Peer discovery response
    DiscoveryResponse { peers: Vec<FederationPeer> },
}

/// Configuration for a transport implementation
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Local peer ID
    pub peer_id: String,
    /// Local federation ID
    pub federation_id: String,
    /// Local DID for signing messages
    pub local_did: Option<Did>,
    /// Listen addresses (format depends on transport implementation)
    pub listen_addresses: Vec<String>,
    /// Bootstrap peers (format depends on transport implementation)
    pub bootstrap_peers: Vec<String>,
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Enable KAD DHT discovery
    pub enable_kad_dht: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Timeout for requests in seconds
    pub request_timeout: u64,
} 