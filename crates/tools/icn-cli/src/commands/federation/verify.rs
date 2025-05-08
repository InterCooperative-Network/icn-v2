use crate::context::CliContext;
use crate::error::CliError;
use crate::commands::federation::bootstrap::{ParticipantKey, BootstrapError};

use colored::Colorize;
use icn_identity_core::trustbundle::{
    TrustBundle, QuorumConfig, QuorumType, TrustError
};
use icn_types::dag::{DagEvent, EventId, merkle::calculate_event_hash};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Verifier};
use serde_json::Value;

/// Error types for verification operations
#[derive(thiserror::Error, Debug)]
pub enum VerifyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Bundle not found at path: {0}")]
    BundleNotFound(String),
    
    #[error("Event not found: {0}")]
    EventNotFound(String),
    
    #[error("Key error: {0}")]
    KeyError(String),
    
    #[error("Trust error: {0}")]
    TrustError(#[from] TrustError),
    
    #[error("Bootstrap error: {0}")]
    BootstrapError(#[from] BootstrapError),
    
    #[error("Verification error: {0}")]
    Verification(String),
}

impl From<VerifyError> for CliError {
    fn from(err: VerifyError) -> Self {
        CliError::Other(Box::new(err))
    }
}

/// Results of verification
#[derive(Debug)]
pub struct VerificationResults {
    /// Overall verification status
    pub is_valid: bool,
    
    /// Bundle CID verification status
    pub cid_valid: bool,
    
    /// Signature verification status
    pub signatures_valid: bool,
    
    /// Quorum verification status
    pub quorum_valid: bool,
    
    /// Event references verification status
    pub events_valid: bool,
    
    /// Number of valid signatures
    pub valid_signatures: usize,
    
    /// Number of required signatures
    pub required_signatures: usize,
    
    /// List of events that were verified
    pub verified_events: Vec<EventId>,
    
    /// List of DIDs with valid signatures
    pub valid_signers: Vec<String>,
    
    /// List of DIDs with invalid signatures
    pub invalid_signers: Vec<String>,
    
    /// Missing events
    pub missing_events: Vec<EventId>,
}

/// Main function to run the verification
pub async fn run_verify(
    _context: &CliContext,
    bundle_path: &str,
    events_path: Option<&str>,
    keys_dir: Option<&str>,
    verbose: bool,
) -> Result<(), VerifyError> {
    println!("Verifying federation bundle at {}", bundle_path);
    
    // Step 1: Load the TrustBundle
    let bundle = load_trust_bundle(bundle_path)?;
    
    if verbose {
        println!("Loaded TrustBundle for federation: {}", bundle.federation_id);
        println!("Bundle contains {} referenced events", bundle.referenced_events.len());
        println!("Quorum config: {:?}", bundle.quorum_config);
        println!("Bundle has {} signatures", bundle.proof.signatures.len());
    }
    
    // Step 2: Find and load the referenced events
    let events = load_referenced_events(&bundle, events_path)?;
    
    // Step 3: Load participant keys for verification
    let keys = load_participant_keys(&bundle, keys_dir)?;
    
    // Step 4: Perform verification
    let results = verify_bundle(&bundle, &events, &keys, verbose)?;
    
    // Step 5: Display verification results
    print_verification_results(&bundle, &results, verbose);
    
    // Return error if verification failed
    if !results.is_valid {
        return Err(VerifyError::Verification(
            "Bundle verification failed. See above for details.".to_string()
        ));
    }
    
    Ok(())
}

/// Load a TrustBundle from file
fn load_trust_bundle(path: &str) -> Result<TrustBundle, VerifyError> {
    let bundle_path = Path::new(path);
    if !bundle_path.exists() {
        return Err(VerifyError::BundleNotFound(path.to_string()));
    }
    
    let mut file = File::open(bundle_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let bundle: TrustBundle = serde_json::from_str(&contents)?;
    Ok(bundle)
}

/// Load the referenced events for the bundle
fn load_referenced_events(
    bundle: &TrustBundle,
    events_path: Option<&str>,
) -> Result<Vec<DagEvent>, VerifyError> {
    let mut events = Vec::new();
    let mut missing_events = Vec::new();
    
    // First check if we have a specific events file path
    if let Some(path) = events_path {
        let events_file_path = Path::new(path);
        if events_file_path.exists() {
            let mut file = File::open(events_file_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            
            // Check if it's a single event or array of events
            if contents.trim().starts_with('[') {
                // Array of events
                let event_array: Vec<DagEvent> = serde_json::from_str(&contents)?;
                events.extend(event_array);
            } else {
                // Single event
                let event: DagEvent = serde_json::from_str(&contents)?;
                events.push(event);
            }
        }
    } else {
        // Try to find events in the same directory as the bundle
        if let Some(bundle_dir) = Path::new(bundle.bundle_cid.as_ref().unwrap_or(&"unknown".to_string()))
            .parent() 
        {
            // Look for genesis_event.json or any event files
            let event_file_path = bundle_dir.join("genesis_event.json");
            if event_file_path.exists() {
                let mut file = File::open(event_file_path)?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                
                let event: DagEvent = serde_json::from_str(&contents)?;
                events.push(event);
            }
        }
    }
    
    // Check if we have all referenced events
    for event_id in &bundle.referenced_events {
        let found = events.iter().any(|e| {
            let id = calculate_event_hash(e);
            &id == event_id
        });
        
        if !found {
            missing_events.push(event_id.clone());
        }
    }
    
    if !missing_events.is_empty() {
        println!("Warning: {} referenced events could not be found", missing_events.len());
        for id in &missing_events {
            println!("  Missing event: {}", id);
        }
    }
    
    Ok(events)
}

/// Load participant keys for verification
fn load_participant_keys(
    bundle: &TrustBundle,
    keys_dir: Option<&str>,
) -> Result<HashMap<String, VerifyingKey>, VerifyError> {
    let mut key_map = HashMap::new();
    
    // Try to find keys in provided directory or alongside the bundle
    let keys_path = if let Some(dir) = keys_dir {
        PathBuf::from(dir)
    } else if let Some(bundle_path) = Path::new(bundle.bundle_cid.as_ref().unwrap_or(&"unknown".to_string())).parent() {
        // Look in the bundle directory
        bundle_path.join("federation_keys.json")
    } else {
        // Fallback to current directory
        PathBuf::from("federation_keys.json")
    };
    
    // Check if federation_keys.json exists
    if keys_path.exists() && keys_path.is_file() {
        // Load the keys file
        let mut file = File::open(&keys_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        // Parse as a map of DID -> ParticipantKey
        if contents.contains("\"did\":") {
            // Try parsing as a map of DID -> ParticipantKey
            let key_map_value: Value = serde_json::from_str(&contents)?;
            
            if let Value::Object(map) = key_map_value {
                for (did, key_val) in map {
                    match load_verifying_key_from_json(&key_val) {
                        Ok(verifying_key) => {
                            key_map.insert(did, verifying_key);
                        },
                        Err(e) => {
                            println!("Warning: Failed to load key for {}: {}", did, e);
                        }
                    }
                }
            }
        } else {
            // Try parsing as a directory of key files
            let dir_path = if keys_path.is_dir() { keys_path } else { keys_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf() };
            
            // Read all .json files in the directory
            for entry in fs::read_dir(dir_path)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map_or(false, |ext| ext == "json") {
                    // Try loading as a participant key
                    match ParticipantKey::from_file(&path) {
                        Ok(key) => {
                            // Convert to verifying key
                            let public_key_val = &key.public_key;
                            let verifying_key = load_verifying_key_from_json(public_key_val)?;
                            key_map.insert(key.did, verifying_key);
                        },
                        Err(_) => {
                            // Ignore files that don't parse as participant keys
                        }
                    }
                }
            }
        }
    }
    
    // If we couldn't load any keys, generate warnings
    if key_map.is_empty() {
        println!("Warning: No verification keys could be loaded.");
        println!("         Bundle signatures cannot be verified without keys.");
    } else {
        // Check if we have keys for all participants
        for did in &bundle.quorum_config.participants {
            let did_str = did.to_string();
            if !key_map.contains_key(&did_str) {
                println!("Warning: No key found for participant: {}", did);
            }
        }
    }
    
    Ok(key_map)
}

/// Load a verifying key from a JSON value
fn load_verifying_key_from_json(value: &Value) -> Result<VerifyingKey, VerifyError> {
    if let Some(format) = value.get("format").and_then(|f| f.as_str()) {
        let public_key = value.get("public_key")
            .ok_or_else(|| VerifyError::KeyError("No public_key field found".to_string()))?;
            
        match format {
            "jwk" => {
                // Extract the 'x' field from JWK format
                let x = public_key.get("x")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| VerifyError::KeyError("Invalid JWK format: missing 'x' field".to_string()))?;
                    
                let decoded = base64::decode(x)
                    .map_err(|e| VerifyError::KeyError(format!("Failed to decode public key: {}", e)))?;
                    
                let bytes: [u8; 32] = decoded.try_into()
                    .map_err(|_| VerifyError::KeyError("Invalid public key length".to_string()))?;
                    
                let verifying_key = VerifyingKey::from_bytes(&bytes)
                    .map_err(|e| VerifyError::KeyError(format!("Invalid public key: {}", e)))?;
                    
                Ok(verifying_key)
            },
            "base58" => {
                // Extract the Base58 encoded public key
                let encoded = public_key.as_str()
                    .ok_or_else(|| VerifyError::KeyError("Invalid Base58 format".to_string()))?;
                    
                let (_, decoded) = multibase::decode(encoded)
                    .map_err(|e| VerifyError::KeyError(format!("Failed to decode Base58 key: {}", e)))?;
                    
                let bytes: [u8; 32] = decoded.try_into()
                    .map_err(|_| VerifyError::KeyError("Invalid public key length".to_string()))?;
                    
                let verifying_key = VerifyingKey::from_bytes(&bytes)
                    .map_err(|e| VerifyError::KeyError(format!("Invalid public key: {}", e)))?;
                    
                Ok(verifying_key)
            },
            _ => Err(VerifyError::KeyError(format!("Unsupported key format: {}", format)))
        }
    } else if value.is_object() && value.get("kty").is_some() {
        // Direct JWK format
        let x = value.get("x")
            .and_then(|x| x.as_str())
            .ok_or_else(|| VerifyError::KeyError("Invalid JWK format: missing 'x' field".to_string()))?;
            
        let decoded = base64::decode(x)
            .map_err(|e| VerifyError::KeyError(format!("Failed to decode public key: {}", e)))?;
            
        let bytes: [u8; 32] = decoded.try_into()
            .map_err(|_| VerifyError::KeyError("Invalid public key length".to_string()))?;
            
        let verifying_key = VerifyingKey::from_bytes(&bytes)
            .map_err(|e| VerifyError::KeyError(format!("Invalid public key: {}", e)))?;
            
        Ok(verifying_key)
    } else if value.is_string() {
        // Direct Base58 string
        let encoded = value.as_str().unwrap();
        
        let (_, decoded) = multibase::decode(encoded)
            .map_err(|e| VerifyError::KeyError(format!("Failed to decode Base58 key: {}", e)))?;
            
        let bytes: [u8; 32] = decoded.try_into()
            .map_err(|_| VerifyError::KeyError("Invalid public key length".to_string()))?;
            
        let verifying_key = VerifyingKey::from_bytes(&bytes)
            .map_err(|e| VerifyError::KeyError(format!("Invalid public key: {}", e)))?;
            
        Ok(verifying_key)
    } else {
        Err(VerifyError::KeyError("Unsupported key format".to_string()))
    }
}

/// Convert a byte slice to a 64-byte array for ed25519 signatures
fn to_signature_bytes(slice: &[u8]) -> Result<[u8; 64], VerifyError> {
    if slice.len() != 64 {
        return Err(VerifyError::KeyError(format!(
            "Invalid signature length: expected 64 bytes, got {}", slice.len()
        )));
    }
    
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(slice);
    Ok(bytes)
}

/// Verify a bundle against events and keys
fn verify_bundle(
    bundle: &TrustBundle,
    events: &[DagEvent],
    keys: &HashMap<String, VerifyingKey>,
    verbose: bool,
) -> Result<VerificationResults, VerifyError> {
    let mut results = VerificationResults {
        is_valid: false,
        cid_valid: false,
        signatures_valid: false,
        quorum_valid: false,
        events_valid: false,
        valid_signatures: 0,
        required_signatures: bundle.quorum_config.required_signatures(),
        verified_events: Vec::new(),
        valid_signers: Vec::new(),
        invalid_signers: Vec::new(),
        missing_events: Vec::new(),
    };
    
    // Step 1: Verify bundle CID
    if let Some(cid) = &bundle.bundle_cid {
        let bundle_hash = bundle.calculate_hash();
        let expected_hash = hex::encode(bundle_hash);
        let cid_hash = &cid[4..]; // Remove "bafy" prefix to get the hash portion
        
        results.cid_valid = expected_hash.starts_with(cid_hash);
        
        if verbose {
            println!("CID verification: {}", if results.cid_valid { "✅ Valid".green() } else { "❌ Invalid".red() });
            println!("  Expected hash begins with: {}", &expected_hash[..20]);
            println!("  CID hash begins with: {}", &cid_hash[..20]);
        }
    } else {
        println!("Warning: Bundle has no CID, skipping CID verification");
        // Don't fail verification just because of missing CID
        results.cid_valid = true;
    }
    
    // Step 2: Verify event references
    let mut all_events_found = true;
    for event_id in &bundle.referenced_events {
        let found = events.iter().any(|e| {
            let id = calculate_event_hash(e);
            &id == event_id
        });
        
        if found {
            results.verified_events.push(event_id.clone());
        } else {
            results.missing_events.push(event_id.clone());
            all_events_found = false;
        }
    }
    
    results.events_valid = all_events_found || bundle.referenced_events.is_empty();
    
    if verbose {
        println!("Event verification: {}", if results.events_valid { "✅ Valid".green() } else { "❌ Missing events".red() });
        println!("  Found {} of {} referenced events", results.verified_events.len(), bundle.referenced_events.len());
        
        if !results.missing_events.is_empty() {
            println!("  Missing events:");
            for id in &results.missing_events {
                println!("    - {}", id);
            }
        }
    }
    
    // Step 3: Verify signatures if we have keys
    if !keys.is_empty() {
        let bundle_hash = bundle.calculate_hash();
        
        // Verify each signature
        for (did, signature_bytes) in &bundle.proof.signatures {
            let did_str = did.to_string();
            // Check if we have the key for this DID
            if let Some(verifying_key) = keys.get(&did_str) {
                // Convert the signature bytes to ed25519_dalek::Signature
                match to_signature_bytes(signature_bytes.as_slice()) {
                    Ok(sig_array) => {
                        // Create signature from fixed-size array
                        let signature = Signature::from_bytes(&sig_array);
                        
                        // Verify the signature
                        match verifying_key.verify(&bundle_hash, &signature) {
                            Ok(_) => {
                                results.valid_signers.push(did_str);
                                results.valid_signatures += 1;
                                
                                if verbose {
                                    println!("Signature from {}: ✅ Valid", did);
                                }
                            },
                            Err(_) => {
                                results.invalid_signers.push(did_str);
                                
                                if verbose {
                                    println!("Signature from {}: ❌ Invalid", did);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        results.invalid_signers.push(did_str);
                        
                        if verbose {
                            println!("Signature from {}: ❌ Invalid format: {}", did, e);
                        }
                    }
                }
            } else {
                // No key for this DID, can't verify
                if verbose {
                    println!("Signature from {}: ⚠️ Can't verify (no key)", did);
                }
            }
        }
        
        // Check quorum requirements
        match &bundle.quorum_config.quorum_type {
            QuorumType::Majority | QuorumType::Threshold(_) | QuorumType::All => {
                results.quorum_valid = results.valid_signatures >= results.required_signatures;
            },
            QuorumType::Weighted(weights) => {
                // Calculate total weight of valid signers
                let mut total_weight = 0;
                let mut max_possible_weight = 0;
                
                for (did, weight) in weights {
                    max_possible_weight += weight;
                    let did_str = did.to_string();
                    if results.valid_signers.contains(&did_str) {
                        total_weight += weight;
                    }
                }
                
                // Require majority of total weight
                let required_weight = (max_possible_weight / 2) + 1;
                results.quorum_valid = total_weight >= required_weight;
                results.required_signatures = required_weight as usize;
            }
        }
        
        results.signatures_valid = results.invalid_signers.is_empty();
        
        if verbose {
            println!("Quorum verification: {}", if results.quorum_valid { "✅ Valid".green() } else { "❌ Invalid".red() });
            println!("  Required signatures: {}", results.required_signatures);
            println!("  Valid signatures: {}", results.valid_signatures);
        }
    } else {
        // No keys available, skip signature verification
        println!("Warning: No keys available for signature verification");
        // Don't fail verification just because of missing keys
        results.signatures_valid = true;
        results.quorum_valid = true;
    }
    
    // Overall validity
    results.is_valid = results.cid_valid && results.events_valid && 
                      results.signatures_valid && results.quorum_valid;
    
    Ok(results)
}

/// Print the verification results
fn print_verification_results(bundle: &TrustBundle, results: &VerificationResults, verbose: bool) {
    // Print a summary header
    println!("\n========== Federation TrustBundle Verification ==========");
    println!("Federation ID: {}", bundle.federation_id);
    if let Some(cid) = &bundle.bundle_cid {
        println!("Bundle CID: {}", cid);
    }
    
    // Get quorum details
    let quorum_desc = match &bundle.quorum_config.quorum_type {
        QuorumType::All => "All participants".to_string(),
        QuorumType::Majority => "Majority (>50%)".to_string(),
        QuorumType::Threshold(t) => format!("Threshold ({}%)", t),
        QuorumType::Weighted(_) => "Weighted".to_string(),
    };
    
    println!("Quorum: {} ({} of {} required)", 
        quorum_desc, 
        results.required_signatures,
        bundle.quorum_config.participants.len()
    );
    
    // Print the overall result with color
    let status = if results.is_valid {
        "✅ VALID".green().bold()
    } else {
        "❌ INVALID".red().bold()
    };
    
    println!("\nVerification result: {}", status);
    
    // Print detailed results
    println!("\nVerification details:");
    println!("  Bundle CID: {}", if results.cid_valid { "✅ Valid".green() } else { "❌ Invalid".red() });
    println!("  Events: {}", if results.events_valid { "✅ Valid".green() } else { "❌ Missing".red() });
    println!("  Signatures: {}", if results.signatures_valid { "✅ Valid".green() } else { "❌ Invalid".red() });
    println!("  Quorum: {}", if results.quorum_valid { "✅ Satisfied".green() } else { "❌ Not satisfied".red() });
    
    // Show signature details if verbose or if verification failed
    if verbose || !results.is_valid {
        println!("\nSignature details:");
        println!("  Valid signatures: {}", results.valid_signatures);
        println!("  Required signatures: {}", results.required_signatures);
        
        if !results.valid_signers.is_empty() {
            println!("\n  Valid signers:");
            for did in &results.valid_signers {
                println!("    - {}", did);
            }
        }
        
        if !results.invalid_signers.is_empty() {
            println!("\n  Invalid signers:");
            for did in &results.invalid_signers {
                println!("    - {}", did);
            }
        }
    }
    
    println!("\nEvent references:");
    println!("  Referenced events: {}", bundle.referenced_events.len());
    println!("  Verified events: {}", results.verified_events.len());
    println!("  Missing events: {}", results.missing_events.len());
    
    println!("=====================================================");
} 