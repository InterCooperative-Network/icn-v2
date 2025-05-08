use crate::rocksdb_dag_store::{DagStore, NodeScope, DagStoreError};
use icn_common::dag::{DAGNode, DAGNodeID};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Error that can occur during runtime execution
#[derive(Debug, thiserror::Error)]
pub enum RuntimeExecutionError {
    #[error("Storage error: {0}")]
    Storage(#[from] DagStoreError),
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Unauthorized execution: {0}")]
    Unauthorized(String),
    
    #[error("Execution error: {0}")]
    Execution(String),
}

/// Result of runtime execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution was successful
    pub success: bool,
    
    /// Result data if any
    pub result: Option<serde_json::Value>,
    
    /// Error message if execution failed
    pub error: Option<String>,
    
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    
    /// Node ID that was executed
    pub node_id: String,
}

/// Runtime executor that uses DAG storage for lineage verification
pub struct DagVerifiedExecutor<S: DagStore> {
    /// DAG store for lineage verification
    dag_store: Arc<S>,
}

impl<S: DagStore> DagVerifiedExecutor<S> {
    /// Create a new executor with the given DAG store
    pub fn new(dag_store: Arc<S>) -> Self {
        Self { dag_store }
    }
    
    /// Execute a WASM module, verifying its lineage first
    pub async fn execute_wasm_module(
        &self,
        cid: &DAGNodeID,
        scope: &NodeScope,
    ) -> Result<ExecutionResult, RuntimeExecutionError> {
        // First, verify the lineage
        if !self.dag_store.verify_lineage(cid, scope).await? {
            return Err(RuntimeExecutionError::Unauthorized(
                format!("Node {} has unauthorized lineage for scope {}", 
                    cid.as_str(), 
                    scope.scope_id
                )
            ));
        }
        
        // Get the node
        let node = match self.dag_store.get_node(cid).await? {
            Some(node) => node,
            None => return Err(RuntimeExecutionError::NodeNotFound(cid.as_str().to_string())),
        };
        
        // In a real implementation, we would extract the WASM module from the node payload
        // and execute it in the WASM runtime
        // For now, we'll just return a simulated success result
        let result = ExecutionResult {
            success: true,
            result: Some(serde_json::json!({
                "message": format!("Successfully executed node {} in scope {}", 
                    cid.as_str(), scope.scope_id)
            })),
            error: None,
            execution_time_ms: 100, // Simulated execution time
            node_id: cid.as_str().to_string(),
        };
        
        info!("Executed node {} in scope {} after successful lineage verification", 
            cid.as_str(), scope.scope_id);
        
        Ok(result)
    }
    
    /// Add a node to the DAG and then execute it
    pub async fn append_and_execute(
        &self,
        node: DAGNode,
        scope: &NodeScope,
    ) -> Result<ExecutionResult, RuntimeExecutionError> {
        // First, add the node to the DAG
        let node_id = self.dag_store.append_node(node).await?;
        
        // Then, execute it
        self.execute_wasm_module(&node_id, scope).await
    }
} 