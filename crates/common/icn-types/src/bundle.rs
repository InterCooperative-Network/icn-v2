use crate::anchor::AnchorRef;
use crate::cid::Cid;
use crate::dag::{DagError, DagNode, DagNodeBuilder, DagPayload, DagStore, SignedDagNode, PublicKeyResolver};
use crate::identity::Did;
use crate::quorum::QuorumProof;
use ed25519_dalek::{Signature, SigningKey, Signer, Verifier, VerifyingKey};
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
    #[error("Invalid payload type")]
    InvalidPayloadType,
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
    pub async fn anchor_to_dag(
        &self,
        author: Did,
        signing_key: &SigningKey,
        dag_store: &mut impl DagStore,
    ) -> Result<Cid, TrustBundleError> {
        // Serialize payload
        let payload = DagPayload::TrustBundle(self.clone());
        let node = DagNodeBuilder::new(author, payload)
            .parents(self.previous_anchors.iter().map(|a| a.cid.clone()).collect())
            .build();

        // Calculate CID before signing
        let node_bytes = serde_json::to_vec(&node).map_err(|e| TrustBundleError::SerializationError(e))?;
        let node_cid = SignedDagNode::calculate_cid(&node_bytes)?;

        // Sign the canonical bytes
        let signature = signing_key.sign(&node_bytes);

        // Create signed node
        let signed_node = SignedDagNode {
            node,
            signature,
            cid: Some(node_cid),
        };

        // Add to DAG store
        let cid = dag_store.add_node(signed_node).await?;

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
    pub async fn from_dag(cid: &Cid, dag_store: &impl DagStore) -> Result<Self, TrustBundleError> {
        // Get the node from the DAG
        let node = dag_store.get_node(cid).await?;
        
        // Verify payload type
        if let DagPayload::TrustBundle(bundle) = node.node.payload {
            Ok(bundle)
        } else {
            Err(TrustBundleError::InvalidPayloadType)
        }
    }
    
    /// Verify that this TrustBundle's previous anchors exist in the DAG
    pub async fn verify_anchors(&self, dag_store: &impl DagStore) -> Result<bool, TrustBundleError> {
        for anchor in &self.previous_anchors {
            match dag_store.get_node(&anchor.cid).await {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(TrustBundleError::DagError(err)),
            }
        }
        
        Ok(true)
    }
    
    /// Get the path of TrustBundles from this bundle to another
    pub async fn get_path_to(
        &self,
        target_cid: &Cid,
        dag_store: &impl DagStore,
    ) -> Result<Vec<TrustBundle>, TrustBundleError> {
        // First, get our node from the DAG
        let source_node = dag_store.get_node(&self.state_cid).await?;
        
        // Find the path between the nodes
        let path = dag_store.find_path(&source_node.cid.unwrap(), target_cid).await?;
        
        // Convert each node in the path to a TrustBundle
        let mut bundles = Vec::new();
        for node in path {
            if let DagPayload::TrustBundle(bundle) = node.node.payload {
                bundles.push(bundle);
            } else {
                // Skip nodes with incorrect payload type in the path?
                // Or return an error?
                eprintln!("Warning: Node {:?} in path has non-TrustBundle payload", node.cid);
            }
        }
        
        Ok(bundles)
    }
    
    /// List all TrustBundles in the DAG
    pub async fn list_all(dag_store: &impl DagStore) -> Result<Vec<(Cid, TrustBundle)>, TrustBundleError> {
        // Get all nodes with TrustBundle payload type
        let nodes = dag_store.get_nodes_by_payload_type("trustbundle").await?;
        
        let mut result = Vec::new();
        for node in nodes {
            if let DagPayload::TrustBundle(bundle) = node.node.payload {
                if let Some(cid) = node.cid {
                    result.push((cid, bundle));
                } else {
                    // Should not happen if nodes come from store
                    eprintln!("Warning: Node from list_all missing CID");
                }
            } else {
                // Should not happen if get_nodes_by_payload_type works
                eprintln!("Warning: Node from list_all has incorrect payload type");
            }
        }
        Ok(result)
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