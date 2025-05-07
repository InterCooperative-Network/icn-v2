use crate::anchor::AnchorRef;
use crate::Cid;
use crate::Did;
use chrono::{DateTime, Utc};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ed25519_dalek::VerifyingKey;
use std::fmt;
use std::sync::Arc;
// Removed specific imports, use full path in #[from]
// use serde_ipld_dagcbor::Error as DagCborError; 
// use rocksdb::Error as RocksDbLibError;      

// Include the RocksDB implementation
#[cfg(feature = "persistence")]
pub mod rocksdb;

// Include the in-memory implementation
pub mod memory;

// Include the sync service implementation
pub mod sync;

// Tests module
#[cfg(test)]
mod tests;

// Async tests module when the async feature is enabled
#[cfg(all(test, feature = "async"))]
mod tests_async;

// Re-export sync types for easier access
pub use sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};

pub mod event;
pub mod event_type;
pub mod event_id;
pub mod payload;
pub mod merkle;
pub mod node;
pub mod ipld;

pub use event::*;
pub use event_type::*;
pub use event_id::*;
pub use payload::*;
// pub use node::*; // Commented out unused import

/// Error types related to DAG operations
#[derive(Error, Debug)]
pub enum DagError {
    #[error("Node not found: {0}")]
    NodeNotFound(Cid),
    #[error("Parent node not found for child {child}: {parent}")]
    ParentNotFound { child: Cid, parent: Cid },
    #[error("Invalid signature for node {0}")]
    InvalidSignature(Cid),
    #[error("Error during DAG-CBOR serialization/deserialization: {0}")]
    SerializationError(String),
    #[error("Invalid node data: {0}")]
    InvalidNodeData(String),
    #[error("Public key resolution failed for DID {0}: {1}")]
    PublicKeyResolutionError(Did, String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("RocksDB error: {0}")]
    RocksDbError(#[from] ::rocksdb::Error),
    #[error("Join error from background task: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("CID calculation or parsing error: {0}")]
    CidError(String),
    #[error("CID mismatch detected for node: {0}")]
    CidMismatch(Cid),
    #[error("Missing parent node in DAG: {0}")]
    MissingParent(Cid),
}

/// Trait for resolving DIDs to public verifying keys
pub trait PublicKeyResolver: Send + Sync {
    fn resolve(&self, did: &Did) -> Result<VerifyingKey, DagError>;
    // Potentially add an async version if needed later
    // async fn resolve_async(&self, did: &Did) -> Result<VerifyingKey, DagError>;
}

/// Metadata associated with a DAG node
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DagNodeMetadata {
    /// Timestamp when the node was created
    pub timestamp: DateTime<Utc>,
    /// Optional sequence number for the author's chain
    pub sequence: Option<u64>,
    /// Optional federation identifier where this node originated
    pub federation_id: Option<String>,
    /// Optional label for categorizing the node
    pub labels: Option<Vec<String>>,
    /// Any additional metadata as JSON
    pub extra: Option<serde_json::Value>,
}

/// Defines the content types that can be stored in a DAG node
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "content")]
pub enum DagPayload {
    /// Raw binary data
    Raw(Vec<u8>),
    /// JSON data
    Json(serde_json::Value),
    /// A reference to another content-addressed object
    Reference(Cid),
    /// A TrustBundle reference
    TrustBundle(Cid),
    /// An execution receipt reference
    ExecutionReceipt(Cid),
}

/// Represents a single node in the Directed Acyclic Graph
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DagNode {
    /// The payload/content of this DAG node
    pub payload: DagPayload,
    /// References to parent nodes this node builds upon
    pub parents: Vec<Cid>,
    /// The DID of the identity that created this node
    pub author: Did,
    /// Metadata associated with this node
    pub metadata: DagNodeMetadata,
}

/// A signed DAG node ready for inclusion in the graph
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedDagNode {
    /// The unsigned DAG node
    pub node: DagNode,
    /// The author's signature over the canonical serialization of the node
    pub signature: Signature,
    /// The computed CID for this node (derived from its contents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<Cid>,
}

impl SignedDagNode {
    /// Calculate the CID for this node based on its canonical serialization (DAG-CBOR)
    pub fn calculate_cid(&self) -> Result<Cid, DagError> {
        // Serialize the inner node using DAG-CBOR for canonical representation
        let canonical_node_bytes = serde_ipld_dagcbor::to_vec(&self.node)
            .map_err(|e| DagError::SerializationError(e.to_string()))?;
            
        // Calculate CID using the canonical DAG-CBOR bytes
        Cid::from_bytes(&canonical_node_bytes)
            .map_err(|e| DagError::CidError(e.to_string()))
    }
    
