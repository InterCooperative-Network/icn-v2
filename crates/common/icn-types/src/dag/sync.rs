use crate::cid::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode};
use thiserror::Error;
use std::collections::HashSet;

/// Errors specific to DAG synchronization
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("DAG error: {0}")]
    DagError(#[from] DagError),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid node data: {0}")]
    InvalidNodeData(String),
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Peer not found: {0}")]
    PeerNotFound(String),
}

/// Represents a remote peer in the federation network
#[derive(Clone, Debug)]
pub struct FederationPeer {
    /// Unique identifier for the peer
    pub id: String,
    /// Connection endpoint (e.g., URL) for the peer
    pub endpoint: String,
    /// Federation this peer belongs to
    pub federation_id: String,
    /// Optional metadata about the peer
    pub metadata: Option<serde_json::Value>,
}

/// A bundle of DAG nodes to be synchronized between peers
#[derive(Clone, Debug)]
pub struct DAGSyncBundle {
    /// The nodes included in this sync bundle
    pub nodes: Vec<SignedDagNode>,
    /// The federation this bundle originated from
    pub federation_id: String,
    /// The peer that sent this bundle
    pub source_peer: Option<String>,
    /// Timestamp when this bundle was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Verification result for a sync bundle
#[derive(Debug)]
pub struct VerificationResult {
    /// Whether the bundle passed verification
    pub is_valid: bool,
    /// Nodes that were accepted
    pub accepted_nodes: Vec<Cid>,
    /// Nodes that were rejected and why
    pub rejected_nodes: Vec<(Cid, String)>,
    /// Detailed verification report
    pub report: String,
}

/// Trait defining the interface for DAG synchronization between federation nodes
#[cfg_attr(feature = "async", async_trait::async_trait)]
pub trait DAGSyncService {
    /// Fetch nodes from a remote peer
    #[cfg(feature = "async")]
    async fn fetch_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn fetch_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError>;

    /// Offer nodes to a remote peer
    #[cfg(feature = "async")]
    async fn offer_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn offer_nodes(&self, peer: &FederationPeer, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError>;

    /// Accept nodes from a remote peer
    #[cfg(feature = "async")]
    async fn accept_bundle(&mut self, bundle: DAGSyncBundle) -> Result<VerificationResult, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn accept_bundle(&mut self, bundle: DAGSyncBundle) -> Result<VerificationResult, SyncError>;

    /// Verify a bundle of nodes from a remote peer
    #[cfg(feature = "async")]
    async fn verify_bundle(&self, bundle: &DAGSyncBundle) -> Result<VerificationResult, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn verify_bundle(&self, bundle: &DAGSyncBundle) -> Result<VerificationResult, SyncError>;

    /// Sync with a peer (fetch all nodes the peer has that we don't)
    #[cfg(feature = "async")]
    async fn sync_with_peer(&mut self, peer: &FederationPeer) -> Result<VerificationResult, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn sync_with_peer(&mut self, peer: &FederationPeer) -> Result<VerificationResult, SyncError>;

    /// Get missing dependencies for a set of nodes
    #[cfg(feature = "async")]
    async fn get_missing_dependencies(&self, nodes: &[SignedDagNode]) -> Result<Vec<Cid>, SyncError>;
    
    #[cfg(not(feature = "async"))]
    fn get_missing_dependencies(&self, nodes: &[SignedDagNode]) -> Result<Vec<Cid>, SyncError>;
} 