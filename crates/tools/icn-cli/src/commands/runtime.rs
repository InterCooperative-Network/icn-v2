use clap::{Arg, Args, Subcommand, ArgMatches, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_runtime::{
    abi::context::HostContext,
    policy::{MembershipIndex, PolicyLoader},
    config::ExecutionConfig,
    ModernWasmExecutor,
    ContextExtension,
};
use icn_types::{Cid, Did, dag::{EventId, DagStore}};
use std::path::{PathBuf, Path};
use std::sync::Arc;
use std::str::FromStr;
use anyhow::Context as AnyhowContext;
use async_trait::async_trait;
use std::sync::OnceLock;
use std::sync::Mutex;
use anyhow::Result;
use wasmtime;

#[derive(Subcommand, Debug)]
pub enum RuntimeCommands {
    /// Execute a WASM module in the ICN runtime.
    Run(RunModuleArgs),
    /// Inspect a WASM module for metadata, imports/exports.
    Inspect(InspectModuleArgs),
    /// Validate a WASM module against ICN runtime expectations.
    Validate(ValidateModuleArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RunModuleArgs {
    /// Path to the WASM module file.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub module_path: String,
    /// Optional CID of the module (if pre-registered or known).
    #[clap(long)]
    pub module_cid: Option<String>,
    /// Optional path to a file containing input data for the execution.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub input_data_path: Option<String>,
    /// Optional EventID that triggers this execution (for DAG linking).
    #[clap(long)]
    pub trigger_event_id: Option<String>,
    // --- Receipt Control Flags ---
    /// Disable automatic ExecutionReceipt issuance.
    #[clap(long, action = clap::ArgAction::SetTrue)]
    pub no_auto_receipt: bool,
    /// Issue receipt but do not anchor it to the DAG.
    #[clap(long, action = clap::ArgAction::SetTrue)]
    pub no_anchor_receipt: bool,
    /// Directory to export issued receipts as JSON (overrides default).
    #[clap(long, value_parser, value_hint = ValueHint::DirPath)]
    pub receipt_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct InspectModuleArgs {
    /// Path to the WASM module file or its CID.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub module_ref: String, 
    /// Show extended details (e.g., ICN-specific manifest sections if present).
    #[clap(long, action = clap::ArgAction::SetTrue)]
    pub extended: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ValidateModuleArgs {
    /// Path to the WASM module file to validate.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub module_path: String,
    /// Optional path to a schema or policy file for validation.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub policy_file: Option<PathBuf>,
}

#[derive(Clone)]
struct RuntimeExecutionContext {
    execution_config: ExecutionConfig,
    log_enabled: bool,
    memory: wasmtime::Memory,
    errors: Arc<Mutex<Option<String>>>,
    policy_loader: Option<Arc<dyn PolicyLoader + Send + Sync>>,
    membership_index: Option<Arc<dyn MembershipIndex + Send + Sync>>,
}

// Add this static
static DUMMY_DID: OnceLock<Did> = OnceLock::new();

impl RuntimeExecutionContext {
    fn new(config: ExecutionConfig, verbose: bool) -> Result<Self> {
        let mut store = wasmtime::Store::default();
        let memory = wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None))?;
        
        Ok(Self {
            execution_config: config,
            log_enabled: verbose,
            memory,
            errors: Arc::new(Mutex::new(None)),
            policy_loader: None,
            membership_index: None,
        })
    }
}

impl ContextExtension for RuntimeExecutionContext {
    fn get_execution_config(&self) -> &ExecutionConfig {
        &self.execution_config
    }
    
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn DagStore + Send + Sync)> {
        None // We don't have a DAG store in this simple implementation
    }
    
    fn node_did(&self) -> Option<&Did> {
        None // We don't have a node DID in this simple implementation
    }
    
    fn federation_did(&self) -> Option<&Did> {
        None // We don't have a federation DID in this simple implementation
    }
    
    fn caller_did(&self) -> Option<&Did> {
        // Initialize the static DID once
        let did = DUMMY_DID.get_or_init(|| {
            Did::from_string("did:icn:placeholder").unwrap_or_else(|_| Did::from(String::from("did:icn:placeholder")))
        });
        Some(did)
    }
}

impl HostContext for RuntimeExecutionContext {
    fn read_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, len: i32) -> Result<String> {
        // Read string from WebAssembly memory
        let memory = self.memory.get(caller)?;
        let data = memory.data(caller);
        let str_data = &data[ptr as usize..(ptr + len) as usize];
        String::from_utf8(str_data.to_vec()).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))
    }

    fn write_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, max_len: i32, s: &str) -> Result<i32> {
        // Write string to WebAssembly memory
        let memory = self.memory.get(caller)?;
        let data = memory.data_mut(caller);
        let bytes = s.as_bytes();
        if bytes.len() > max_len as usize {
            return Err(anyhow::anyhow!("String too long"));
        }
        data[ptr as usize..ptr as usize + bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len() as i32)
    }

    fn malloc(&self, caller: &mut impl wasmtime::AsContextMut, size: i32) -> Result<i32> {
        // Allocate memory in WebAssembly
        let memory = self.memory.get(caller)?;
        let data = memory.data_mut(caller);
        let ptr = data.len();
        data.extend(vec![0; size as usize]);
        Ok(ptr as i32)
    }

    fn free(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32) -> Result<()> {
        // Free memory in WebAssembly (no-op in this simple implementation)
        Ok(())
    }

    fn get_caller_did(&self) -> Did {
        // Return the dummy DID
        DUMMY_DID.get_or_init(|| {
            Did::from_string("did:icn:placeholder").unwrap_or_else(|_| Did::from(String::from("did:icn:placeholder")))
        }).clone()
    }

    fn log_message(&self, message: &str) {
        if self.log_enabled {
            println!("[WASM] {}", message);
        }
    }

    async fn verify_signature(&self, _did: &Did, _message: &[u8], _signature: &[u8]) -> bool {
        // Simple implementation always returns true
        true
    }

    fn set_error(&self, error: String) {
        if let Ok(mut errors) = self.errors.lock() {
            *errors = Some(error);
        }
    }

    fn get_error(&self) -> Option<String> {
        self.errors.lock().ok().and_then(|errors| errors.clone())
    }

    fn clear_error(&self) {
        if let Ok(mut errors) = self.errors.lock() {
            *errors = None;
        }
    }

    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>> {
        self.policy_loader.clone()
    }

    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>> {
        self.membership_index.clone()
    }
}

