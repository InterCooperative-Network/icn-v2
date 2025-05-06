use anyhow::{Result, anyhow, Context};
use serde::{Serialize, Deserialize};
use icn_types::{dag::DagStore, Cid, Did};
// use icn_identity_core::did::DidKey; // Unused import
use chrono::{DateTime, Utc};
// use std::str::FromStr; // Unused import
use hex::FromHex;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_ipld_dagcbor;
// use std::path::PathBuf; // Unused import
// use futures; // Unused import - futures::executor::block_on is used directly
use multibase; // Added for Cid string parsing

/// A report on the verification status of a dispatch credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// The DID of the credential issuer
    pub issuer_did: String,
    
    /// Whether the signature is cryptographically valid
    pub signature_valid: bool,
    
    /// Whether the issuer is in the current trust policy
    pub is_trusted: bool,
    
    /// Whether the credential has been revoked
    pub is_revoked: bool,
    
    /// The version of the trust policy used for verification
    pub policy_version: String,
    
    /// Whether the trust policy lineage was verified
    pub lineage_verified: bool,
    
    /// Overall validity of the credential
    pub overall_valid: bool,
    
    /// Optional capability verification status
    pub capability_match: Option<bool>,
    
    /// Error message if verification failed
    pub error: Option<String>,
    
    /// Verification timestamp
    pub timestamp: DateTime<Utc>,
}

/// Dispatch credential data structure based on W3C Verifiable Credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchCredential {
    /// Credential context for JSON-LD
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (unique identifier)
    pub id: String,
    
    /// Credential type
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    
    /// Issuer DID (scheduler that made the dispatch decision)
    pub issuer: String,
    
    /// Issuance date
    pub issuanceDate: DateTime<Utc>,
    
    /// Credential subject (requestor and task details)
    pub credentialSubject: DispatchCredentialSubject,
    
    /// Cryptographic proof
    pub proof: Option<DispatchCredentialProof>,
}

/// Subject of the dispatch credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchCredentialSubject {
    /// Requestor DID
    pub id: String,
    
    /// Task request details
    pub taskRequest: TaskRequestDetails,
    
    /// Capability requirements used for dispatch
    pub capabilities: CapabilitySelector,
    
    /// Selected node DID
    pub selectedNode: String,
    
    /// Score of the selected bid
    pub score: f64,
    
    /// Dispatch timestamp
    pub dispatchTime: DateTime<Utc>,
    
    /// Number of nodes that matched the requirements
    pub matchingNodeCount: usize,
    
    /// Selected bid details
    pub bid: BidDetails,
}

/// Task request details for the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequestDetails {
    /// WASM module hash
    pub wasm_hash: String,
    
    /// WASM module size in bytes
    pub wasm_size: usize,
    
    /// Input data URIs
    pub inputs: Vec<String>,
    
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: u64,
    
    /// Required memory in MB
    pub memory_mb: u64,
    
    /// Required CPU cores
    pub cores: u64,
    
    /// Task priority (1-100)
    pub priority: u8,
    
    /// Timestamp when the task was requested
    pub timestamp: DateTime<Utc>,
    
    /// Federation ID
    pub federation_id: String,
}

/// Capability selector for dispatch requirements
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CapabilitySelector {
    /// Required compute capabilities
    pub compute: Option<Vec<String>>,
    
    /// Required hardware capabilities
    pub hardware: Option<Vec<String>>,
    
    /// Required network capabilities
    pub network: Option<Vec<String>>,
    
    /// Required storage capabilities
    pub storage: Option<Vec<String>>,
    
    /// Required security capabilities
    pub security: Option<Vec<String>>,
}

/// Bid details for the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidDetails {
    /// Bid CID in the DAG
    pub bidCid: String,
    
    /// Offered latency in milliseconds
    pub latency: u64,
    
    /// Available memory in MB
    pub memory: u64,
    
    /// Available CPU cores
    pub cores: u64,
    
    /// Bidder's reputation score
    pub reputation: u8,
    
    /// Renewable energy percentage
    pub renewable: u8,
}

/// Cryptographic proof for the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchCredentialProof {
    /// Proof type (e.g., Ed25519Signature2020)
    #[serde(rename = "type")]
    pub proof_type: String,
    
    /// Verification method identifier
    pub verificationMethod: String,
    
    /// Creation date of the proof
    pub created: DateTime<Utc>,
    
    /// Hex-encoded signature value
    pub proofValue: String,
}