    /// Ensure the CID is computed and stored
    pub fn ensure_cid(&mut self) -> Result<Cid, DagError> {
        if self.cid.is_none() {
            let cid = self.calculate_cid()?;
            self.cid = Some(cid.clone());
            Ok(cid)
        } else {
            Ok(self.cid.clone().unwrap())
        }
    }
    
    /// Create an AnchorRef from this node
    pub fn to_anchor_ref(&mut self) -> Result<AnchorRef, DagError> {
        let cid = self.ensure_cid()?;
        let object_type = match &self.node.payload {
            DagPayload::TrustBundle(_) => Some("TrustBundle".to_string()),
            DagPayload::ExecutionReceipt(_) => Some("ExecutionReceipt".to_string()),
            _ => None,
        };
        
        Ok(AnchorRef {
            cid,
            object_type,
            timestamp: self.node.metadata.timestamp,
        })
    }
}

/// Trait defining the interface for DAG storage backends
#[cfg_attr(feature = "async", async_trait::async_trait)]
pub trait DagStore {
    /// Add a signed node to the DAG
    #[cfg(feature = "async")]
    async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError>;
    
    /// Retrieve a node by its CID
    #[cfg(feature = "async")]
    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError>;
    
    /// Retrieve raw block data by its CID
    #[cfg(feature = "async")]
    async fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, DagError>;
    
    /// Get a list of the current tip nodes (nodes with no children)
    #[cfg(feature = "async")]
    async fn get_tips(&self) -> Result<Vec<Cid>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_tips(&self) -> Result<Vec<Cid>, DagError>;
    
    /// Get all nodes in a topologically ordered sequence
    #[cfg(feature = "async")]
    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError>;
    
    /// Get all nodes by a specific author
    #[cfg(feature = "async")]
    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError>;
    
    /// Get nodes matching a specific payload type
    #[cfg(feature = "async")]
    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError>;
    
    /// Find the path between two nodes (if one exists)
    #[cfg(feature = "async")]
    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError>;
    
    #[cfg(not(feature = "async"))]
    fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError>;
    
    /// Verify all signatures and structural integrity of a DAG branch, starting from a tip.
    /// Returns Ok(()) if valid, or an Err(DagError) indicating the first validation failure.
    #[cfg(feature = "async")]
    async fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError>;
    
    #[cfg(not(feature = "async"))]
    fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError>;
}

/// Builder for creating new DAG nodes
pub struct DagNodeBuilder {
    payload: Option<DagPayload>,
    parents: Vec<Cid>,
    author: Option<Did>,
    metadata: DagNodeMetadata,
}

impl DagNodeBuilder {
    /// Create a new DAG node builder with default values
    pub fn new() -> Self {
        Self {
            payload: None,
            parents: Vec::new(),
            author: None,
            metadata: DagNodeMetadata {
                timestamp: Utc::now(),
                sequence: None,
                federation_id: None,
                labels: None,
                extra: None,
            },
        }
    }
    
    /// Set the payload for this node
    pub fn with_payload(mut self, payload: DagPayload) -> Self {
        self.payload = Some(payload);
        self
    }
    
    /// Add a parent CID to this node
    pub fn with_parent(mut self, parent: Cid) -> Self {
        self.parents.push(parent);
        self
    }
    
    /// Add multiple parent CIDs to this node
    pub fn with_parents(mut self, parents: Vec<Cid>) -> Self {
        self.parents.extend(parents);
        self
    }
    
    /// Set the author's DID
    pub fn with_author(mut self, author: Did) -> Self {
        self.author = Some(author);
        self
    }
    
    /// Set the sequence number
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.metadata.sequence = Some(sequence);
        self
    }
    
    /// Set the federation ID
    pub fn with_federation_id(mut self, federation_id: String) -> Self {
        self.metadata.federation_id = Some(federation_id);
        self
    }
    
    /// Add a label to this node
    pub fn with_label(mut self, label: String) -> Self {
        if self.metadata.labels.is_none() {
            self.metadata.labels = Some(Vec::new());
        }
        if let Some(labels) = &mut self.metadata.labels {
            labels.push(label);
        }
        self
    }
    
    /// Set extra metadata
    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.metadata.extra = Some(extra);
        self
    }
    
    /// Build the DAG node
    pub fn build(self) -> Result<DagNode, DagError> {
        let payload = self.payload.ok_or_else(|| DagError::InvalidNodeData("Payload is required".to_string()))?;
        let author = self.author.ok_or_else(|| DagError::InvalidNodeData("Author is required".to_string()))?;
        
        Ok(DagNode {
            payload,
            parents: self.parents,
            author,
            metadata: self.metadata,
        })
    }
}

