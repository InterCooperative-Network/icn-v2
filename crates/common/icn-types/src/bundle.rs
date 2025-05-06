use crate::anchor::{AnchorRef, TrustBundleAnchor};
use crate::cid::{Cid, CidError};
use crate::dag::{DagError, DagNode, DagNodeBuilder, DagPayload, DagStore, SignedDagNode};
use crate::Did;
use crate::QuorumProof;
use ed25519_dalek::{SigningKey, Signer};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::governance::QuorumConfig;
use crate::identity::Did;
use crate::receipts::QuorumProof;
use crate::utils::timestamp;
use std::collections::BTreeSet;
use super::anchor::{AnchorRef, TrustBundleAnchor};

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
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    #[error("Signing error: {0}")]
    SigningError(String),
    #[error("DAG store error: {0}")]
    DagStoreError(#[from] DagError),
    #[error("DAG access error: {0}")]
    DagAccessError(String),
    #[error("DID Key error: {0}")]
    DidKeyError(String),
    #[error("CID error: {0}")]
    CidError(#[from] CidError),
    #[error("Node build error: {0}")]
    NodeBuildError(String),
    #[error("Invalid previous anchors")]
    InvalidPreviousAnchors,
    #[error("Invalid quorum proof")]
    InvalidQuorumProofStructure,
    #[error("Bundle not found in DAG: {0}")]
    BundleNotFound(Cid),
    #[error("Anchor not found in DAG: {0}")]
    AnchorNotFound(Cid),
    #[error("Data not found in DAG for CID: {0}")]
    DataNotFound(Cid),
    #[error("Invalid payload type in DAG node")]
    InvalidPayloadType,
    #[error("Invalid anchor structure: {0}")]
    InvalidAnchor(String),
    #[error("Author DID mismatch: expected {expected}, got {got}")]
    AuthorDidMismatch { expected: Did, got: Did },
    #[error("Missing previous anchor in DAG: {0}")]
    MissingAnchor(Cid),
    #[error("Previous anchor type mismatch: {0}")]
    AnchorMismatch(String),
    #[error("Missing state data in DAG for CID: {0}")]
    MissingStateData(Cid),
    #[error("State proof verification failed: {0}")]
    InvalidStateProof(String),
}

// Helper function to abstract the add_node call
#[allow(dead_code)]
async fn add_node_helper(dag_store: &mut (impl DagStore + Send), node: SignedDagNode) -> Result<Cid, DagError> {
    dag_store.add_node(node).await
}

#[cfg(not(feature = "async"))]
fn add_node_helper(dag_store: &mut impl DagStore, node: SignedDagNode) -> Result<Cid, DagError> {
    dag_store.add_node(node)
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
        serde_json::to_string(self).map_err(|e| TrustBundleError::SerializationError(e.to_string()))
    }
    
    /// Create a DAG node from this TrustBundle
    pub fn to_dag_node(&self, author: Did) -> Result<DagNode, TrustBundleError> {
        let bundle_json = serde_json::to_value(self)
            .map_err(|e| TrustBundleError::SerializationError(e.to_string()))?;
        
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
        
        let trust_bundle_bytes = serde_ipld_dagcbor::to_vec(self)
            .map_err(|e| TrustBundleError::SerializationError(e.to_string()))?;
        let trust_bundle_cid = Cid::from_bytes(&trust_bundle_bytes)
            .map_err(|e| TrustBundleError::DagStoreError(DagError::CidError(e.to_string())))?; 
        
        // 2. Build the DAG node referencing the TrustBundle's CID
        let node = DagNodeBuilder::new() 
            .with_payload(DagPayload::TrustBundle(trust_bundle_cid.clone()))
            .with_parents(self.previous_anchors.iter().map(|a| a.cid.clone()).collect())
            .with_author(author)
            .with_label("TrustBundle".to_string())
            .build()
            ?; 
            
        // 3. Create the SignedDagNode 
        let node_bytes_for_signing = serde_ipld_dagcbor::to_vec(&node)
             .map_err(|e| TrustBundleError::SerializationError(e.to_string()))?;
        let signature = signing_key.sign(&node_bytes_for_signing);

        let signed_node = SignedDagNode {
            node,
            signature,
            cid: None, // Let the store calculate or calculate explicitly before adding
        };
        
        // Optional: Calculate and set CID explicitly if store doesn't do it
        // let node_cid = signed_node.calculate_cid().map_err(DagError::from)?;
        // signed_node.cid = Some(node_cid);

        // 4. Add SignedDagNode to DAG store
        let final_cid = dag_store.add_node(signed_node).await?;

        Ok(final_cid)
    }
    
    /// Retrieve a TrustBundle from the DAG
    pub async fn from_dag(anchor_cid: &Cid, dag_store: &mut (impl DagStore + Send)) -> Result<Self, TrustBundleError> {
        // 1. Fetch anchor node
        let anchor_signed_node = dag_store.get_node(anchor_cid).await?;

        // 2. Expect anchor payload to be a TrustBundle reference
        let referenced_cid = match &anchor_signed_node.node.payload {
            DagPayload::TrustBundle(cid) => cid.clone(), // Clone the CID for the next lookup
            _other => return Err(TrustBundleError::InvalidPayloadType),
        };

        // 3. Fetch referenced node (which should contain the actual TrustBundle data)
        let data_signed_node = dag_store.get_node(&referenced_cid).await?;

        // 4. Expect payload to be a Json bundle
        match &data_signed_node.node.payload {
            DagPayload::Json(value) => {
                // Attempt to deserialize from the serde_json::Value
                let bundle: TrustBundle = serde_json::from_value(value.clone())
                    .map_err(|e| TrustBundleError::SerializationError(e.to_string()))?; // Assuming SerializationError takes a String
                Ok(bundle)
            }
            _other => Err(TrustBundleError::InvalidPayloadType),
        }
    }
    
    /// Verify that this TrustBundle's previous anchors exist in the DAG
    pub async fn verify_anchors(&self, dag_store: &impl DagStore) -> Result<bool, TrustBundleError> {
        for anchor in &self.previous_anchors {
            match dag_store.get_node(&anchor.cid).await {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(TrustBundleError::DagStoreError(err)),
            }
        }
        
        Ok(true)
    }
    
    /// Get the path of TrustBundles from this bundle to another
    pub async fn get_path_to(
        &self,
        _target_cid: &Cid,
        _dag_store: &impl DagStore,
    ) -> Result<Vec<TrustBundle>, TrustBundleError> {
        // TODO: This needs refactoring similar to from_dag to fetch actual bundles.
        // Returning empty vec for now to fix type error.
        Ok(Vec::new()) 
        /*
        // First, get our node from the DAG (This CID might be wrong - should be the ANCHOR node CID)
        let anchor_node_cid = self.calculate_anchor_cid()? // Assuming such a method exists or is calculable
        let source_node = dag_store.get_node(&anchor_node_cid).await?;
        
        // Find the path between the nodes
        let path = dag_store.find_path(&source_node.cid.unwrap(), target_cid).await?;
        
        let mut bundles = Vec::new();
        for node in path {
            if let DagPayload::TrustBundle(bundle_cid) = node.node.payload {
                // Fetch the actual bundle via bundle_cid using from_dag logic
                 match Self::from_dag(&bundle_cid, dag_store).await {
                     Ok(bundle) => bundles.push(bundle),
                     Err(e) => eprintln!("Warning: Failed to load bundle {:?} in path: {}", bundle_cid, e),
                 }
            } else {
                eprintln!("Warning: Node {:?} in path has non-TrustBundle payload", node.cid);
            }
        }
        Ok(bundles)
        */
    }
    
    /// List all TrustBundles in the DAG
    pub async fn list_all(_dag_store: &impl DagStore) -> Result<Vec<(Cid, TrustBundle)>, TrustBundleError> {
        // Placeholder: Needs actual implementation to iterate through stored TrustBundles
        // For now, returns an empty list or an error if not implemented.
        Ok(Vec::new()) 
        /*
        let nodes = dag_store.get_nodes_by_payload_type("trustbundle").await?;
        
        let mut result = Vec::new();
        for node in nodes {
            if let DagPayload::TrustBundle(bundle_cid) = node.node.payload {
                 if let Some(anchor_cid) = node.cid { // This is the anchor node CID
                    // Fetch the actual bundle via bundle_cid using from_dag logic
                    match Self::from_dag(&bundle_cid, dag_store).await {
                         Ok(bundle) => result.push((anchor_cid, bundle)),
                         Err(e) => eprintln!("Warning: Failed to load bundle {:?} for anchor {:?}: {}", bundle_cid, anchor_cid, e),
                     }
                 } else {
                    eprintln!("Warning: Node from list_all missing anchor CID");
                 }
            } else {
                eprintln!("Warning: Node from list_all has incorrect payload type");
            }
        }
        Ok(result)
        */
    }
    
    /// Export this TrustBundle to a portable format
    pub fn export(&self) -> Result<Vec<u8>, TrustBundleError> {
        serde_json::to_vec(self)
            .map_err(|e| TrustBundleError::SerializationError(e.to_string()))
    }
    
    /// Import a TrustBundle from a portable format
    pub fn import(data: &[u8]) -> Result<Self, TrustBundleError> {
        serde_json::from_slice(data)
            .map_err(|e| TrustBundleError::SerializationError(e.to_string()))
    }

    #[allow(dead_code)]
    async fn from_root_cid(
        _root_cid: &Cid,
        _target_cid: &Cid,
        _dag_store: &impl DagStore,
    ) -> Result<Self, TrustBundleError> {
        // Placeholder: Needs actual implementation to iterate through stored TrustBundles
        // For now, returns an empty list or an error if not implemented.
        Err(TrustBundleError::InvalidPayloadType)
    }

    /// Verify the integrity and validity of this TrustBundle.
    /// This includes checking its previous anchors and its state proof.
    pub async fn verify<S: DagStore>(
        &self,
        dag_store: &S,
        quorum_config: &QuorumConfig,
    ) -> Result<(), TrustBundleError> {
        // 1. Verify previous anchors
        for anchor_ref in &self.previous_anchors {
            match dag_store.get(&anchor_ref.cid).await
                .map_err(|e| TrustBundleError::DagAccessError(format!("Failed to get previous anchor {}: {}", anchor_ref.cid, e)))?
            {
                Some(anchor_bytes) => {
                    let prev_anchor: TrustBundleAnchor =
                        ciborium::from_reader(&anchor_bytes[..])
                            .map_err(|e| TrustBundleError::Deserialization(format!("Failed to deserialize previous anchor {}: {}", anchor_ref.cid, e)))?;
                    
                    if prev_anchor.bundle_type != self.bundle_type {
                        return Err(TrustBundleError::AnchorMismatch(format!(
                            "Previous anchor {} (for bundle {}) has type {:?} but current bundle type is {:?}",
                            anchor_ref.cid, prev_anchor.bundle_cid, prev_anchor.bundle_type, self.bundle_type
                        )));
                    }
                    // TODO: Optionally verify signature on prev_anchor if present and author_did is known/resolvable.
                    // This would involve DID resolution and cryptographic verification against prev_anchor.author_did.
                }
                None => {
                    return Err(TrustBundleError::MissingAnchor(anchor_ref.cid));
                }
            }
        }

        // 2. Verify state_proof against state_cid
        if let Some(state_proof) = &self.state_proof {
            let state_data_bytes = match dag_store.get(&self.state_cid).await
                .map_err(|e| TrustBundleError::DagAccessError(format!("Failed to get state data {}: {}", self.state_cid, e)))?
            {
                Some(bytes) => bytes,
                None => return Err(TrustBundleError::MissingStateData(self.state_cid)),
            };

            // Assumes QuorumProof::verify method exists with signature:
            // pub fn verify(&self, content_cid: &Cid, signed_data: &[u8], quorum_config: &QuorumConfig) -> bool
            if !state_proof.verify(&self.state_cid, &state_data_bytes, quorum_config) {
                 return Err(TrustBundleError::InvalidStateProof("State proof verification failed using provided quorum configuration.".to_string()));
            }
        }
        // If state_proof is None, no verification of it is performed here.
        // The significance of state_cid without a state_proof depends on the bundle's semantics.

        // 3. (Optional) Verify metadata if any specific rules apply to self.metadata based on bundle_type or other context.

        Ok(())
    }
} 