/// Trusted DID policy for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicyStore {
    /// Federation ID this policy applies to
    pub federation_id: String,
    
    /// List of trusted DIDs with their trust levels
    pub trusted_dids: Vec<TrustedDidEntry>,
    
    /// CID of this policy in the DAG
    pub policy_cid: Option<String>,
    
    /// Reference to a previous policy (for updates)
    pub previous_policy_cid: Option<String>,
}

/// Entry in the trusted DID list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDidEntry {
    /// The DID to trust
    pub did: String,
    
    /// Trust level for this DID
    pub level: TrustLevel,
    
    /// Optional expiration date
    pub expires: Option<DateTime<Utc>>,
    
    /// Optional notes about this DID
    pub notes: Option<String>,
}

/// Trust level for DIDs in the policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Fully trusted entity (can submit manifests, dispatch tasks, etc.)
    Full,
    
    /// Can only submit manifests
    ManifestProvider,
    
    /// Can only request tasks
    Requestor,
    
    /// Can only execute tasks
    Worker,
    
    /// Trusted for admin operations
    Admin,
}

/// Revocation entry for credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationEntry {
    /// The type of entity being revoked
    pub revocation_type: RevocationType,
    
    /// The target DID or credential ID being revoked
    pub target_id: String,
    
    /// When the revocation was issued
    pub issued_at: DateTime<Utc>,
    
    /// Reason for revocation (optional)
    pub reason: Option<String>,
    
    /// CID of the revocation entry in the DAG
    pub cid: Option<String>,
}

/// Type of entity being revoked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationType {
    /// Revocation of a specific credential
    Credential,
    
    /// Revocation of all credentials issued by a DID
    Issuer,
    
    /// Revocation of all credentials issued to a DID
    Subject,
}

/// Verify a dispatch credential against trust policies and revocation status
pub fn verify_dispatch_credential(
    vc_json: &str, 
    dag_store: &impl DagStore, 
    policy_store: &TrustPolicyStore
) -> Result<VerificationReport> {
    // Parse the credential from JSON
    let credential: DispatchCredential = serde_json::from_str(vc_json)
        .context("Failed to parse dispatch credential from JSON")?;
    
    // Start building verification report
    let mut report = VerificationReport {
        issuer_did: credential.issuer.clone(),
        signature_valid: false,
        is_trusted: false,
        is_revoked: false,
        policy_version: policy_store.policy_cid.clone()
            .unwrap_or_else(|| "local".to_string()),
        lineage_verified: false,
        overall_valid: false,
        capability_match: None,
        error: None,
        timestamp: Utc::now(),
    };
    
    // Step 1: Verify the signature
    match verify_credential_signature(&credential) {
        Ok(is_valid) => {
            report.signature_valid = is_valid;
            if !is_valid {
                report.error = Some("Invalid credential signature".to_string());
                return Ok(report);
            }
        },
        Err(e) => {
            report.error = Some(format!("Signature verification error: {}", e));
            return Ok(report);
        }
    }
    
    // Step 2: Check if issuer is in the trust policy
    let issuer_did = Did::from_string(&credential.issuer)
        .map_err(|e| anyhow!("Failed to parse issuer DID: {}", e))?;
    let is_trusted = policy_store.trusted_dids.iter()
        .any(|entry| {
            match Did::from_string(&entry.did) {
                Ok(did) => {
                    // Make sure it's a trusted scheduler or admin
                    did == issuer_did && 
                    (entry.level == TrustLevel::Full || entry.level == TrustLevel::Admin) &&
                    // Check expiration if present
                    entry.expires.map_or(true, |exp| exp > Utc::now())
                }
                Err(_) => false, // If DID parsing fails, it's not a match
            }
        });
    
    report.is_trusted = is_trusted;
    if !is_trusted {
        report.error = Some("Issuer not trusted in current policy".to_string());
        report.overall_valid = false;
        return Ok(report);
    }
    
    // Step 3: Check for revocations
    if let Err(e) = check_revocation_status(dag_store, &credential) {
        report.error = Some(format!("Error checking revocation status: {}", e));
        report.overall_valid = false;
        return Ok(report);
    }
    
    // If we have a policy CID, verify policy lineage
    if let Some(policy_cid_str) = &policy_store.policy_cid {
        match verify_policy_lineage(dag_store, policy_cid_str) {
            Ok(verified) => {
                report.lineage_verified = verified;
                if !verified {
                    report.error = Some("Trust policy lineage verification failed".to_string());
                    report.overall_valid = false;
                    return Ok(report);
                }
            },
            Err(e) => {
                report.error = Some(format!("Policy lineage verification error: {}", e));
                report.overall_valid = false;
                return Ok(report);
            }
        }
    } else {
        // If no policy CID, we can't verify lineage but don't fail
        report.lineage_verified = false;
    }
    
    // All checks passed
    report.overall_valid = true;
    
    Ok(report)
}

