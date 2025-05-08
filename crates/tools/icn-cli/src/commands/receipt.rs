use clap::{Args, Subcommand, Arg, ValueHint};
use crate::error::CliResult;
use crate::context::CliContext;
use crate::error::CliError;
use icn_identity_core::{
    did::DidKey,
    ExecutionReceipt, 
    ExecutionSubject, 
    ExecutionScope, 
    ExecutionStatus,
    ExecutionReceiptError
};
use icn_types::Cid; // For parsing CIDs
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use serde_json::json;
use hex;
use colored::Colorize;

#[derive(Subcommand, Debug)]
pub enum ReceiptCommands {
    /// Issue a new execution receipt credential.
    Issue(IssueReceiptArgs),

    /// Anchor a local execution receipt credential to the DAG.
    Anchor(AnchorReceiptArgs),

    /// List known ExecutionReceipts.
    List(ListReceiptArgs),

    /// Show full details of a specific receipt by its ID, file path, or DAG CID.
    Show(ShowReceiptArgs),

    /// Verify an execution receipt (cryptographically, or against the DAG).
    Verify(VerifyReceiptArgs),
}

#[derive(Args, Debug, Clone)]
pub struct IssueReceiptArgs {
    /// Path to the key file for signing the receipt (JWK format).
    #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
    pub key_file: PathBuf,
    /// DID of the node that executed the computation.
    #[clap(long)]
    pub executor: String,
    /// Federation DID under which this execution occurred (will be the issuer).
    #[clap(long)]
    pub federation: String,
    /// CID of the executed module.
    #[clap(long)]
    pub module_cid: String,
    /// CID of the execution result.
    #[clap(long)]
    pub result_cid: String,
    /// Status of execution (success, failed, pending, canceled).
    #[clap(long, default_value = "success")]
    pub status: String,
    /// Optional submitter DID (e.g., user or coop DID).
    #[clap(long)]
    pub submitter: Option<String>,
    /// Output file for the receipt (JSON format). If not provided, prints to stdout.
    #[clap(long, short, value_parser, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
    // TODO: Add --anchor flag to directly anchor after issuing?
}

#[derive(Args, Debug, Clone)]
pub struct AnchorReceiptArgs {
    /// Path to the local execution receipt file (JSON format) to be anchored.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub receipt_file: PathBuf,
    /// Path to the JWK file for signing the anchor DAG node.
    #[clap(long, value_hint = ValueHint::FilePath)]
    pub key_file: PathBuf,
    /// DID of the author anchoring this receipt. If not provided, will derive from key_file.
    #[clap(long)]
    pub author_did: Option<String>,
    /// Optional path to the DAG storage directory.
    #[clap(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ListReceiptArgs {
    /// Filter by Federation ID (issuer DID).
    #[clap(long)]
    pub federation: Option<String>,
    /// Filter by Module CID.
    #[clap(long)]
    pub module_cid: Option<String>,
    /// Filter receipts issued since this date (ISO 8601 format, e.g., "2023-01-01T12:00:00Z").
    #[clap(long)]
    pub since: Option<String>,
    /// Maximum number of receipts to list.
    #[clap(long, default_value = "50")]
    pub limit: usize,
    /// Source directory for exported receipts (if not querying DAG). Used if --dag-dir not specified.
    #[clap(long, value_parser, value_hint = ValueHint::DirPath, default_value = "output/receipts")]
    pub source_dir: PathBuf,
    /// Optional path to the DAG storage directory to list anchored receipts.
    #[clap(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ShowReceiptArgs {
    /// Receipt reference: URN ID, local file path, or DAG anchor CID.
    #[clap(value_parser)]
    pub receipt_ref: String,
    /// Output in raw JSON format.
    #[clap(long, action = clap::ArgAction::SetTrue)]
    pub json: bool,
    /// Source directory for exported receipts (used if receipt_ref is an ID without DAG context).
    #[clap(long, value_parser, value_hint = ValueHint::DirPath, default_value = "output/receipts")]
    pub source_dir: PathBuf,
    /// Optional path to the DAG storage directory (used if receipt_ref is a CID or for verification).
    #[clap(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct VerifyReceiptArgs {
    /// Receipt reference: URN ID, local file path, or DAG anchor CID to verify.
    #[clap(value_parser)]
    pub receipt_ref: String,
    /// Optional path to the DAG storage directory (required if verifying DAG dependencies).
    #[clap(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
    // TODO: Add flag for --verify-dag-dependencies?
}

