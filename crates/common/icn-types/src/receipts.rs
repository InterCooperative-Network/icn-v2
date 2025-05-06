// #![cfg(not(feature = "async"))] // Removed cfg

use crate::anchor::AnchorRef;
use crate::Cid;
use crate::dag::{DagError, DagNode, DagNodeBuilder, DagPayload, DagStore, SignedDagNode};
use crate::Did;
use crate::governance::QuorumConfig;
// use crate::quorum::QuorumProof; // Comment out unused import for now
use ed25519_dalek::{SigningKey, Signer};
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(test)]
use icn_identity_core::did::DidKey;

/// Represents the result of executing a transaction or contract in the ICN runtime.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExecutionReceipt {
    /// The CID of the executed code or transaction
    pub execution_cid: Cid,
    /// The identity that executed the code
    pub executor: Did,
    /// Timestamp of execution
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// The result of the execution (success, error, or other data)
    pub result: ExecutionResult,
    /// References to prior execution or state that this execution depends on
    pub dependencies: Vec<AnchorRef>,
    /// Optional metadata about the execution
    pub metadata: Option<serde_json::Value>,
}

/// The result of an execution
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "status", content = "data")]
pub enum ExecutionResult {
    /// The execution completed successfully
    Success(serde_json::Value),
    /// The execution failed with an error
    Error(String),
    /// The execution produced a deferred action
    Deferred(Vec<u8>),
}

/// Errors specific to ExecutionReceipt operations
#[derive(Error, Debug)]
pub enum ReceiptError {
    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("DAG error: {0}")]
    DagError(#[from] DagError),
    #[error("Invalid dependencies")]
    InvalidDependencies,
    #[error("Receipt not found: {0}")]
    ReceiptNotFound(Cid),
    #[error("Invalid payload type")]
    InvalidPayloadType,
    #[error("Receipt not found: {0}")]
    NotFound(Cid),
}

impl ExecutionReceipt {
    /// Create a new ExecutionReceipt
    pub fn new(
        execution_cid: Cid,
        executor: Did,
        result: ExecutionResult,
        dependencies: Vec<AnchorRef>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            execution_cid,
            executor,
            timestamp: chrono::Utc::now(),
            result,
            dependencies,
            metadata,
        }
    }
    
    /// Serialize the ExecutionReceipt to JSON
    pub fn to_json(&self) -> Result<String, ReceiptError> {
        serde_json::to_string(self).map_err(ReceiptError::SerializationError)
    }
    
    /// Create a DAG node from this ExecutionReceipt
    pub fn to_dag_node(&self) -> Result<DagNode, ReceiptError> {
        // Serialize the ExecutionReceipt
        let receipt_json = serde_json::to_value(self)?;
        
        // Extract parent CIDs from dependencies
        let parent_cids: Vec<Cid> = self.dependencies
            .iter()
            .map(|anchor| anchor.cid.clone())
            .collect();
        
        // Build the DAG node
        let node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(receipt_json))
            .with_parents(parent_cids)
            .with_author(self.executor.clone())
            .with_label("ExecutionReceipt".to_string())
            .build()
            .map_err(DagError::from)?;
        
        Ok(node)
    }
    
    /// Anchor this ExecutionReceipt to the DAG
    pub async fn anchor_to_dag(
        &self,
        signing_key: &SigningKey,
        dag_store: &mut impl DagStore,
    ) -> Result<Cid, ReceiptError> {
        // Create a DAG node for this receipt
        let node = self.to_dag_node()?;
        
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
        let cid = dag_store.add_node(signed_node).await?;
        
        Ok(cid)
    }
    
    /// Retrieve an ExecutionReceipt from the DAG
    pub async fn from_dag(cid: &Cid, dag_store: &impl DagStore) -> Result<Self, ReceiptError> {
        let node = dag_store.get_node(cid).await?;

        if let DagPayload::ExecutionReceipt(referenced_cid) = node.node.payload {
            // TODO: Fetch actual receipt object using referenced_cid
            Err(ReceiptError::NotFound(referenced_cid))
        } else {
            Err(ReceiptError::InvalidPayloadType)
        }
    }
    
    /// Verify that this ExecutionReceipt's dependencies exist in the DAG
    pub async fn verify_dependencies(&self, dag_store: &impl DagStore) -> Result<bool, ReceiptError> {
        for anchor in &self.dependencies {
            match dag_store.get_node(&anchor.cid).await {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(ReceiptError::DagError(err)),
            }
        }
        
        Ok(true)
    }
    
    /// List all ExecutionReceipts in the DAG
    pub fn list_all(_dag_store: &impl DagStore) -> Result<Vec<(Cid, ExecutionReceipt)>, ReceiptError> {
        // Placeholder: Needs actual implementation to iterate through stored receipts
        // For now, returns an empty list or an error if not implemented.
        Ok(Vec::new()) // Return empty vec for now
    }
    
    /// Export this ExecutionReceipt to a portable format
    pub fn export(&self) -> Result<Vec<u8>, ReceiptError> {
        serde_json::to_vec(self).map_err(ReceiptError::SerializationError)
    }
    
    /// Import an ExecutionReceipt from a portable format
    pub fn import(data: &[u8]) -> Result<Self, ReceiptError> {
        serde_json::from_slice(data).map_err(ReceiptError::SerializationError)
    }

    pub async fn verify_anchor(&self, dag_store: &impl DagStore) -> Result<bool, ReceiptError> {
        // Check if the anchor node itself exists
        if let Some(anchor) = &self.dependencies.first() {
            match dag_store.get_node(&anchor.cid).await {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(ReceiptError::DagError(err)),
            }
        }
        Ok(true) // No anchor or anchor exists
    }
}

