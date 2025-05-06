use clap::{Subcommand, Arg, ValueHint};
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
    /// Issue a new execution receipt
    Issue {
        /// Path to the key file for signing the receipt (JWK format)
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        key_file: PathBuf,

        /// DID of the node that executed the computation
        #[clap(long)]
        executor: String,

        /// Federation DID under which this execution occurred (will be the issuer)
        #[clap(long)]
        federation: String,

        /// CID of the executed module
        #[clap(long)]
        module_cid: String,

        /// CID of the execution result
        #[clap(long)]
        result_cid: String,

        /// Status of execution (success, failed, pending, canceled)
        #[clap(long, default_value = "success")]
        status: String,

        /// Optional submitter DID (e.g., user or coop DID)
        #[clap(long)]
        submitter: Option<String>,

        /// Output file for the receipt (JSON format). If not provided, prints to stdout.
        #[clap(long, short, value_parser, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },

    /// List known ExecutionReceipts
    List {
        /// Filter by Federation ID (issuer DID)
        #[clap(long)]
        federation: Option<String>,

        /// Filter by Module CID
        #[clap(long)]
        module_cid: Option<String>,

        /// Filter receipts issued since this date (ISO 8601 format, e.g., "2023-01-01T12:00:00Z")
        #[clap(long)]
        since: Option<String>,

        /// Maximum number of receipts to list
        #[clap(long, default_value = "50")]
        limit: usize,

        /// Source directory for exported receipts (if not querying DAG)
        #[clap(long, value_parser, value_hint = ValueHint::DirPath, default_value = "output/receipts")]
        source_dir: PathBuf,
    },

    /// Show full details of a specific receipt by its ID or file path
    Show {
        /// Receipt ID (URN or CID) or path to a local receipt JSON file
        #[clap(value_parser)]
        receipt_ref: String, // Can be ID or path

        /// Output in raw JSON format
        #[clap(long, action = clap::ArgAction::SetTrue)]
        json: bool,

        /// Source directory for exported receipts (if looking up by ID)
        #[clap(long, value_parser, value_hint = ValueHint::DirPath, default_value = "output/receipts")]
        source_dir: PathBuf,
    },

    /// Verify an execution receipt from a file or stdin
    Verify {
        /// Path to the receipt file (JSON format). If not provided, reads from stdin.
        #[clap(value_parser, value_hint = ValueHint::FilePath)]
        file: Option<PathBuf>,
        // Trusted issuer DIDs could be loaded from a config file or passed as args in future.
    },
}

pub async fn handle_receipt_command(
    _context: &mut CliContext, // CliContext might be needed for DAG store access in List/Show by CID
    cmd: &ReceiptCommands,
) -> Result<(), CliError> {
    match cmd {
        ReceiptCommands::Issue {
            key_file,
            executor,
            federation,
            module_cid,
            result_cid,
            status,
            submitter,
            output,
        } => {
            issue_receipt_cli(
                key_file,
                executor,
                federation,
                module_cid,
                result_cid,
                status,
                submitter.as_deref(),
                output.as_ref(), // Pass Option<&PathBuf>
            ).await?;
        },
        ReceiptCommands::List { federation, module_cid, since, limit, source_dir } => {
            list_receipts_cli(federation.as_deref(), module_cid.as_deref(), since.as_deref(), *limit, source_dir).await?;
        },
        ReceiptCommands::Show { receipt_ref, json, source_dir } => {
            show_receipt_cli(receipt_ref, *json, source_dir).await?;
        },
        ReceiptCommands::Verify { file } => {
            verify_receipt_cli(file.as_ref()).await?;
        },
    }
    Ok(())
}

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
    let did_key = DidKey::from_jwk(&key_data).map_err(|e| CliError::InvalidKey(e.to_string()))?;

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
        .sign(&did_key)?; // Error converted by From trait

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