pub async fn handle_runtime_command(
    context: &mut CliContext, 
    cmd: &RuntimeCommands,
) -> CliResult {
    if context.verbose { println!("Handling Runtime command: {:?}", cmd); }
    match cmd {
        RuntimeCommands::Run(args) => handle_run_module(context, args).await,
        RuntimeCommands::Inspect(args) => handle_inspect_module(context, args).await,
        RuntimeCommands::Validate(args) => handle_validate_module(context, args).await,
    }
}

async fn handle_run_module(context: &mut CliContext, args: &RunModuleArgs) -> CliResult {
    println!("Executing WASM module: {}", args.module_path);
    
    // Create execution configuration
    let mut execution_config = ExecutionConfig::default();
    if args.no_auto_receipt {
        execution_config.auto_issue_receipts = false;
        execution_config.anchor_receipts = false; 
    }
    
    if args.no_anchor_receipt {
        execution_config.anchor_receipts = false;
    }
    
    if let Some(dir) = &args.receipt_dir {
        execution_config.receipt_export_dir = Some(dir.clone());
    }
    
    // Create runtime context with our execution config
    let runtime_ctx = RuntimeExecutionContext::new(execution_config, context.verbose)?;
    let runtime_ctx = Arc::new(runtime_ctx);
    
    // Create the executor
    let executor = ModernWasmExecutor::new()
        .map_err(|e| CliError::Other(format!("Failed to create WASM executor: {}", e).into()))?;
    
    // Load the WASM module
    let wasm_bytes = executor.load_module_from_file(&args.module_path)
        .map_err(|e| CliError::Other(format!("Failed to load WASM module: {}", e).into()))?;
    
    println!("Module loaded: {} bytes", wasm_bytes.len());
    
    // Generate or use the provided module CID
    let module_cid = if let Some(cid_str) = &args.module_cid {
        Cid::from_str(cid_str)
            .map_err(|_| CliError::InvalidArgument(format!("Invalid CID format: {}", cid_str)))?
    } else {
        // Generate a CID from the module bytes
        // This is a simplified approach - real implementations would use proper IPLD hashing
        Cid::from_bytes(&[1u8; 32])
            .map_err(|_| CliError::Other("Failed to generate CID".into()))?
    };
    
    // Parse event ID if provided
    let event_id = if let Some(event_id_str) = &args.trigger_event_id {
        let event_id_bytes = hex::decode(event_id_str)
            .map_err(|_| CliError::InvalidArgument(format!("Invalid event ID format: {}", event_id_str)))?;
        
        if event_id_bytes.len() != 32 {
            return Err(CliError::InvalidArgument(format!(
                "Event ID must be 32 bytes, got {} bytes", event_id_bytes.len()
            )));
        }
        
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&event_id_bytes);
        Some(EventId(arr))
    } else {
        None
    };
    
    // Read input data if provided
    let input_data = if let Some(input_path) = &args.input_data_path {
        Some(std::fs::read(input_path)
            .map_err(|e| CliError::Io(e))?)
    } else {
        None
    };
    
    // Set fuel limit (a reasonable default for CLI execution)
    let fuel_limit = Some(10_000_000);
    
    println!("Executing module...");
    
    // Execute the module
    let result = executor.execute(
        &wasm_bytes,
        runtime_ctx,
        module_cid.clone(),
        event_id,
        input_data.as_deref(),
        fuel_limit
    ).await
    .map_err(|e| CliError::Other(format!("Module execution failed: {}", e).into()))?;
    
    // Print execution results
    println!("\nExecution complete!");
    println!("  Module CID: {}", result.module_cid);
    println!("  Execution time: {} ms", result.execution_time_ms);
    
    if let Some(fuel) = result.fuel_consumed {
        println!("  Fuel consumed: {}", fuel);
    }
    
    println!("  Result CID: {}", result.result_cid);
    
    Ok(())
}

