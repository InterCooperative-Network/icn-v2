use wasmtime::*;
use anyhow::{Context, Result};
use crate::config::ExecutionConfig;
use crate::host::receipt::issue_execution_receipt;
use crate::abi::bindings::register_host_functions;
use crate::abi::context::HostContext;
use icn_types::{Cid, dag::EventId, Did};
use icn_identity_core::did::DidKey;
use std::path::Path;
use std::fs;
use std::sync::Arc;
use std::time::Instant;
use log::{info, debug, warn, error};

/// Result of a WASM execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// CID of the executed module
    pub module_cid: Cid,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Fuel consumed (if metering was enabled)
    pub fuel_consumed: Option<u64>,
    /// CID of the result data
    pub result_cid: Cid,
}

/// Context extension trait for WASM execution
pub trait ContextExtension {
    /// Get execution configuration
    fn get_execution_config(&self) -> &ExecutionConfig;
    
    /// Get mutable access to the DAG store if available
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)>;
    
    /// Get node DID if available
    fn node_did(&self) -> Option<&Did> { None }
    
    /// Get federation DID if available
    fn federation_did(&self) -> Option<&Did> { None }
    
    /// Get caller DID if available
    fn caller_did(&self) -> Option<&Did> { None }
    
    /// Get federation keypair if available
    fn federation_keypair(&self) -> Option<DidKey> { None }
}

// Implement ContextExtension for Arc<T> where T: ContextExtension
impl<T: ContextExtension + ?Sized> ContextExtension for Arc<T> {
    fn get_execution_config(&self) -> &ExecutionConfig {
        (**self).get_execution_config()
    }

    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)> {
        Arc::get_mut(self).and_then(|inner| inner.get_dag_store_mut())
    }
    
    fn node_did(&self) -> Option<&Did> {
        (**self).node_did()
    }
    
    fn federation_did(&self) -> Option<&Did> {
        (**self).federation_did()
    }
    
    fn caller_did(&self) -> Option<&Did> {
        (**self).caller_did()
    }
    
    fn federation_keypair(&self) -> Option<DidKey> {
        (**self).federation_keypair()
    }
}

/// Executes WASM modules and provides resource usage metrics
pub struct ModernWasmExecutor {
    engine: Engine,
}

impl ModernWasmExecutor {
    /// Create a new WASM executor
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(true); // Enable fuel for execution metering
        
        let engine = Engine::new(&config)?;
        
