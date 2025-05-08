use crate::error::RuntimeError;
use crate::metering::{ResourceLimits, ResourceMeter, ResourceUsageCollector};
use crate::config::{RuntimeConfig, WasiConfig};
use crate::host::{HostState, HostFunctions, register_host_functions};
use icn_common::identity::{ScopedIdentity, Credential};
use icn_common::resource::{ResourceType, Receipt, ResourceUsage};
use icn_common::verification::Signature;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wasmtime::{Config as WasmConfig, Engine, Module, Store, Linker, Instance, Trap, ExternRef};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Result of a WebAssembly execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether the execution was successful
    pub success: bool,
    
    /// Return value if any (serialized to JSON)
    pub return_value: Option<serde_json::Value>,
    
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    
    /// Resource usage during execution
    pub resource_usage: HashMap<ResourceType, u64>,
    
    /// Any error message if execution failed
    pub error_message: Option<String>,
    
    /// Exit code if the module called exit()
    pub exit_code: Option<i32>,
}

/// Receipt for a completed execution with resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Unique ID for this receipt
    pub id: String,
    
    /// The identity that executed the module
    pub executor: ScopedIdentity,
    
    /// The identity that requested the execution
    pub requester: String,
    
    /// The scope this execution occurred in
    pub scope: String,
    
    /// The module ID that was executed
    pub module_id: String,
    
    /// Execution result
    pub result: ExecutionResult,
    
    /// Timestamp of execution
    pub timestamp: u64,
    
    /// Signature of the executor
    pub signature: Signature,
}

impl ExecutionReceipt {
    /// Create a new execution receipt
    pub fn new(
        executor: ScopedIdentity,
        requester: String,
        scope: String,
        module_id: String,
        result: ExecutionResult,
        private_key: &ed25519_dalek::SecretKey,
    ) -> Result<Self, RuntimeError> {
        let id = Uuid::new_v4().to_string();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a receipt without signature for signing
        let temp_receipt = Self {
            id: id.clone(),
            executor: executor.clone(),
            requester: requester.clone(),
            scope: scope.clone(),
            module_id: module_id.clone(),
            result: result.clone(),
            timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize and sign the receipt
        let data_to_sign = serde_json::to_vec(&temp_receipt)
            .map_err(|e| RuntimeError::Wasm(format!("Failed to serialize receipt: {}", e)))?;
            
        // Create signature
        let public_key = ed25519_dalek::PublicKey::from(private_key);
        
        let keypair = ed25519_dalek::Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            id,
            executor,
            requester,
            scope,
            module_id,
            result,
            timestamp,
            signature,
        })
    }
    
    /// Convert to an ICN Receipt
    pub fn to_icn_receipt(&self) -> Receipt {
        let mut resource_usage = ResourceUsage {
            id: Uuid::new_v4().to_string(),
            consumer: self.executor.clone(),
            allocation_id: "".to_string(), // Would be set in a real implementation
            scope: self.scope.clone(),
            timestamp: self.timestamp,
            resources: self.result.resource_usage.clone(),
            context: Some(format!("Module execution: {}", self.module_id)),
            metadata: None,
        };
        
        Receipt {
            id: self.id.clone(),
            issuer: self.executor.clone(),
            requester: self.requester.clone(),
            scope: self.scope.clone(),
            timestamp: self.timestamp,
            resource_usage,
            success: self.result.success,
            result: self.result.return_value.clone(),
            anchor: None,
            signature: self.signature.clone(),
        }
    }
}

/// Interface for WebAssembly executors
#[async_trait]
pub trait Executor: Send + Sync + 'static {
    /// Load a module from WebAssembly bytes
    async fn load_module(&self, wasm_bytes: &[u8], module_id: &str) -> Result<(), RuntimeError>;
    
    /// Execute a loaded module
    async fn execute_module(
        &self,
        module_id: &str,
        function_name: &str,
        params: &[serde_json::Value],
        requester: &str,
        credentials: Vec<Credential>,
    ) -> Result<ExecutionReceipt, RuntimeError>;
    
    /// Check if a module is loaded
    async fn has_module(&self, module_id: &str) -> bool;
    
    /// Unload a module
    async fn unload_module(&self, module_id: &str) -> Result<(), RuntimeError>;
}

/// WebAssembly executor implementation using Wasmtime
pub struct WasmExecutor {
    /// Runtime configuration
    config: RuntimeConfig,
    
    /// WebAssembly engine
    engine: Engine,
    
    /// Loaded modules
    modules: Arc<Mutex<HashMap<String, Module>>>,
    
    /// Identity executing the modules
    identity: ScopedIdentity,
    
    /// Scope for execution
    scope: String,
    
