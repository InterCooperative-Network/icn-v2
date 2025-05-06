use anyhow::{Result, Context};
use icn_wallet::{verify_dispatch_credential, TrustPolicyStore, TrustedDidEntry, TrustLevel};
use icn_types::dag::memory::MemoryDagStore;
use std::fs;
use std::process;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }
    
    match args[1].as_str() {
        "verify-dispatch" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path for verify-dispatch command");
                print_usage();
                process::exit(1);
            }
            
            let file_path = &args[2];
            let dag_dir = args.get(3).map(|s| s.as_str()).unwrap_or("./dag");
            let policy_path = args.get(4).map(|s| s.as_str()).unwrap_or("./policy.json");
            
            verify_dispatch(file_path, dag_dir, policy_path)?;
        }
        _ => {
            eprintln!("Error: Unknown command '{}'", args[1]);
            print_usage();
            process::exit(1);
        }
    }
    
    Ok(())
}

fn print_usage() {
    println!("Usage: verify_cli <command> [arguments]");
    println!("");
    println!("Commands:");
    println!("  verify-dispatch <file> [dag-dir] [policy-file]  Verify a dispatch credential");
}

fn verify_dispatch(file_path: &str, dag_dir: &str, policy_path: &str) -> Result<()> {
    // Load the credential from file
    let vc_json = fs::read_to_string(file_path)
        .context(format!("Failed to read credential file: {}", file_path))?;
    
    // Create a memory DAG store (in a real app, this would be loaded from disk)
    let dag_store = MemoryDagStore::new();
    
    // Create a dummy trust policy (in a real app, this would be loaded from disk)
    let policy_store = TrustPolicyStore {
        federation_id: "test-federation".to_string(),
        trusted_dids: vec![
            TrustedDidEntry {
                did: "did:icn:scheduler123".to_string(),
                level: TrustLevel::Admin,
                expires: None,
                notes: Some("Test trusted scheduler".to_string()),
            }
        ],
        policy_cid: None,
        previous_policy_cid: None,
    };
    
    // Verify the credential
    let report = verify_dispatch_credential(&vc_json, &dag_store, &policy_store)
        .context("Failed to verify dispatch credential")?;
    
    // Print the verification results
    println!("\n===== Verification Report =====");
    println!("Issuer: {}", report.issuer_did);
    println!("Signature valid: {}", report.signature_valid);
    println!("Issuer trusted: {}", report.is_trusted);
    println!("Credential revoked: {}", report.is_revoked);
    println!("Policy version: {}", report.policy_version);
    println!("Policy lineage verified: {}", report.lineage_verified);
    println!("Overall validity: {}", report.overall_valid);
    
    if let Some(error) = report.error {
        println!("Error: {}", error);
    }
    
    println!("==============================\n");
    
    Ok(())
} 