        Ok(Self { engine })
    }
    
    /// Load WASM module from a file path
    pub fn load_module_from_file(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let wasm_bytes = fs::read(path.as_ref())
            .with_context(|| format!("Failed to read WASM module from {}", path.as_ref().display()))?;
        
        Ok(wasm_bytes)
    }
    
    /// Validate a WASM module against ICN requirements
    pub fn validate_module(&self, wasm_bytes: &[u8]) -> Result<bool> {
        // Create a module and check for compatibility
        let _ = Module::new(&self.engine, wasm_bytes)
            .with_context(|| "Failed to validate WASM module")?;
        
        // TODO: Add additional validation specific to ICN requirements
        // 1. Check for approved import namespaces
        // 2. Verify module doesn't use restricted features
        // 3. Enforce memory limits
            
        Ok(true)
    }
    
    /// Execute a WASM module with the given context and input
    pub async fn execute<T>(&self, 
        wasm_bytes: &[u8], 
        ctx: Arc<T>,
        module_cid: Cid,
        event_id: Option<EventId>,
        input_data: Option<&[u8]>,
        fuel_limit: Option<u64>
    ) -> Result<ExecutionResult> 
    where 
        T: HostContext + ContextExtension + Send + Sync + 'static 
    {
        let start_time = Instant::now();
        
        // Create module from wasm bytes
        let module = Module::new(&self.engine, wasm_bytes)
            .with_context(|| "Failed to create WASM module")?;
        
        // Create linker and register host functions
        let mut linker = Linker::new(&self.engine);
        register_host_functions(&mut linker)?;
        
        // Create store with context
        let mut store = Store::new(&self.engine, ctx);
        
        // Configure fuel if limit is specified
        if let Some(limit) = fuel_limit {
            store.add_fuel(limit)
                .with_context(|| "Failed to add fuel to store")?;
        }
        
        // Instantiate module
        let instance = linker.instantiate_async(&mut store, &module).await
            .with_context(|| "Failed to instantiate WASM module")?;
        
        // Find entry point
        let entry_func = self.find_entry_point(&instance, &mut store)?;
        
        // Set input data in the context if provided
        if let Some(data) = input_data {
            // This would ideally use a context method to set input data
            // For now we're just demonstrating the flow
            debug!("Setting input data of {} bytes", data.len());
        }
        
        // Execute the WASM module
        debug!("Executing WASM module...");
        entry_func.call_async(&mut store, ()).await
            .with_context(|| "Failed to execute WASM module")?;
        
        // Get fuel consumption if enabled
        let fuel_consumed = if fuel_limit.is_some() {
            // store.fuel_consumed() returns Option<u64> directly
            store.fuel_consumed()
        } else {
            None
        };
        
        // Compute execution time
        let execution_time = start_time.elapsed();
        info!("WASM module executed in {:?}", execution_time);
        
        // Create result
        let result = ExecutionResult {
            module_cid,
            execution_time_ms: execution_time.as_millis() as u64,
            fuel_consumed,
            // In a real implementation, we'd capture the actual output
            result_cid: Cid::from_bytes(&[0u8; 32]).unwrap_or_else(|_| {
                Cid::from_bytes(&[1u8; 32]).unwrap()
            }),
        };
        
        // Handle receipt generation if configured
        self.handle_receipt_generation(&mut store, &result, event_id).await?;
        
        Ok(result)
    }
    
    /// Find a valid entry point in the WASM module
    fn find_entry_point<T>(&self, 
        instance: &Instance, 
        store: &mut Store<Arc<T>>
    ) -> Result<TypedFunc<(), ()>> 
    where 
        T: HostContext + Send + Sync + 'static 
    {
        // Try standard entry points in order of preference
        for name in &["_start", "main", "run", "execute"] {
            if let Ok(func) = instance.get_typed_func::<(), ()>(&mut *store, name) {
                debug!("Found entry point: {}", name);
                return Ok(func);
            }
        }
        
        Err(anyhow::anyhow!("No supported entry point found in module"))
    }
    
    /// Handle receipt generation based on execution configuration
    async fn handle_receipt_generation<T>(
        &self,
        store: &mut Store<Arc<T>>,
        result: &ExecutionResult,
        event_id: Option<EventId>
    ) -> Result<Option<String>>
    where 
        T: HostContext + ContextExtension + Send + Sync + 'static 
    {
        // Check if receipt generation is enabled in the execution config
        // We can access this directly through the ContextExtension implementation on Store
        let auto_issue_receipts = store.get_execution_config().auto_issue_receipts;
        let anchor_receipts = store.get_execution_config().anchor_receipts;
        let receipt_export_dir = store.get_execution_config().receipt_export_dir.clone();
        
        if !auto_issue_receipts {
            return Ok(None);
        }
        
        // Generate and sign execution receipt using the store directly as it implements ContextExtension
        match issue_execution_receipt(
            store, // Pass the store directly instead of ctx_ref.as_ref()
            &result.module_cid,
            &result.result_cid,
            event_id.as_ref()
        ) {
            Ok(receipt) => {
                info!("🔏 ExecutionReceipt issued: {}", receipt.id);
                
                // Anchor receipt to DAG if configured
                if anchor_receipts {
                    let mut dag_anchored_successfully = false;
                    
                    // Directly use get_dag_store_mut on the store 
                    if let Some(dag_store_mut_ref) = store.get_dag_store_mut() {
                        match crate::dag_anchor::anchor_execution_receipt(&receipt, dag_store_mut_ref, event_id).await {
                            Ok(anchored_event_id) => {
                                info!("🧾 ExecutionReceipt anchored to DAG. Event ID: {}", anchored_event_id);
                                dag_anchored_successfully = true;
                            }
                            Err(e) => error!("Failed to anchor receipt: {}", e),
                        }
                    } else {
                        warn!("DAG store not available. Receipt not anchored.");
                    }
                    
                    if !dag_anchored_successfully && anchor_receipts {
                        warn!("DAG anchoring was configured but could not be performed.");
                    }
                }
                
                // Export receipt to file if configured
                if let Some(out_dir) = &receipt_export_dir {
                    if !out_dir.exists() {
                        if let Err(e) = std::fs::create_dir_all(out_dir) {
                            error!("Failed to create receipt export directory {}: {}", out_dir.display(), e);
                        }
                    }
                    
                    if out_dir.exists() {
                        let path = out_dir.join(format!("receipt-{}.json", receipt.id));
                        match serde_json::to_vec_pretty(&receipt) {
                            Ok(json_bytes) => {
                                if let Err(e) = std::fs::write(&path, json_bytes) {
                                    error!("Failed to save receipt to file {}: {}", path.display(), e);
                                } else {
                                    info!("📄 Receipt saved to: {}", path.display());
                                }
                            }
                            Err(e) => error!("Failed to serialize receipt to JSON for saving: {}", e),
                        }
                    }
                }
                
                Ok(Some(receipt.id))
            }
            Err(e) => {
                error!("Failed to issue ExecutionReceipt: {}", e);
                Ok(None)
            }
        }
    }
} 