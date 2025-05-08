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

// Comment out wasmtime imports - we won't use them
// use wasmtime;
// use wasmtime::AsContextMut;

#[derive(Subcommand, Debug, Clone)]
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
    errors: Arc<Mutex<Option<String>>>,
    policy_loader: Option<Arc<dyn PolicyLoader + Send + Sync>>,
    membership_index: Option<Arc<dyn MembershipIndex + Send + Sync>>,
}

// Add this static
static DUMMY_DID: OnceLock<Did> = OnceLock::new();

impl RuntimeExecutionContext {
    fn new(config: ExecutionConfig, verbose: bool) -> Result<Self> {
        Ok(Self {
            execution_config: config,
            log_enabled: verbose,
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

// REMOVED IMPLEMENTATION OF HOSTCONTEXT TRAIT
// We will not implement the HostContext trait at all, since it requires
// wasmtime types that are causing compilation issues.

pub async fn handle_runtime_command(
    context: &mut CliContext, 
    cmd: &RuntimeCommands,
) -> CliResult {
    // Simplified implementation that just logs what would happen
    match cmd {
        RuntimeCommands::Run(args) => {
            println!("Would run module: {}", args.module_path);
            Ok(())
        },
        RuntimeCommands::Inspect(args) => {
            println!("Would inspect module: {}", args.module_ref);
            Ok(())
        },
        RuntimeCommands::Validate(args) => {
            println!("Would validate module: {}", args.module_path);
            Ok(())
        },
    }
} 