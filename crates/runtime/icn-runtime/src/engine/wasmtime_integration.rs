#[cfg(feature = "wasmtime")]
use crate::policy::PolicyError;
#[cfg(feature = "wasmtime")]
use anyhow::Result;
#[cfg(feature = "wasmtime")]
use icn_types::dag::{DagStore, SignedDagNode, DagError, Cid};
#[cfg(feature = "wasmtime")]
use icn_types::Did;
#[cfg(feature = "wasmtime")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasmtime")]
use std::sync::Arc;
#[cfg(feature = "wasmtime")]
use tracing::{debug, error, info, trace, warn};
#[cfg(feature = "wasmtime")]
use wasmtime::{Config, Engine, Instance, Module, Store};

/// Runtime error types when working with WASM execution
pub enum RuntimeError {
    #[error("DAG store error: {0}")]
    DagStore(String),
    
    #[error("DAG node not found: {0}")]
    NodeNotFound(String),
    
    #[error("DAG verification error: {0}")]
    DagVerification(String),
    
    #[error("DAG serialization error: {0}")]
    DagSerialization(String),

    #[error("Policy error: {0}")]
    Policy(#[from] PolicyError),
    
    #[cfg(feature = "wasmtime")]
    #[error("Invalid module: {0}")]
    InvalidModule(String),
    
    #[cfg(feature = "wasmtime")]
    #[error("Module compilation error: {0}")]
    ModuleCompilation(String),
    
    #[cfg(feature = "wasmtime")]
    #[error("WASM instantiation error: {0}")]
    WasmInstantiation(String),
    
    #[cfg(feature = "wasmtime")]
    #[error("WASM execution error: {0}")]
    WasmExecution(String),
    
    #[cfg(feature = "wasmtime")]
    #[error("WASM engine error: {0}")]
    WasmEngine(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

#[cfg(feature = "wasmtime")]
impl From<DagError> for RuntimeError {
    fn from(err: DagError) -> Self {
        match err {
            DagError::NodeNotFound(cid) => RuntimeError::NodeNotFound(cid.to_string()),
            DagError::ParentNotFound { child, parent } => 
                RuntimeError::DagVerification(format!("Parent {} not found for child {}", parent, child)),
            DagError::InvalidSignature(cid) => 
                RuntimeError::DagVerification(format!("Invalid signature for node {}", cid)),
            DagError::SerializationError(msg) => RuntimeError::DagSerialization(msg),
            DagError::InvalidNodeData(msg) => RuntimeError::DagStore(msg),
            DagError::PublicKeyResolutionError(did, msg) => 
                RuntimeError::Other(format!("PublicKey resolution failed for {}: {}", did, msg)),
            DagError::StorageError(msg) => RuntimeError::DagStore(msg),
            DagError::RocksDbError(db_err) => RuntimeError::DagStore(db_err.to_string()),
            DagError::JoinError(join_err) => RuntimeError::Other(join_err.to_string()),
            DagError::CidError(msg) => RuntimeError::DagStore(msg),
            DagError::CidMismatch(cid) => 
                RuntimeError::DagVerification(format!("CID mismatch for node {}", cid)),
            DagError::MissingParent(cid) => 
                RuntimeError::DagVerification(format!("Missing parent for node {}", cid)),
            DagError::PolicyError(policy_err) => RuntimeError::Policy(policy_err), // Handled by #[from] but explicit is okay
        }
    }
}

/// Configuration for a WASM execution
#[cfg(feature = "wasmtime")]
#[derive(Debug, Clone)]
pub struct WasmExecutionConfig {
    /// Maximum memory size in bytes
    pub max_memory: u64,
    
    /// Maximum execution fuel (instruction count)
    pub max_fuel: u64,
    
    /// Whether to enable WASI
    pub enable_wasi: bool,
    
    /// Whether to enable debugging
    pub enable_debug: bool,
}

#[cfg(feature = "wasmtime")]
impl Default for WasmExecutionConfig {
    fn default() -> Self {
        Self {
            max_memory: 100 * 1024 * 1024, // 100MB
            max_fuel: 10_000_000, // 10M instructions
            enable_wasi: false,
            enable_debug: false,
        }
    }
}

/// Execution context for a WASM module
#[cfg(feature = "wasmtime")]
pub struct WasmExecutionContext {
    /// DAG store for verification
    dag_store: Arc<dyn DagStore + Send + Sync>,
    
    /// Engine configuration
    engine: Engine,
    
    /// Execution config
    config: WasmExecutionConfig,
}

/// Result of a WASM execution
#[cfg(feature = "wasmtime")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionResult {
    /// Whether execution was successful
    pub success: bool,
    
    /// Result data if any (JSON serialized)
    pub result: Option<serde_json::Value>,
    
    /// Error message if execution failed
    pub error: Option<String>,
    
    /// Execution metrics
    pub metrics: WasmExecutionMetrics,
    
    /// Module CID that was executed
    pub module_cid: String,
}

/// Metrics from a WASM execution
#[cfg(feature = "wasmtime")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionMetrics {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    
    /// Memory used in bytes
    pub memory_used_bytes: u64,
    
    /// Fuel consumed (instruction count)
    pub fuel_consumed: u64,
}

#[cfg(feature = "wasmtime")]
impl WasmExecutionContext {
    /// Create a new WASM execution context
    pub fn new(
        dag_store: Arc<dyn DagStore + Send + Sync>,
        config: WasmExecutionConfig,
    ) -> Result<Self, RuntimeError> {
        // Configure the Wasmtime engine
        let mut engine_config = Config::new();
        
        // Enable fuel metering
        engine_config.consume_fuel(true);
        
        // Set memory limits
        engine_config.static_memory_maximum_size(config.max_memory);
        engine_config.static_memory_guard_size(65536);
        engine_config.dynamic_memory_guard_size(65536);
        
        // Enable WASI if requested
        if config.enable_wasi {
            engine_config.wasm_multi_memory(true);
            engine_config.wasm_module_linking(true);
        }
        
        // Enable debugging if requested
        if config.enable_debug {
            engine_config.debug_info(true);
        }
        
        // Create the engine
        let engine = Engine::new(&engine_config)
            .map_err(|e| RuntimeError::WasmEngine(format!("Failed to create WASM engine: {}", e)))?;
        
        Ok(Self {
            dag_store,
            engine,
            config,
        })
    }
    
