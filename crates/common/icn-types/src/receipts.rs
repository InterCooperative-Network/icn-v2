use crate::anchor::AnchorRef;
use crate::cid::Cid;
use crate::dag::{DagError, DagNode, DagNodeBuilder, DagPayload, DagStore, SignedDagNode};
use crate::identity::Did;
// use crate::quorum::QuorumProof; // Comment out unused import for now
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, SigningKey};
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
    pub fn anchor_to_dag(
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
        let cid = dag_store.add_node(signed_node)?;
        
        Ok(cid)
    }
    
    /// Retrieve an ExecutionReceipt from the DAG
    pub fn from_dag(cid: &Cid, dag_store: &impl DagStore) -> Result<Self, ReceiptError> {
        // Get the node from the DAG
        let node = dag_store.get_node(cid)?;
        
        // Extract the ExecutionReceipt from the node's payload
        match &node.node.payload {
            DagPayload::Json(value) => {
                serde_json::from_value(value.clone()).map_err(ReceiptError::SerializationError)
            }
            _ => Err(ReceiptError::ReceiptNotFound(cid.clone())),
        }
    }
    
    /// Verify that this ExecutionReceipt's dependencies exist in the DAG
    pub fn verify_dependencies(&self, dag_store: &impl DagStore) -> Result<bool, ReceiptError> {
        for anchor in &self.dependencies {
            match dag_store.get_node(&anchor.cid) {
                Ok(_) => {}, // Node exists
                Err(DagError::NodeNotFound(_)) => return Ok(false),
                Err(err) => return Err(ReceiptError::DagError(err)),
            }
        }
        
        Ok(true)
    }
    
    /// List all ExecutionReceipts in the DAG
    pub fn list_all(dag_store: &impl DagStore) -> Result<Vec<(Cid, ExecutionReceipt)>, ReceiptError> {
        // Get all nodes with ExecutionReceipt payload type
        let nodes = dag_store.get_nodes_by_payload_type("receipt")?;
        
        // Convert each node to an ExecutionReceipt
        let mut receipts = Vec::new();
        for node in nodes {
            if let DagPayload::Json(value) = &node.node.payload {
                let receipt: ExecutionReceipt = serde_json::from_value(value.clone())?;
                receipts.push((node.cid.unwrap(), receipt));
            }
        }
        
        Ok(receipts)
    }
    
    /// Export this ExecutionReceipt to a portable format
    pub fn export(&self) -> Result<Vec<u8>, ReceiptError> {
        serde_json::to_vec(self).map_err(ReceiptError::SerializationError)
    }
    
    /// Import an ExecutionReceipt from a portable format
    pub fn import(data: &[u8]) -> Result<Self, ReceiptError> {
        serde_json::from_slice(data).map_err(ReceiptError::SerializationError)
    }

    /// Anchor this ExecutionReceipt to the DAG using a DidKey
    #[cfg(test)]
    pub fn anchor_to_dag_with_key(
        &self,
        key: &DidKey,
        dag_store: &mut impl DagStore,
    ) -> Result<Cid, ReceiptError> {
        // Create a DAG node for this receipt
        let node = self.to_dag_node()?;
        
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
} 