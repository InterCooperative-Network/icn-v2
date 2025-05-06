use clap::Subcommand;
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
use std::path::PathBuf;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use serde_json::json;
use hex;

#[derive(Subcommand, Debug)]
pub enum ReceiptCommands {
    /// Issue a new execution receipt
    Issue {
        /// Path to the key file for signing the receipt
        #[clap(long)]
        key_file: String,

        /// DID of the node that executed the computation
        #[clap(long)]
        executor: String,

        /// Federation DID under which this execution occurred
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

        /// Optional submitter DID
        #[clap(long)]
        submitter: Option<String>,

        /// Output file for the receipt (JSON format)
        #[clap(long)]
        output: Option<String>,
    },

    /// Verify an execution receipt
    Verify {
        /// Path to the receipt file (JSON format)
        #[clap(long)]
        receipt_file: String,

        /// Skip verification against linked DAG events
        #[clap(long, default_value = "false")]
        skip_dag_verification: bool,
    },

    /// Display an execution receipt in human-readable format
    Show {
        /// Path to the receipt file (JSON format)
        #[clap(long)]
        receipt_file: String,
    },
}

pub async fn handle_receipt_command(
    context: &mut CliContext,
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
            issue_receipt(
                key_file,
                executor,
                federation,
                module_cid,
                result_cid,
                status,
                submitter.as_deref(),
                output.as_deref(),
            ).await?;
        },
        ReceiptCommands::Verify {
            receipt_file,
            skip_dag_verification,
        } => {
            verify_receipt(receipt_file, *skip_dag_verification).await?;
        },
        ReceiptCommands::Show {
            receipt_file,
        } => {
            show_receipt(receipt_file).await?;
        },
    }

    Ok(())
}

async fn issue_receipt(
    key_file: &str,
    executor: &str,
    federation: &str,
    module_cid: &str,
    result_cid: &str,
    status_str: &str,
    submitter: Option<&str>,
    output: Option<&str>,
) -> Result<(), CliError> {
    // Load the signing key
    let key_data = fs::read_to_string(key_file)
        .map_err(|e| CliError::IOError(e))?;
    
    let did_key = DidKey::from_jwk(&key_data)
        .map_err(|e| CliError::InvalidKey(e.to_string()))?;

    // Parse the execution status
    let status = match status_str.to_lowercase().as_str() {
        "success" => ExecutionStatus::Success,
        "failed" => ExecutionStatus::Failed,
        "pending" => ExecutionStatus::Pending,
        "canceled" => ExecutionStatus::Canceled,
        _ => return Err(CliError::InvalidArgument(format!("Invalid status: {}", status_str))),
    };

    // Create the execution scope
    let scope = ExecutionScope::Federation {
        federation_id: federation.to_string(),
    };

    // Create the execution subject
    let subject = ExecutionSubject {
        id: executor.to_string(),
        scope,
        submitter: submitter.map(|s| s.to_string()),
        module_cid: module_cid.to_string(),
        result_cid: result_cid.to_string(),
        event_id: None, // No DAG event ID for CLI-issued receipts
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        status,
        additional_properties: Some(json!({
            "issuedBy": "icn-cli",
            "version": env!("CARGO_PKG_VERSION"),
        })),
    };

    // Generate a UUID for the receipt
    let receipt_id = format!("urn:uuid:{}", Uuid::new_v4());

    // Create and sign the receipt
    let receipt = ExecutionReceipt::new(
        receipt_id,
        did_key.did().to_string(),
        subject,
    ).sign(&did_key)
        .map_err(|e| CliError::Other(Box::new(e)))?;

    // Serialize the receipt
    let receipt_json = receipt.to_json()
        .map_err(|e| CliError::Other(Box::new(e)))?;

    // Determine output path
    let output_path = if let Some(path) = output {
        PathBuf::from(path)
    } else {
        PathBuf::from(format!("receipt-{}.json", Uuid::new_v4()))
    };

    // Write the receipt to file
    fs::write(&output_path, receipt_json)
        .map_err(|e| CliError::IOError(e))?;

    println!("✅ Execution receipt issued successfully");
    println!("   Receipt ID: {}", receipt.id);
    println!("   Issuer: {}", receipt.issuer);
    println!("   Saved to: {}", output_path.display());

    Ok(())
}