    /// Private key for signing receipts
    private_key: ed25519_dalek::SecretKey,
}

impl WasmExecutor {
    /// Create a new WebAssembly executor
    pub fn new(
        config: RuntimeConfig,
        identity: ScopedIdentity,
        scope: String,
        private_key: ed25519_dalek::SecretKey,
    ) -> Result<Self, RuntimeError> {
        // Create Wasmtime configuration
        let mut wasm_config = WasmConfig::new();
        
        // Apply resource limits
        config.resource_limits.apply_to_config(&mut wasm_config);
        
        // Set compilation threads if specified
        if let Some(threads) = config.compilation_threads {
            wasm_config.parallel_compilation(threads > 1);
        }
        
        // Create engine
        let engine = Engine::new(&wasm_config)
            .map_err(|e| RuntimeError::Wasm(format!("Failed to create WebAssembly engine: {}", e)))?;
            
        Ok(Self {
            config,
            engine,
            modules: Arc::new(Mutex::new(HashMap::new())),
            identity,
            scope,
            private_key,
        })
    }
    
    /// Register a linear memory in the store
    fn register_memory(&self, linker: &mut Linker<HostState>) -> Result<(), RuntimeError> {
        // No actual implementation needed for now
        Ok(())
    }
    
    /// Setup WASI if enabled
    fn setup_wasi(
        &self,
        builder: &mut WasiCtxBuilder,
        wasi_config: &WasiConfig,
    ) -> Result<(), RuntimeError> {
        // Set arguments
        builder.args(&wasi_config.args);
        
        // Set environment variables
        for env in &wasi_config.allowed_env_vars {
            if let Ok(value) = std::env::var(env) {
                builder.env(env, &value);
            }
        }
        
        // Set directories
        for (name, path) in &wasi_config.allowed_dirs {
            builder.preopened_dir(path, name)
                .map_err(|e| RuntimeError::Wasm(format!("Failed to preopen directory: {}", e)))?;
        }
        
        // Set stdin/stdout/stderr
        if wasi_config.inherit_stdin {
            builder.inherit_stdin();
        }
        
        if wasi_config.inherit_stdout {
            builder.inherit_stdout();
        }
        
        if wasi_config.inherit_stderr {
            builder.inherit_stderr();
        }
        
        Ok(())
    }
    