/// Verify the cryptographic signature of a dispatch credential
fn verify_credential_signature(credential: &DispatchCredential) -> Result<bool> {
    // If there's no proof, it's not valid
    let proof = match &credential.proof {
        Some(p) => p,
        None => return Ok(false),
    };
    
    // Only support Ed25519 signatures for now
    if proof.proof_type != "Ed25519Signature2020" {
        return Err(anyhow!("Unsupported proof type: {}", proof.proof_type));
    }
    
    // Extract the verification method from the proof
    let method_parts: Vec<&str> = proof.verificationMethod.split('#').collect();
    if method_parts.len() != 2 {
        return Err(anyhow!("Invalid verification method format"));
    }
    
    // The DID should match the issuer
    let did_from_method = method_parts[0];
    if did_from_method != credential.issuer {
        return Err(anyhow!("Verification method DID doesn't match issuer"));
    }
    
    // Clone the credential and remove the proof (signatures don't include themselves)
    let mut credential_to_verify = credential.clone();
    credential_to_verify.proof = None;
    
    // Convert to canonical form (DAG-CBOR) for verification
    let canonical_bytes = serde_ipld_dagcbor::to_vec(&credential_to_verify)
        .context("Failed to serialize credential to DAG-CBOR for verification")?;
    
    // Extract the signature
    let signature_bytes = Vec::from_hex(&proof.proofValue)
        .context("Failed to decode hex signature")?;
    
    let signature = Signature::from_slice(&signature_bytes)
        .context("Failed to parse signature")?;
    
    // Get the public key from the DID
    let did = Did::from_string(&credential.issuer)
        .map_err(|e| anyhow!("Failed to parse issuer DID for signature: {}", e))?;
    let pubkey_bytes = did.public_key_bytes();
    
    let pubkey_array: &[u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = pubkey_bytes[..].try_into()
        .map_err(|_| anyhow!("Invalid public key length"))?;
    let verifying_key = VerifyingKey::from_bytes(pubkey_array)
        .map_err(|e| anyhow!("Failed to create verifying key: {}", e))?;
    
    // Verify the signature
    match verifying_key.verify(&canonical_bytes, &signature) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Check if a credential has been revoked
fn check_revocation_status(dag_store: &impl DagStore, credential: &DispatchCredential) -> Result<bool> {
    // This would fetch revocation notices from the DAG
    // For now, we'll implement a simplified version that looks for RevocationRecord nodes
    
    // Get ordered nodes to find the most recent revocations
    let nodes = futures::executor::block_on(dag_store.get_ordered_nodes())
        .context("Failed to get DAG nodes")?;
    
    for node in nodes {
        if let Some(_cid) = &node.cid {
            if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                // Check if it's a revocation record
                if payload.get("type").and_then(|t| t.as_str()) == Some("RevocationRecord") {
                    if let Some(revocation) = payload.get("revocation") {
                        // Parse the revocation entry
                        if let Ok(entry) = serde_json::from_value::<RevocationEntry>(revocation.clone()) {
                            // Check if this revocation applies to our credential
                            match entry.revocation_type {
                                RevocationType::Credential => {
                                    if entry.target_id == credential.id {
                                        return Ok(true); // Credential directly revoked
                                    }
                                },
                                RevocationType::Issuer => {
                                    if entry.target_id == credential.issuer {
                                        return Ok(true); // Issuer has been revoked
                                    }
                                },
                                RevocationType::Subject => {
                                    if entry.target_id == credential.credentialSubject.id {
                                        return Ok(true); // Subject has been revoked
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
    
    // No applicable revocations found
    Ok(false)
}

/// Verify the trust policy lineage starting from a specific CID
fn verify_policy_lineage(dag_store: &impl DagStore, policy_cid_str: &str) -> Result<bool> {
    // Parse the CID string
    // 1. Decode from multibase string (e.g., "bafy...") to bytes
    let (_base, decoded_bytes) = multibase::decode(policy_cid_str)
        .map_err(|e| anyhow!("Invalid multibase encoding for policy CID string '{}': {}", policy_cid_str, e))?;
    
    // 2. Create Cid from decoded bytes
    let cid = Cid::from_bytes(&decoded_bytes)
        .map_err(|e| anyhow!("Invalid policy CID bytes from string '{}': {}", policy_cid_str, e))?;
    
    // Get the node from the DAG
    let node = futures::executor::block_on(dag_store.get_node(&cid))
        .context("Failed to get policy node from DAG")?;
    
    // Check if it's a TrustPolicyRecord
    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
        if payload.get("type").and_then(|t| t.as_str()) == Some("TrustPolicyRecord") {
            // Check the signature on this node
            // (In a full implementation we would verify the node signature here)
            
            // Extract the policy credential
            if let Some(credential_value) = payload.get("policy") {
                // Parse the credential
                let credential: TrustPolicyCredential = serde_json::from_value(credential_value.clone())
                    .context("Failed to parse trust policy credential")?;
                
                // If this policy has a previous one, recursively verify it
                if let Some(prev_cid_str) = &credential.credentialSubject.previousPolicyId {
                    // Recursively verify the previous policy
                    return verify_policy_lineage(dag_store, prev_cid_str);
                } else {
                    // This is a root policy, no previous to verify
                    return Ok(true);
                }
            }
        }
    }
    
    // Not a valid policy node
    Ok(false)
}

/// Credential subject data for trust policy
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustPolicyCredential {
    /// Credential context for JSON-LD
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (unique identifier)
    pub id: String,
    
    /// Credential type
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    
    /// Issuer DID (admin that created the policy)
    pub issuer: String,
    
    /// Issuance date
    pub issuanceDate: DateTime<Utc>,
    
    /// Credential subject (trust policy)
    pub credentialSubject: TrustPolicySubject,
    
    /// Cryptographic proof
    pub proof: Option<TrustPolicyProof>,
}

/// Subject of the trust policy credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustPolicySubject {
    /// Federation ID this policy applies to
    pub federationId: String,
    
    /// Previous policy CID if this is an update
    pub previousPolicyId: Option<String>,
    
    /// Other fields not needed for verification
    #[serde(flatten)]
    pub other: serde_json::Value,
}

/// Cryptographic proof for the trust policy
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustPolicyProof {
    /// Proof type (e.g., Ed25519Signature2020)
    #[serde(rename = "type")]
    pub proof_type: String,
    
    /// Verification method identifier
    pub verificationMethod: String,
    
    /// Creation date of the proof
    pub created: DateTime<Utc>,
    
    /// Hex-encoded signature value
    pub proofValue: String,
}

/// Create a simplified JSON verification result suitable for APIs
pub fn verify_dispatch_credential_json(json: &str) -> Result<String> {
    // This function would integrate with the loaded DAG and policy stores
    // For now, we'll create a placeholder implementation
    
    // Parse the credential to extract basic information
    let credential: DispatchCredential = serde_json::from_str(json)
        .context("Failed to parse dispatch credential from JSON")?;
    
    // Create a simplified verification report
    let report = VerificationReport {
        issuer_did: credential.issuer.clone(),
        signature_valid: true, // Simplified - would call verify_credential_signature
        is_trusted: true,      // Simplified - would check against trust policy
        is_revoked: false,     // Simplified - would check revocation status
        policy_version: "local".to_string(),
        lineage_verified: true, // Simplified - would verify policy lineage
        overall_valid: true,    // Simplified
        capability_match: None,
        error: None,
        timestamp: Utc::now(),
    };
    
    // Serialize to JSON
    let result_json = serde_json::to_string(&report)
        .context("Failed to serialize verification report")?;
    
    Ok(result_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[test]
    fn test_verification_report_serialization() {
        let report = VerificationReport {
            issuer_did: "did:icn:scheduler123".to_string(),
            signature_valid: true,
            is_trusted: true,
            is_revoked: false,
            policy_version: "QmPolicyHash".to_string(),
            lineage_verified: true,
            overall_valid: true,
            capability_match: Some(true),
            error: None,
            timestamp: Utc::now(),
        };
        
        let json = serde_json::to_string_pretty(&report).unwrap();
        
        // Should include all required fields
        assert!(json.contains("issuer_did"));
        assert!(json.contains("signature_valid"));
        assert!(json.contains("is_trusted"));
        assert!(json.contains("is_revoked"));
        
        // Deserialize back
        let deserialized: VerificationReport = serde_json::from_str(&json).unwrap();
        
        // Check key fields
        assert_eq!(report.issuer_did, deserialized.issuer_did);
        assert_eq!(report.signature_valid, deserialized.signature_valid);
        assert_eq!(report.is_trusted, deserialized.is_trusted);
        assert_eq!(report.overall_valid, deserialized.overall_valid);
    }
} 