async fn list_receipts_cli(
    federation_filter: Option<&str>,
    module_cid_filter: Option<&str>,
    since_filter: Option<&str>,
    limit: usize,
    source_dir: &PathBuf,
) -> Result<(), CliError> {
    println!("{} Listing execution receipts...", "ℹ️".dimmed());
    println!("  Source: {}", source_dir.display());
    if let Some(fed) = federation_filter { println!("  Filter Federation: {}", fed); }
    if let Some(mod_cid) = module_cid_filter { println!("  Filter Module CID: {}", mod_cid); }
    if let Some(since) = since_filter { println!("  Filter Since:      {}", since); }
    println!("  Limit:           {}", limit);

    // Placeholder: Actual implementation would:
    // 1. If DAG store is configured and accessible:
    //    - Query DAG for EventType::Receipt.
    //    - Fetch corresponding ExecutionReceipts by their CIDs from payload.
    //    - Apply filters during or after fetching.
    // 2. Else, or as a fallback/alternative:
    //    - Scan the `source_dir` for .json files.
    //    - Deserialize each into ExecutionReceipt.
    //    - Apply filters (federation on issuer, module_cid on subject, date on issuance_date).
    //    - Sort by date and take limit.
    // 3. Print a table.

    if !source_dir.exists() || !source_dir.is_dir() {
        println!("{}", "Warning: Source directory does not exist or is not a directory.".yellow());
        return Ok(());
    }

    let mut receipts_found: Vec<ExecutionReceipt> = Vec::new();
    for entry in fs::read_dir(source_dir).map_err(CliError::Io)? {
        let entry = entry.map_err(CliError::Io)?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(receipt) = ExecutionReceipt::from_json(&content) {
                    receipts_found.push(receipt);
                }
            }
        }
    }

    // Apply filters (basic example)
    let filtered_receipts = receipts_found.into_iter()
        .filter(|r| federation_filter.map_or(true, |f| r.issuer == f))
        .filter(|r| module_cid_filter.map_or(true, |mc| r.credential_subject.module_cid == mc))
        // TODO: Implement 'since' date parsing and filtering
        .take(limit)
        .collect::<Vec<_>>();

    if filtered_receipts.is_empty() {
        println!("No receipts found matching criteria.");
    } else {
        println!("───────────────────────────────────────────────────────────────────────────────────────────────────");
        println!("{:<38} {:<45} {:<15} {:<25}", "ID", "ISSUER", "MODULE CID (Trunc)", "TIMESTAMP");
        println!("───────────────────────────────────────────────────────────────────────────────────────────────────");
        for r in filtered_receipts {
            let mod_cid_trunc = if r.credential_subject.module_cid.len() > 12 {
                format!("{}...", &r.credential_subject.module_cid[..12])
            } else {
                r.credential_subject.module_cid.clone()
            };
            println!("{:<38} {:<45} {:<15} {:<25}", 
                r.id, 
                r.issuer, 
                mod_cid_trunc, 
                r.issuance_date.to_rfc3339()
            );
        }
        println!("───────────────────────────────────────────────────────────────────────────────────────────────────");
    }

    Ok(())
}