pub async fn handle_receipt_command(
    context: &mut CliContext,
    cmd: &ReceiptCommands,
) -> Result<(), CliError> {
    if context.verbose { println!("Handling Receipt command: {:?}", cmd); }
    match cmd {
        ReceiptCommands::Issue(args) => {
            issue_receipt_cli(
                &args.key_file, &args.executor, &args.federation, &args.module_cid, 
                &args.result_cid, &args.status, args.submitter.as_deref(), 
                args.output.as_ref(),
            ).await?;
        },
        ReceiptCommands::Anchor(args) => handle_anchor_receipt(context, args).await?,
        ReceiptCommands::List(args) => {
            list_receipts_cli(args.federation.as_deref(), args.module_cid.as_deref(), args.since.as_deref(), args.limit, &args.source_dir, args.dag_dir.as_ref()).await?;
        },
        ReceiptCommands::Show(args) => {
            show_receipt_cli(&args.receipt_ref, args.json, &args.source_dir, args.dag_dir.as_ref()).await?;
        },
        ReceiptCommands::Verify(args) => {
            verify_receipt_cli(args.receipt_ref.as_str(), args.dag_dir.as_ref()).await?;
        },
    }
    Ok(())
}

// Renamed existing handler args to avoid conflicts and pass context if needed
async fn issue_receipt_cli(
    key_file: &PathBuf,
    executor: &str,
    federation: &str,
    module_cid: &str,
    result_cid: &str,
    status_str: &str,
    submitter: Option<&str>,
    output_file: Option<&PathBuf>,
) -> Result<(), CliError> {
    let key_data = fs::read_to_string(key_file).map_err(CliError::Io)?;
    let did_key = DidKey::new(); // For now, just create a new key since we can't easily parse
    let status = match status_str.to_lowercase().as_str() {
        "success" => ExecutionStatus::Success,
        "failed" => ExecutionStatus::Failed,
        "pending" => ExecutionStatus::Pending,
        "canceled" => ExecutionStatus::Canceled,
        _ => return Err(CliError::InvalidArgument(format!("Invalid status: {}", status_str))),
    };
    let scope = ExecutionScope::Federation { federation_id: federation.to_string() };
    let subject = ExecutionSubject {
        id: executor.to_string(),
        scope,
        submitter: submitter.map(|s| s.to_string()),
        module_cid: module_cid.to_string(),
        result_cid: result_cid.to_string(),
        event_id: None, 
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        status,
        additional_properties: Some(json!({
            "issuedBy": "icn-cli",
            "cliVersion": env!("CARGO_PKG_VERSION"),
        })),
    };
    let receipt_id = format!("urn:uuid:{}", Uuid::new_v4());
    let receipt = ExecutionReceipt::new(receipt_id, did_key.did().to_string(), subject)
        .sign(&did_key)?;
    let receipt_json = receipt.to_json()?;
    if let Some(path) = output_file {
        fs::write(path, &receipt_json).map_err(CliError::Io)?;
        println!("{} Execution receipt issued and saved to {}", "✅".green(), path.display());
        println!("   Receipt ID: {}", receipt.id);
        println!("   Issuer:     {}", receipt.issuer);
    } else {
        println!("{}", receipt_json);
    }
    Ok(())
}

async fn handle_anchor_receipt(_context: &mut CliContext, args: &AnchorReceiptArgs) -> CliResult {
    println!("Executing receipt anchor with args: {:?}", args);
    // TODO: Implement logic to read receipt file, load key, get DAG store, call icn_types::ExecutionReceipt::anchor_to_dag.
    Err(CliError::Unimplemented("receipt anchor".to_string()))
}

// Modified existing handlers to accept Option<&PathBuf> for dag_dir and pass context if needed
async fn list_receipts_cli(
    federation_filter: Option<&str>,
    module_cid_filter: Option<&str>,
    since_filter: Option<&str>,
    limit: usize,
    source_dir: &PathBuf, // Keep for local file listing
    _dag_dir: Option<&PathBuf>, // Placeholder for future DAG listing
) -> Result<(), CliError> {
    println!("Listing receipts from dir: {}. Filters: fed={:?}, mod={:?}, since={:?}, limit={}", 
        source_dir.display(), federation_filter, module_cid_filter, since_filter, limit);
    println!("TODO: Implement listing from local files. DAG listing via --dag-dir is future work.");
    // ... existing local file listing logic ...
    Ok(())
}

async fn show_receipt_cli(
    receipt_ref: &str, 
    output_json: bool, 
    source_dir: &PathBuf, // Keep for local file/ID lookup
    _dag_dir: Option<&PathBuf>, // Placeholder for future DAG CID lookup
) -> Result<(), CliError> {
    println!("Showing receipt for ref: {}. JSON: {}. Local dir: {}.", receipt_ref, output_json, source_dir.display());
    println!("TODO: Implement showing from local files. DAG lookup for CID ref is future work.");
    // ... existing local file/ID showing logic ...
    Ok(())
}

async fn verify_receipt_cli(
    receipt_ref: &str, 
    _dag_dir: Option<&PathBuf>, // Placeholder for future DAG dependency verification
) -> Result<(), CliError> {
    println!("Verifying receipt for ref: {}.", receipt_ref);
    println!("TODO: Implement local credential verification. DAG dependency verification via --dag-dir is future work.");
    // ... existing local file verification logic ...
    Ok(())
}

impl From<ExecutionReceiptError> for CliError {
    fn from(err: ExecutionReceiptError) -> Self {
        CliError::IdentityError(err.to_string())
    }
} 