/// A wrapper for DagStore that provides shared mutable access
/// 
/// This is a convenience wrapper around Arc<tokio::sync::Mutex<Box<dyn DagStore>>>
/// to handle the mutability requirements of the DagStore trait while allowing
/// shared access across threads. Ideal for use in services that need to
/// share a DagStore across multiple components.
#[derive(Clone)]
pub struct SharedDagStore {
    inner: Arc<tokio::sync::Mutex<Box<dyn DagStore + Send + Sync>>>,
}

impl SharedDagStore {
    /// Create a new SharedDagStore from a boxed DagStore
    pub fn new(store: Box<dyn DagStore + Send + Sync>) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(store)),
        }
    }
    
    /// Create a new SharedDagStore from an existing Arc<Box<dyn DagStore>>
    pub fn from_arc(store: Arc<Box<dyn DagStore + Send + Sync>>) -> Self {
        // Convert the immutable Arc<Box<dyn DagStore>> to a mutable version
        // This is safe because the Mutex provides exclusive access
        let inner_box: Box<dyn DagStore + Send + Sync> = match Arc::try_unwrap(store) {
            Ok(boxed) => boxed,
            Err(arc) => {
                // If we can't get exclusive ownership, we need to clone the inner store
                // This should be avoided in production code but works for a transition
                let cloned = Box::new(ClonedDagStore::new(arc)) as Box<dyn DagStore + Send + Sync>;
                cloned
            }
        };
        
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(inner_box)),
        }
    }
    
    /// Add a node to the DAG store with shared mutable access
    pub async fn add_node(&self, node: SignedDagNode) -> Result<Cid, DagError> {
        let mut store = self.inner.lock().await;
        store.add_node(node).await
    }
    
    /// Get a node from the DAG store
    pub async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let store = self.inner.lock().await;
        store.get_node(cid).await
    }
    
    /// Get raw data from the DAG store
    pub async fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, DagError> {
        let store = self.inner.lock().await;
        store.get_data(cid).await
    }
    
    /// Get tip CIDs from the DAG store
    pub async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let store = self.inner.lock().await;
        store.get_tips().await
    }
    
    /// Get ordered nodes from the DAG store
    pub async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        let store = self.inner.lock().await;
        store.get_ordered_nodes().await
    }
    
    /// Get nodes by author
    pub async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let store = self.inner.lock().await;
        store.get_nodes_by_author(author).await
    }
    
    /// Get nodes by payload type
    pub async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let store = self.inner.lock().await;
        store.get_nodes_by_payload_type(payload_type).await
    }
    
    /// Find a path between nodes
    pub async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let store = self.inner.lock().await;
        store.find_path(from, to).await
    }
    
    /// Verify a branch of the DAG
    pub async fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        let store = self.inner.lock().await;
        store.verify_branch(tip, resolver).await
    }
}

// A helper struct to facilitate cloning Arc<Box<dyn DagStore>> safely
#[derive(Clone)]
struct ClonedDagStore {
    store: Arc<Box<dyn DagStore + Send + Sync>>,
}

impl ClonedDagStore {
    fn new(store: Arc<Box<dyn DagStore + Send + Sync>>) -> Self {
        Self { store }
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl DagStore for ClonedDagStore {
    async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError> {
        // This is unsafe but necessary for the transition
        // In a real implementation, the source DagStore would need to be thread-safe internally
        let store_ptr = Arc::as_ptr(&self.store);
        
        // SAFETY: This is unsafe and should be replaced with a proper solution
        // The real fix is to redesign the DagStore trait to not require &mut self
        unsafe {
            let store_mut = &mut **(store_ptr as *mut Box<dyn DagStore + Send + Sync>);
            store_mut.add_node(node).await
        }
    }
    
    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        self.store.get_node(cid).await
    }
    
    async fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, DagError> {
        self.store.get_data(cid).await
    }
    
    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        self.store.get_tips().await
    }
    
    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        self.store.get_ordered_nodes().await
    }
    
    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        self.store.get_nodes_by_author(author).await
    }
    
    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        self.store.get_nodes_by_payload_type(payload_type).await
    }
    
    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        self.store.find_path(from, to).await
    }
    
    async fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        self.store.verify_branch(tip, resolver).await
    }
} 