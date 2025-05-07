use wasmtime::*;
use anyhow::Result;
use std::sync::Arc;
use crate::abi::{bindings::register_host_functions, context::HostContext};
use icn_types::{Cid, dag::EventId, Did};
use crate::host::receipt::{issue_execution_receipt, ReceiptContextExt};
use crate::config::ExecutionConfig;
use icn_identity_core::did::DidKey;

// Placeholder for actual VmContext definition - this should provide config and dag_store access
pub trait RuntimeContextExt {
    fn get_execution_config(&self) -> &ExecutionConfig;
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)>;
    fn vm_context(&self) -> &dyn crate::abi::context::HostContext;
    
    // Implement convenience methods for ReceiptContextExt
    fn node_did(&self) -> Option<&Did> { None }
    fn federation_did(&self) -> Option<&Did> { None }
    fn caller_did(&self) -> Option<&Did> { None }
    fn federation_keypair(&self) -> Option<DidKey> { None }
}

// Implement ReceiptContextExt for any type that implements RuntimeContextExt
impl<T: RuntimeContextExt> ReceiptContextExt for T {
    fn node_did(&self) -> Option<&Did> {
        self.node_did()
    }
    
    fn federation_did(&self) -> Option<&Did> {
        self.federation_did()
    }
    
    fn caller_did(&self) -> Option<&Did> {
        self.caller_did()
    }
    
    fn federation_keypair(&self) -> Option<DidKey> {
        self.federation_keypair()
    }
}

// Implement RuntimeContextExt for Arc<T> where T: RuntimeContextExt
impl<T: RuntimeContextExt + ?Sized> RuntimeContextExt for Arc<T> {
    fn get_execution_config(&self) -> &ExecutionConfig {
        (**self).get_execution_config()
    }

    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)> {
        Arc::get_mut(self).and_then(|inner| inner.get_dag_store_mut())
    }

    fn vm_context(&self) -> &dyn crate::abi::context::HostContext {
        (**self).vm_context()
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

/// Executes WASM modules within a HostContext.
pub struct WasmExecutor<T: HostContext + RuntimeContextExt + Send + Sync + 'static> { 
    engine: Engine,
    linker: Linker<Arc<T>>, 
}

impl<T: HostContext + RuntimeContextExt + Send + Sync + 'static> WasmExecutor<T> {
    pub fn new() -> Result<Self> { 
        let mut config = Config::new();
        config.async_support(true); 
        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker)?;
        Ok(Self { engine, linker })
    }

    pub async fn run_module_async(
        &self, 
        wasm_bytes: &[u8], 
        ctx_arc: Arc<T>,
        module_cid: Cid,
        triggering_event_id: Option<EventId>,
    ) -> Result<()> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, ctx_arc.clone());
        let instance = self.linker.instantiate_async(&mut store, &module).await?;

        let entry_func_name;
        let entry_func: TypedFunc<(), ()>;
        if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
            entry_func = func;
            entry_func_name = "_start";
        } else if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, "main") {
            entry_func = func;
            entry_func_name = "main";
        } else if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, "run") {
            entry_func = func;
            entry_func_name = "run";
        } else {
            return Err(anyhow::anyhow!("No standard async entrypoint (_start, main, run) found in module."));
        }
        log::debug!("Found entry point: {}", entry_func_name);

        entry_func.call_async(&mut store, ()).await?;
        log::info!("WASM module executed successfully (async).");

        // --- ExecutionReceipt Issuance Hook ---
        let vm_ctx_ref = store.data();
        
        // We need to get the configuration options first to avoid borrowing issues
        let auto_issue_receipts = store.data().get_execution_config().auto_issue_receipts;
        let anchor_receipts = store.data().get_execution_config().anchor_receipts;
        let receipt_export_dir = store.data().get_execution_config().receipt_export_dir.clone();

        if auto_issue_receipts {
            // Create a dummy result CID - this should be properly implemented
            let result_cid = Cid::from_bytes(&[0u8; 32]).unwrap_or_else(|_| {
                // Fallback if the dummy CID creation fails
                Cid::from_bytes(&[1u8; 32]).unwrap()
            });
            
            match issue_execution_receipt(
                vm_ctx_ref,      
                &module_cid,
                &result_cid, 
                triggering_event_id.as_ref(),
            ) {
                Ok(receipt) => {
                    log::info!("🔏 ExecutionReceipt issued: {}", receipt.id);

                    if anchor_receipts {
                        let mut dag_anchored_successfully = false;
                        
                        // Access the data mutably
                        let host_data_mut = store.data_mut();
                        if let Some(dag_store_mut_ref) = host_data_mut.get_dag_store_mut() {
                            match crate::dag_anchor::anchor_execution_receipt(&receipt, dag_store_mut_ref, triggering_event_id).await {
                                Ok(anchored_event_id) => {
                                    log::info!("🧾 ExecutionReceipt anchored to DAG. Event ID: {}", anchored_event_id);
                                    dag_anchored_successfully = true;
                                }
                                Err(e) => log::error!("Failed to anchor receipt: {}", e),
                            }
                        } else {
                            log::warn!("DAG store not available (get_dag_store_mut returned None). Receipt not anchored.");
                        }
                        
                        if !dag_anchored_successfully && anchor_receipts {
                            log::warn!("DAG anchoring was configured but could not be performed due to access issues.");
                        }
                    }

                    if let Some(out_dir) = &receipt_export_dir {
                        if !out_dir.exists() {
                            if let Err(e) = std::fs::create_dir_all(out_dir) {
                                log::error!("Failed to create receipt export directory {}: {}", out_dir.display(), e);
                            } 
                        }
                        if out_dir.exists() {
                            let path = out_dir.join(format!("receipt-{}.json", receipt.id));
                            match serde_json::to_vec_pretty(&receipt) {
                                Ok(json_bytes) => {
                                    if let Err(e) = std::fs::write(&path, json_bytes) {
                                        log::error!("Failed to save receipt to file {}: {}", path.display(), e);
                                    } else {
                                        log::info!("📄 Receipt saved to: {}", path.display());
                                    }
                                }
                                Err(e) => log::error!("Failed to serialize receipt to JSON for saving: {}", e),
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to issue ExecutionReceipt: {}", e);
                }
            }
        }
        // --- End of Hook ---

        Ok(())
    }
} 