async fn verify_receipt(
    receipt_file: &str,
    skip_dag_verification: bool,
) -> Result<(), CliError> {
    // Load the receipt
    let receipt_json = fs::read_to_string(receipt_file)
        .map_err(|e| CliError::IOError(e))?;

    // Parse the receipt
    let receipt = ExecutionReceipt::from_json(&receipt_json)
        .map_err(|e| CliError::Other(Box::new(e)))?;

    // Verify the signature
    match receipt.verify() {
        Ok(true) => {
            println!("✅ Signature verification successful");
        },
        Ok(false) => {
            println!("❌ Signature verification failed");
            return Err(CliError::VerificationFailed("Signature verification failed".to_string()));
        },
        Err(e) => {
            println!("❌ Signature verification error: {}", e);
            return Err(CliError::VerificationFailed(format!("Signature verification error: {}", e)));
        }
    }

    // Additional DAG verification would go here
    if !skip_dag_verification && receipt.credential_subject.event_id.is_some() {
        println!("ℹ️ DAG verification skipped (not implemented yet)");
    }

    println!("✅ Receipt verification completed successfully");
    println!("   Receipt ID: {}", receipt.id);
    println!("   Issuer: {}", receipt.issuer);
    println!("   Executor: {}", receipt.credential_subject.id);

    Ok(())
}

async fn show_receipt(
    receipt_file: &str,
) -> Result<(), CliError> {
    // Load the receipt
    let receipt_json = fs::read_to_string(receipt_file)
        .map_err(|e| CliError::IOError(e))?;

    // Parse the receipt
    let receipt = ExecutionReceipt::from_json(&receipt_json)
        .map_err(|e| CliError::Other(Box::new(e)))?;

    // Print receipt details
    println!("📝 EXECUTION RECEIPT");
    println!("────────────────────────────────────────");
    println!("ID:           {}", receipt.id);
    println!("Issued by:    {}", receipt.issuer);
    println!("Issued at:    {}", receipt.issuance_date);
    println!("────────────────────────────────────────");
    println!("EXECUTION DETAILS");
    println!("Executor:     {}", receipt.credential_subject.id);
    
    // Print scope-specific information
    match &receipt.credential_subject.scope {
        ExecutionScope::Federation { federation_id } => {
            println!("Scope:        Federation Execution");
            println!("Federation:   {}", federation_id);
        },
        ExecutionScope::MeshCompute { task_id, job_id } => {
            println!("Scope:        Mesh Compute Task");
            println!("Task ID:      {}", task_id);
            println!("Job ID:       {}", job_id);
        },
        ExecutionScope::Cooperative { coop_id, module } => {
            println!("Scope:        Cooperative Execution");
            println!("Coop ID:      {}", coop_id);
            println!("Module:       {}", module);
        },
        ExecutionScope::Custom { description, .. } => {
            println!("Scope:        Custom");
            println!("Description:  {}", description);
        },
    }
    
    if let Some(submitter) = &receipt.credential_subject.submitter {
        println!("Submitter:    {}", submitter);
    }
    
    println!("────────────────────────────────────────");
    println!("EXECUTION RESULT");
    println!("Status:       {:?}", receipt.credential_subject.status);
    println!("Module CID:   {}", receipt.credential_subject.module_cid);
    println!("Result CID:   {}", receipt.credential_subject.result_cid);
    println!("Timestamp:    {}", receipt.credential_subject.timestamp);
    
    if let Some(event_id) = &receipt.credential_subject.event_id {
        println!("Event ID:     {}", hex::encode(event_id.0));
    }
    
    // Print verification status
    match receipt.verify() {
        Ok(true) => println!("Verification:  ✅ Valid signature"),
        _ => println!("Verification:  ❌ Invalid signature"),
    }
    
    println!("────────────────────────────────────────");

    Ok(())
}

impl From<ExecutionReceiptError> for CliError {
    fn from(err: ExecutionReceiptError) -> Self {
        CliError::Other(Box::new(err))
    }
} 