    /// Validate module imports against allowed imports
    fn validate_imports(&self, module: &Module) -> Result<(), RuntimeError> {
        for import in module.imports() {
            let import_name = format!("{}:{}", import.module(), import.name());
            
            if !self.config.allowed_imports.iter().any(|allowed| {
                // Exact match
                *allowed == import_name ||
                // Module-level match (all imports from this module are allowed)
                *allowed == import.module()
            }) {
                return Err(RuntimeError::WasmValidation(
                    format!("Import not allowed: {}", import_name)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Setup the execution environment
    fn setup_execution(
        &self,
        module_id: &str,
    ) -> Result<(Store<HostState>, Linker<HostState>, Module), RuntimeError> {
        // Get the module
        let modules = self.modules.lock().unwrap();
        let module = modules.get(module_id)
            .ok_or_else(|| RuntimeError::InvalidParameter(
                format!("Module {} not found", module_id)
            ))?
            .clone();
            
        // Create resource limits for execution
        let mut resource_limits = HashMap::new();
        resource_limits.insert(ResourceType::ComputeTime, self.config.resource_limits.max_instructions);
        resource_limits.insert(ResourceType::Memory, self.config.resource_limits.max_memory);
        resource_limits.insert(ResourceType::Operations, 1000); // Arbitrary limit for now
        
        // Create resource meter
        let resource_meter = Arc::new(ResourceMeter::new(
            resource_limits,
            self.identity.id().to_string(),
            self.scope.clone(),
        ));
        
        // Create host state
        let host_state = HostState::new(
            resource_meter,
            Vec::new(), // Credentials will be added later
        );
        
        // Create store with resource limits
        let store_limits = self.config.resource_limits.create_store_limits();
        let mut store = Store::new(&self.engine, host_state);
        
        // Allocate fuel for execution
        store.add_fuel(self.config.resource_limits.max_instructions)
            .map_err(|e| RuntimeError::Wasm(format!("Failed to add fuel to store: {}", e)))?;
            
        // Create linker
        let mut linker = Linker::new(&self.engine);
        
        // Register memory
        self.register_memory(&mut linker)?;
        
        // Setup WASI if enabled
        if self.config.enable_wasi {
            let mut wasi_builder = WasiCtxBuilder::new();
            self.setup_wasi(&mut wasi_builder, &self.config.wasi_config)?;
            
            let wasi_ctx = wasi_builder.build();
            wasmtime_wasi::add_to_linker(&mut linker, |host| host)
                .map_err(|e| RuntimeError::Wasm(format!("Failed to add WASI to linker: {}", e)))?;
        }
        
        // Setup ICN host functions
        register_host_functions(&mut store)?;
        
        Ok((store, linker, module))
    }
    
    /// Parse parameters for a function call
    fn parse_parameters(
        &self,
        module: &Module,
        function_name: &str,
        params: &[serde_json::Value],
    ) -> Result<Vec<wasmtime::Val>, RuntimeError> {
        // For now, just return an empty vector of parameters
        // In a real implementation, we would parse the parameters based on the function signature
        Ok(Vec::new())
    }
    
    /// Handle execution result
    fn handle_execution_result(
        &self,
        result: Result<Box<[wasmtime::Val]>, Trap>,
        execution_time: Duration,
        resource_usage: ResourceUsageCollector,
    ) -> ExecutionResult {
        match result {
            Ok(values) => {
                // Parse return values
                let return_value = match values.len() {
                    0 => None,
                    _ => {
                        // In a real implementation, we would convert the return values to JSON
                        // For now, just return a dummy value
                        Some(serde_json::json!({
                            "result": "success",
                            "values": values.len()
                        }))
                    }
                };
                
                ExecutionResult {
                    success: true,
                    return_value,
                    execution_time_ms: execution_time.as_millis() as u64,
                    resource_usage: resource_usage.resources,
                    error_message: None,
                    exit_code: None,
                }
            }
            Err(trap) => {
                let error_message = trap.to_string();
                let exit_code = if error_message.contains("exit") {
                    // Try to extract exit code from the message
                    error_message.chars()
                        .filter(|c| c.is_digit(10))
                        .collect::<String>()
                        .parse::<i32>()
                        .ok()
                } else {
                    None
                };
                
                ExecutionResult {
                    success: false,
                    return_value: None,
                    execution_time_ms: execution_time.as_millis() as u64,
                    resource_usage: resource_usage.resources,
                    error_message: Some(error_message),
                    exit_code,
                }
            }
        }
    }
}

#[async_trait]
impl Executor for WasmExecutor {
    async fn load_module(&self, wasm_bytes: &[u8], module_id: &str) -> Result<(), RuntimeError> {
        // Compile the module
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| RuntimeError::WasmCompilation(format!("Failed to compile module: {}", e)))?;
            
        // Validate imports
        self.validate_imports(&module)?;
        
        // Store the module
        let mut modules = self.modules.lock().unwrap();
        modules.insert(module_id.to_string(), module);
        
        Ok(())
    }
    
    async fn execute_module(
        &self,
        module_id: &str,
        function_name: &str,
        params: &[serde_json::Value],
        requester: &str,
        credentials: Vec<Credential>,
    ) -> Result<ExecutionReceipt, RuntimeError> {
        // Setup the execution environment
        let (mut store, mut linker, module) = self.setup_execution(module_id)?;
        
        // Add credentials to the store
        store.data_mut().credentials = credentials;
        
        // Get the resource meter
        let resource_meter = store.data().resource_meter.clone();
        
        // Parse parameters
        let params = self.parse_parameters(&module, function_name, params)?;
        
        // Instantiate the module
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| RuntimeError::WasmInstantiation(format!("Failed to instantiate module: {}", e)))?;
            
        // Get the function
        let func = instance.get_func(&mut store, function_name)
            .ok_or_else(|| RuntimeError::InvalidParameter(
                format!("Function {} not found in module", function_name)
            ))?;
            
        // Start measurement
        let start_time = Instant::now();
        
        // Execute the function
        let result = func.call(&mut store, &params, &mut []);
        
        // End measurement
        let execution_time = start_time.elapsed();
        
        // Get resource usage
        let resource_usage = resource_meter.get_usage();
        
        // Handle result
        let execution_result = self.handle_execution_result(
            result,
            execution_time,
            resource_usage,
        );
        
        // Create receipt
        let receipt = ExecutionReceipt::new(
            self.identity.clone(),
            requester.to_string(),
            self.scope.clone(),
            module_id.to_string(),
            execution_result,
            &self.private_key,
        )?;
        
        Ok(receipt)
    }
    
    async fn has_module(&self, module_id: &str) -> bool {
        let modules = self.modules.lock().unwrap();
        modules.contains_key(module_id)
    }
    
    async fn unload_module(&self, module_id: &str) -> Result<(), RuntimeError> {
        let mut modules = self.modules.lock().unwrap();
        
        if modules.remove(module_id).is_none() {
            return Err(RuntimeError::InvalidParameter(
                format!("Module {} not found", module_id)
            ));
        }
        
        Ok(())
    }
} 