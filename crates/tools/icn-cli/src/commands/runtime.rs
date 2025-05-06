use clap::{Arg, Args, Subcommand, ArgMatches, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_runtime::config::ExecutionConfig;
use std::path::PathBuf;

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

pub async fn handle_runtime_command(
    context: &mut CliContext, 
    cmd: &RuntimeCommands,
) -> CliResult {
    if context.verbose { println!("Handling Runtime command: {:?}", cmd); }
    match cmd {
        RuntimeCommands::Run(args) => {
            println!("Executing runtime command: Run");
            println!("  Module Path: {}", args.module_path);
            if let Some(cid) = &args.module_cid { println!("  Module CID: {}", cid); }
            if let Some(path) = &args.input_data_path { println!("  Input Data Path: {}", path); }
            if let Some(id) = &args.trigger_event_id { println!("  Trigger Event ID: {}", id); }

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
            } else if args.no_auto_receipt {
                execution_config.receipt_export_dir = None;
            }

            println!("  Effective ExecutionConfig for receipts:");
            println!("    Auto-issue:     {}", execution_config.auto_issue_receipts);
            println!("    Anchor receipts:  {}", execution_config.anchor_receipts);
            if let Some(dir) = &execution_config.receipt_export_dir {
                println!("    Export directory: {}", dir.display());
            } else {
                println!("    Export directory: None");
            }
            println!("Runtime execution logic (placeholder)... Arguments processed.");
            // TODO: Implement actual Wasm execution
            Ok(())
        }
        RuntimeCommands::Inspect(args) => handle_inspect_module(context, args).await,
        RuntimeCommands::Validate(args) => handle_validate_module(context, args).await,
    }
}

// Placeholder handlers
async fn handle_inspect_module(_context: &mut CliContext, args: &InspectModuleArgs) -> CliResult {
    println!("Executing runtime inspect for module: {}, extended: {}", args.module_ref, args.extended);
    // TODO: Implement Wasm module inspection (e.g., using wasmparser or similar)
    //       to show imports, exports, custom sections (like ICN manifest).
    Err(CliError::Unimplemented("runtime inspect".to_string()))
}

async fn handle_validate_module(_context: &mut CliContext, args: &ValidateModuleArgs) -> CliResult {
    println!("Executing runtime validate for module: {}, policy: {:?}", args.module_path, args.policy_file);
    // TODO: Implement Wasm module validation against ICN rules (e.g., allowed imports, resource limits)
    //       and optional policy file.
    Err(CliError::Unimplemented("runtime validate".to_string()))
} 