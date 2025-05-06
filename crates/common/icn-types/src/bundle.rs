use crate::anchor::AnchorRef;
use crate::cid::Cid;
use crate::dag::{DagError, DagNode, DagNodeBuilder, DagPayload, DagStore, SignedDagNode};
use crate::identity::Did;
use crate::quorum::QuorumProof;
use ed25519_dalek::{Signature, SigningKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(test)]
use icn_identity_core::did::DidKey;

/// A core data structure in ICN, representing a stateful object anchored to the DAG
/// and secured by a quorum proof.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrustBundle {
    /// The Content ID of the current state data associated with this bundle.
    pub state_cid: Cid,
    /// Proof that the current state is valid according to the governing policy.
    pub state_proof: QuorumProof,
    /// References to previous TrustBundles or other relevant DAG nodes this bundle builds upon.
    pub previous_anchors: Vec<AnchorRef>,
    /// Optional metadata about the bundle itself.
    pub metadata: Option<serde_json::Value>,
}

/// Errors specific to TrustBundle operations
#[derive(Error, Debug)]
pub enum TrustBundleError {
    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("DAG error: {0}")]
    DagError(#[from] DagError),
    #[error("Invalid previous anchors")]
    InvalidPreviousAnchors,
    #[error("Invalid quorum proof")]
    InvalidQuorumProof,
    #[error("Bundle not found: {0}")]
    BundleNotFound(Cid),
}

impl TrustBundle {
    /// Create a new TrustBundle
    pub fn new(
        state_cid: Cid,
        state_proof: QuorumProof,
        previous_anchors: Vec<AnchorRef>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            state_cid,
            state_proof,
            previous_anchors,
            metadata,
        }
    }
    
    /// Serialize the TrustBundle to JSON
    pub fn to_json(&self) -> Result<String, TrustBundleError> {
        serde_json::to_string(self).map_err(TrustBundleError::SerializationError)
    }
    
    /// Create a DAG node from this TrustBundle
    pub fn to_dag_node(&self, author: Did) -> Result<DagNode, TrustBundleError> {
        // Serialize the TrustBundle
        let bundle_json = serde_json::to_value(self)?;
        
        // Extract parent CIDs from previous anchors
        let parent_cids: Vec<Cid> = self.previous_anchors
            .iter()
            .map(|anchor| anchor.cid.clone())
            .collect();
        
        // Build the DAG node
        let node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(bundle_json))
            .with_parents(parent_cids)
            .with_author(author)
            .with_label("TrustBundle".to_string())
            .build()
            .map_err(DagError::from)?;
        
        Ok(node)
    }
    
    /// Anchor this TrustBundle to the DAG
    pub fn anchor_to_dag(
        &self,
        author: Did,
        signing_key: &SigningKey,
        dag_store: &mut impl DagStore,
    ) -> Result<Cid, TrustBundleError> {
        // Create a DAG node for this bundle
        let node = self.to_dag_node(author)?;
        
        // Serialize the node for signing
        let node_bytes = serde_json::to_vec(&node)?;
        
        // Sign the node
        let signature = signing_key.sign(&node_bytes);
        
        // Create a signed node
        let signed_node = SignedDagNode {
            node,
            signature,
            cid: None, // Will be computed when added to the DAG
        };
        
        // Add to the DAG store
        let cid = dag_store.add_node(signed_node)?;
        
        Ok(cid)
    }
    
    /// Anchor this TrustBundle to the DAG using a DidKey
    #[cfg(test)]
    pub fn anchor_to_dag_with_key(
        &self,
        author: Did,
        key: &DidKey,
        dag_store: &mut impl DagStore,
    ) -> Result<Cid, TrustBundleError> {
        // Create a DAG node for this bundle
        let node = self.to_dag_node(author)?;
        
        // Serialize the node for signing
        let node_bytes = serde_json::to_vec(&node)?;
        
        // Sign the node
        let signature = key.sign(&node_bytes);
        
        // Create a signed node
        let signed_node = SignedDagNode {
            node,
            signature,
            cid: None, // Will be computed when added to the DAG
        };
        
        // Add to the DAG store
        let cid = dag_store.add_node(signed_node)?;
        
        Ok(cid)
    }
    
    /// Retrieve a TrustBundle from the DAG
    pub fn from_dag(cid: &Cid, dag_store: &impl DagStore) -> Result<Self, TrustBundleError> {
        // Get the node from the DAG
        let node = dag_store.get_node(cid)?;
        
        // Extract the TrustBundle from the node's payload
        match &node.node.payload {
            DagPayload::Json(value) => {
                serde_json::from_value(value.clone()).map_err(TrustBundleError::SerializationError)
            }
            _ => Err(TrustBundleError::BundleNotFound(cid.clone())),
        }
    }
    
    /// Verify that this TrustBundle's previous anchors exist in the DAG
    pub fn verify_anchors(&self, dag_store: &impl DagStore) -> Result<bool, TrustBundleError> {
        for anchor in &self.previous_anchors {
            match dag_store.get_node(&anchor.cid) {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(TrustBundleError::DagError(err)),
            }
        }
        
        Ok(true)
    }
    
    /// Get the path of TrustBundles from this bundle to another
    pub fn get_path_to(
        &self,
        target_cid: &Cid,
        dag_store: &impl DagStore,
    ) -> Result<Vec<TrustBundle>, TrustBundleError> {
        // First, get our node from the DAG
        let source_node = dag_store.get_node(&self.state_cid)?;
        
        // Find the path between the nodes
        let path = dag_store.find_path(&source_node.cid.unwrap(), target_cid)?;
        
        // Convert each node in the path to a TrustBundle
        let mut bundles = Vec::new();
        for node in path {
            if let DagPayload::Json(value) = &node.node.payload {
                let bundle: TrustBundle = serde_json::from_value(value.clone())?;
                bundles.push(bundle);
            }
        }
        
        Ok(bundles)
    }
    
    /// List all TrustBundles in the DAG
    pub fn list_all(dag_store: &impl DagStore) -> Result<Vec<(Cid, TrustBundle)>, TrustBundleError> {
        // Get all nodes with TrustBundle payload type
        let nodes = dag_store.get_nodes_by_payload_type("trustbundle")?;
        
        // Convert each node to a TrustBundle
        let mut bundles = Vec::new();
        for node in nodes {
            if let DagPayload::Json(value) = &node.node.payload {
                let bundle: TrustBundle = serde_json::from_value(value.clone())?;
                bundles.push((node.cid.unwrap(), bundle));
            }
        }
        
        Ok(bundles)
    }
    
    /// Export this TrustBundle to a portable format
    pub fn export(&self) -> Result<Vec<u8>, TrustBundleError> {
        serde_json::to_vec(self).map_err(TrustBundleError::SerializationError)
    }
    
    /// Import a TrustBundle from a portable format
    pub fn import(data: &[u8]) -> Result<Self, TrustBundleError> {
        serde_json::from_slice(data).map_err(TrustBundleError::SerializationError)
    }
} 