// Placeholder handlers
async fn handle_inspect_module(_context: &mut CliContext, args: &InspectModuleArgs) -> CliResult {
    println!("Executing runtime inspect for module: {}, extended: {}", args.module_ref, args.extended);
    
    // Create the executor
    let executor = ModernWasmExecutor::new()
        .map_err(|e| CliError::Other(format!("Failed to create WASM executor: {}", e).into()))?;
    
    // Check if the reference is a file path or CID
    if Path::new(&args.module_ref).exists() {
        // Load the WASM module from file
        let wasm_bytes = executor.load_module_from_file(&args.module_ref)
            .map_err(|e| CliError::Other(format!("Failed to load WASM module: {}", e).into()))?;
        
        println!("\nModule information:");
        println!("  Size: {} bytes", wasm_bytes.len());
        
        // Simple validation check
        match executor.validate_module(&wasm_bytes) {
            Ok(true) => println!("  Validation: Passed basic validation"),
            Ok(false) => println!("  Validation: Failed, but no specific errors reported"),
            Err(e) => println!("  Validation: Failed with error: {}", e),
        }
    } else {
        return Err(CliError::NotFound(format!("Module not found at path: {}", args.module_ref)));
    }
    
    println!("\nDetailed inspection of WASM modules is not yet implemented.");
    println!("Future versions will show imports, exports, and custom sections.");
    
    Ok(())
}

async fn handle_validate_module(_context: &mut CliContext, args: &ValidateModuleArgs) -> CliResult {
    println!("Executing runtime validate for module: {}, policy: {:?}", args.module_path, args.policy_file);
    
    // Create the executor
    let executor = ModernWasmExecutor::new()
        .map_err(|e| CliError::Other(format!("Failed to create WASM executor: {}", e).into()))?;
    
    // Load the WASM module
    let wasm_bytes = executor.load_module_from_file(&args.module_path)
        .map_err(|e| CliError::Other(format!("Failed to load WASM module: {}", e).into()))?;
    
    println!("Module loaded: {} bytes", wasm_bytes.len());
    
    // Validate the module
    match executor.validate_module(&wasm_bytes) {
        Ok(true) => {
            println!("✅ Module validation successful!");
            println!("The module meets the basic requirements for ICN execution.");
        },
        Ok(false) => {
            println!("⚠️ Module validation failed without specific errors.");
            println!("The module may not be suitable for ICN execution.");
        },
        Err(e) => {
            println!("❌ Module validation failed: {}", e);
            println!("The module cannot be executed in the ICN runtime.");
            return Err(CliError::Other(format!("Module validation failed: {}", e).into()));
        }
    }
    
    println!("\nFull validation against ICN policies is not yet implemented.");
    println!("Future versions will check for compliance with security and resource limits.");
    
    Ok(())
} 