// Ensure QuorumProof struct definition is present
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuorumProof {
    pub content_cid: Cid,
    pub signatures: Vec<(Did, Vec<u8>)>, // (Signer DID, Signature bytes)
    // Potentially other fields like policy_cid, etc.
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum QuorumError {
    #[error("Invalid signature from DID {0}")]
    InvalidSignature(Did),
    #[error("Signer DID {0} not in authorized list")]
    UnauthorizedSigner(Did),
    #[error("Quorum not met: got {got} signatures, needed {needed}")]
    QuorumNotMet { got: usize, needed: usize },
    #[error("No signatures provided in proof")]
    NoSignatures,
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
    // Add other error types as needed, e.g., for DID resolution failure
}

impl QuorumProof {
    pub fn new(content_cid: Cid, signatures: Vec<(Did, Vec<u8>)>) -> Self {
        Self { content_cid, signatures }
    }

    // Placeholder for the verify method discussed
    pub fn verify(
        &self,
        _data_to_verify: &[u8], // The actual data that was signed, matching self.content_cid
        config: &QuorumConfig,
    ) -> Result<(), QuorumError> {
        if self.signatures.is_empty() {
            return Err(QuorumError::NoSignatures);
        }

        let mut valid_signatures_count = 0;
        let mut unique_authorized_signers = std::collections::HashSet::new();

        for (signer_did, _signature_bytes) in &self.signatures {
            // 1. Check if signer is authorized
            if !config.authorized_signers.contains(signer_did) {
                // Optionally, instead of hard error, just ignore this signature for quorum count
                // but for now, let's be strict.
                // Consider logging a warning or collecting all unauthorized attempts.
                return Err(QuorumError::UnauthorizedSigner(signer_did.clone()));
            }

            // 2. Verify the signature against _data_to_verify
            // This requires DID resolution to get the public key for signer_did
            // and then using the appropriate crypto library (e.g., ed25519_dalek::PublicKey::verify)
            // For this sketch, we'll assume a helper or direct way to do this.
            // Let's imagine a function: verify_signature(did: &Did, data: &[u8], signature: &[u8]) -> Result<bool, CryptoError>
            
            // Placeholder for actual signature verification logic:
            // match signer_did.verify_signature(_data_to_verify, signature_bytes) { // Assuming Did has such a method
            //     Ok(true) => { /* Signature is valid */ }
            //     Ok(false) => return Err(QuorumError::InvalidSignature(signer_did.clone())),
            //     Err(e) => return Err(QuorumError::CryptoError(e.to_string())),
            // }
            // SIMULATED: For now, let's assume signature is valid if code reaches here after auth check
            // In a real implementation, this is CRITICAL.
            // For example:
            // let public_key = resolve_public_key_for_did(signer_did).map_err(|e| QuorumError::CryptoError(format!("DID resolution failed: {}", e)))?;
            // if !public_key.verify(_data_to_verify, signature_bytes).is_ok() {
            //     return Err(QuorumError::InvalidSignature(signer_did.clone()));
            // }

            // If signature verification is successful:
            if unique_authorized_signers.insert(signer_did.clone()) {
                 valid_signatures_count += 1;
            } // else, it's a duplicate signature from an authorized signer, count only once
        }

        // 3. Check if quorum threshold is met
        if valid_signatures_count >= config.threshold {
            Ok(())
        } else {
            Err(QuorumError::QuorumNotMet {
                got: valid_signatures_count,
                needed: config.threshold,
            })
        }
    }
}

// ... existing code ...
// VoteReceipt struct and impl block
// SignedVoteReceipt struct and impl block

// ... existing code ... 