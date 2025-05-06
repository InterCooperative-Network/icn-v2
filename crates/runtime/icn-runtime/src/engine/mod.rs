use wasmtime::*;
use anyhow::Result;
use std::sync::Arc;
use crate::abi::{bindings::register_host_functions, context::HostContext};

/// Executes WASM modules within a HostContext.
pub struct WasmExecutor<T: HostContext + Send + Sync + 'static> { // Added Send + Sync
    engine: Engine,
    linker: Linker<Arc<T>>, // Store type is now Arc<T>
}

impl<T: HostContext + Send + Sync + 'static> WasmExecutor<T> { // Added Send + Sync
    /// Creates a new executor, linking host functions defined in the ABI.
    pub fn new() -> Result<Self> { // Return Result for potential linker errors
        let mut config = Config::new();
        // config.async_support(true); // Keep sync for now based on bindings
        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);

        // Register host functions using the ABI bindings module
        register_host_functions(&mut linker)?;

        Ok(Self { engine, linker })
    }

    /// Runs a WASM module with the given context.
    /// Looks for standard entry points (_start, main, run).
    pub fn run_module(&self, wasm_bytes: &[u8], ctx: Arc<T>) -> Result<()> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        // Pass the Arc<T> context to the store
        let mut store = Store::new(&self.engine, ctx);

        // Instantiate using the pre-defined linker
        let instance = self.linker.instantiate(&mut store, &module)?;

        // Find and call the entry point function (sync)
        let entry_func = instance.get_typed_func::<(), ()>(&mut store, "_start")
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "main"))
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "run"))
            .map_err(|e| anyhow::anyhow!("No standard entrypoint (_start, main, run) found in module: {}", e))?;

        // Call the entry point (sync)
        entry_func.call(&mut store, ())?;

        Ok(())
    }
} 