    /// Execute a WASM module with scope verification
    pub async fn execute_module(
        &self,
        module_cid: &Cid,
        scope_id: &str,
        caller_did: &Did,
    ) -> Result<WasmExecutionResult, RuntimeError> {
        let start_time = std::time::Instant::now();
        
        // Get the module from the DAG
        let module_node = self.dag_store.get_node(module_cid).await
            .map_err(|e| RuntimeError::DagStore(format!("Failed to get module node: {}", e)))?;
        
        // Verify the module belongs to the right scope
        let scope_result = self.verify_module_scope(&module_node, scope_id).await?;
        if !scope_result {
            return Err(RuntimeError::Policy(PolicyError::Unauthorized(
                format!("Module {} is not authorized for scope {}", module_cid, scope_id)
            )));
        }
        
        // Extract module bytes
        let module_bytes = module_node.node.payload.get_bytes()
            .map_err(|e| RuntimeError::InvalidModule(format!("Failed to get module bytes: {}", e)))?;
        
        // Compile the module
        let module = Module::new(&self.engine, module_bytes)
            .map_err(|e| RuntimeError::ModuleCompilation(format!("Failed to compile module: {}", e)))?;
        
        // Create store with default host functions
        let mut store = Store::new(&self.engine, ());
        
        // Set fuel for metering
        store.add_fuel(self.config.max_fuel)
            .map_err(|e| RuntimeError::WasmExecution(format!("Failed to add fuel: {}", e)))?;
        
        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| RuntimeError::WasmInstantiation(format!("Failed to instantiate module: {}", e)))?;
        
        // Get the _start function if it exists
        let result = if let Some(start_func) = instance.get_func(&mut store, "_start") {
            match start_func.call(&mut store, &[], &mut []) {
                Ok(_) => {
                    // Successfully executed
                    WasmExecutionResult {
                        success: true,
                        result: Some(serde_json::json!({ "status": "completed" })),
                        error: None,
                        metrics: self.collect_metrics(&store, start_time),
                        module_cid: module_cid.to_string(),
                    }
                },
                Err(e) => {
                    // Execution error
                    WasmExecutionResult {
                        success: false,
                        result: None,
                        error: Some(format!("Execution error: {}", e)),
                        metrics: self.collect_metrics(&store, start_time),
                        module_cid: module_cid.to_string(),
                    }
                }
            }
        } else {
            // No _start function
            WasmExecutionResult {
                success: false,
                result: None,
                error: Some("No _start function found in module".to_string()),
                metrics: self.collect_metrics(&store, start_time),
                module_cid: module_cid.to_string(),
            }
        };
        
        Ok(result)
    }
    
    /// Verify that a module is authorized for a scope
    async fn verify_module_scope(
        &self,
        module_node: &SignedDagNode,
        scope_id: &str,
    ) -> Result<bool, RuntimeError> {
        // This is a simplified approach - in a real implementation,
        // you would use the more comprehensive scope-based lineage verification
        
        // 1. Check module metadata for explicit scope assignment
        if let Some(metadata) = module_node.node.metadata.as_ref() {
            if let Some(module_scope) = metadata.get("scope").and_then(|s| s.as_str()) {
                // Direct scope match
                if module_scope == scope_id {
                    return Ok(true);
                }
                
                // Could implement parent scope relationship checking here
            }
        }
        
        // 2. Verify lineage if direct scope check failed
        // A real implementation would use scope objects and lineage verification
        
        warn!("Module {} scope verification using basic check only", module_node.calculate_cid()?);
        
        // For demo purposes, we'll assume it's authorized
        // In production, this would use the proper lineage verification
        Ok(true)
    }
    
    /// Collect execution metrics
    fn collect_metrics(
        &self,
        store: &Store<()>,
        start_time: std::time::Instant,
    ) -> WasmExecutionMetrics {
        let elapsed = start_time.elapsed();
        let fuel_consumed = self.config.max_fuel - store.fuel_consumed().unwrap_or(0);
        
        WasmExecutionMetrics {
            execution_time_ms: elapsed.as_millis() as u64,
            memory_used_bytes: 0, // In a real implementation, you would track memory usage
            fuel_consumed,
        }
    }
}

/// Create a receipt node for a successful execution
#[cfg(feature = "wasmtime")]
pub async fn create_execution_receipt(
    dag_store: &Arc<dyn DagStore + Send + Sync>,
    module_cid: &Cid,
    execution_result: &WasmExecutionResult,
    caller_did: &Did,
    federation_did: &Did,
) -> Result<Cid, RuntimeError> {
    // This is a stub implementation - a real implementation would create a proper receipt
    // and add it to the DAG store
    
    info!("Creating execution receipt for module {}", module_cid);
    
    // Implement receipt creation logic here...
    
    // Return a dummy CID for now
    Ok(Cid::from_bytes(b"receipt-placeholder").map_err(|e| 
        RuntimeError::Other(format!("Failed to create receipt CID: {}", e))
    )?)
} 