async fn show_receipt_cli(receipt_ref: &str, output_json: bool, source_dir: &PathBuf) -> Result<(), CliError> {
    // Try to read as a file path first
    let receipt_json_content = if PathBuf::from(receipt_ref).is_file() {
        fs::read_to_string(receipt_ref).map_err(CliError::Io)?
    } else {
        // Assume receipt_ref is an ID (URN or CID) and try to find it in source_dir
        // This is a simplified lookup; a real CID lookup would involve a DAG store or better indexing.
        // For URNs like "urn:uuid:...", we might look for "receipt-urn:uuid:....json"
        let potential_filename = if receipt_ref.starts_with("urn:uuid:") {
            format!("{}.json", receipt_ref) // Or how CLI issue names them e.g. receipt-{uuid}.json
        } else {
             format!("{}.json", receipt_ref) // Assume if not path, it might be a CID-like ID or full URN
        };
        let file_path = source_dir.join(&potential_filename);
        if file_path.exists() {
            fs::read_to_string(&file_path).map_err(CliError::Io)?
        } else {
             // Attempt to search for any file that might contain this ID if it's part of the name
            // This is a very basic search, improve if needed.
            let mut found_path: Option<PathBuf> = None;
            if source_dir.exists() && source_dir.is_dir() {
                for entry in fs::read_dir(source_dir).map_err(CliError::Io)? {
                    let entry = entry.map_err(CliError::Io)?;
                    let path = entry.path();
                    if path.is_file() && path.file_name().map_or(false, |name| name.to_string_lossy().contains(receipt_ref)) {
                        found_path = Some(path);
                        break;
                    }
                }
            }
            if let Some(p) = found_path {
                 fs::read_to_string(p).map_err(CliError::Io)?
            } else {
                return Err(CliError::NotFound(format!("Receipt not found by ID/path: {}", receipt_ref)));
            }
        }
    };

    let receipt = ExecutionReceipt::from_json(&receipt_json_content)?;

    if output_json {
        println!("{}", serde_json::to_string_pretty(&receipt).map_err(ExecutionReceiptError::JsonSerialization)?);
    } else {
        println!("📝 {}", "EXECUTION RECEIPT".bold());
        println!("──────────────────────────────────────────────────");
        println!("{:<18} {}", "ID:", receipt.id);
        println!("{:<18} {}", "Issued by:", receipt.issuer);
        println!("{:<18} {}", "Issued at:", receipt.issuance_date.to_rfc3339());
        println!("──────────────────────────────────────────────────");
        println!("{}", "Credential Subject:".cyan());
        println!("  {:<16} {}", "Executor DID:", receipt.credential_subject.id);
        
        match &receipt.credential_subject.scope {
            ExecutionScope::Federation { federation_id } => {
                println!("  {:<16} Federation Execution", "Scope:");
                println!("    {:<14} {}", "Federation:", federation_id);
            },
            ExecutionScope::MeshCompute { task_id, job_id } => {
                println!("  {:<16} Mesh Compute Task", "Scope:");
                println!("    {:<14} {}", "Task ID:", task_id);
                println!("    {:<14} {}", "Job ID:", job_id);
            },
            ExecutionScope::Cooperative { coop_id, module } => {
                println!("  {:<16} Cooperative Execution", "Scope:");
                println!("    {:<14} {}", "Coop ID:", coop_id);
                println!("    {:<14} {}", "Module:", module);
            },
            ExecutionScope::Custom { description, metadata } => {
                println!("  {:<16} Custom", "Scope:");
                println!("    {:<14} {}", "Description:", description);
                println!("    {:<14} {}", "Metadata:", serde_json::to_string_pretty(metadata).unwrap_or_default());
            },
        }
        if let Some(submitter) = &receipt.credential_subject.submitter {
            println!("  {:<16} {}", "Submitter DID:", submitter);
        }
        println!("  {:<16} {}", "Module CID:", receipt.credential_subject.module_cid);
        println!("  {:<16} {}", "Result CID:", receipt.credential_subject.result_cid);
        println!("  {:<16} {}", "Timestamp:", receipt.credential_subject.timestamp);
        println!("  {:<16} {:?}", "Status:", receipt.credential_subject.status);
        if let Some(event_id) = &receipt.credential_subject.event_id {
            println!("  {:<16} {}", "Event ID:", hex::encode(event_id.0));
        }
        if let Some(props) = &receipt.credential_subject.additional_properties {
            println!("  {:<16} {}", "Additional:", serde_json::to_string_pretty(props).unwrap_or_default());
        }
        println!("──────────────────────────────────────────────────");
        println!("{}", "Proof:".cyan());
        if let Some(proof) = &receipt.proof {
            println!("  {:<16} {}", "Type:", proof.type_);
            println!("  {:<16} {}", "Created:", proof.created.to_rfc3339());
            println!("  {:<16} {}", "Purpose:", proof.proof_purpose);
            println!("  {:<16} {}", "Verification:", proof.verification_method);
            println!("  {:<16} {}...", "Signature:", &proof.proof_value[..std::cmp::min(proof.proof_value.len(), 40)]);
            match receipt.verify() {
                Ok(true) => println!("  {:<16} {}", "Verification:", "✅ Valid Signature".green()),
                _ => println!("  {:<16} {}", "Verification:", "❌ Invalid Signature".red()),
            }
        } else {
            println!("  No proof present.");
        }
        println!("──────────────────────────────────────────────────");
    }
    Ok(())
}

async fn verify_receipt_cli(file_path_opt: Option<&PathBuf>) -> Result<(), CliError> {
    let receipt_json_content = if let Some(path) = file_path_opt {
        fs::read_to_string(path).map_err(CliError::Io)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).map_err(CliError::Io)?;
        buffer
    };

    if receipt_json_content.trim().is_empty() {
        return Err(CliError::InvalidArgument("Receipt content is empty. Provide a file path or pipe JSON to stdin.".to_string()));
    }

    let receipt = ExecutionReceipt::from_json(&receipt_json_content)?;
    println!("Verifying ExecutionReceipt: {}", receipt.id);
    println!("Issued by: {}", receipt.issuer);

    match receipt.verify() {
        Ok(true) => {
            println!("✅ {}", "Signature Verification Successful".green());
            Ok(())
        }
        Ok(false) => { // This case should ideally not be hit if verify() returns Err for invalid sig
            println!("❌ {}", "Signature Verification Failed (verify returned false)".red());
            Err(CliError::VerificationFailed("Signature verification failed".to_string()))
        }
        Err(e) => {
            println!("❌ {}: {}", "Signature Verification Error".red(), e);
            Err(CliError::VerificationFailed(format!("Signature verification error: {}", e)))
        }
    }
}

impl From<ExecutionReceiptError> for CliError {
    fn from(err: ExecutionReceiptError) -> Self {
        CliError::Other(Box::new(err))
    }
} 