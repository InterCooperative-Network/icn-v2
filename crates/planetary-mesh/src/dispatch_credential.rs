use anyhow::{Result, anyhow, Context};
use chrono::{DateTime, Utc};
use icn_core_types::Did;
use icn_identity_core::did::DidKey;
use icn_types::dag::{DagStore, Cid, DagPayload, SignedDagNode};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use log::{debug, info, warn, error};
use crate::cap_index::CapabilitySelector;
use multibase::{Base, encode, decode};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};

/// W3C Verifiable Credential for a dispatch decision
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

/// Type of verification result for dispatch credentials
#[derive(Debug, PartialEq)]
pub enum VerificationStatus {
    /// Credential signature is valid
    Valid,
    
    /// Credential has no signature
    Unsigned,
    
    /// Credential signature is invalid
    Invalid,
    
    /// Credential matches DAG record
    MatchesDag,
    
    /// Credential doesn't match DAG record
    DagMismatch,
}

impl DispatchCredential {
    /// Create a new dispatch credential with default context and type
    pub fn new(id: String, issuer: String, subject: DispatchCredentialSubject) -> Self {
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://icn.network/context/mesh-compute/v1".to_string(),
            ],
            id,
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "DispatchReceipt".to_string(),
            ],
            issuer,
            issuanceDate: Utc::now(),
            credentialSubject: subject,
            proof: None,
        }
    }
    
    /// Sign the credential with a DID key
    pub fn sign(&mut self, did_key: &DidKey) -> Result<()> {
        // Store the current issuance date
        let issuance_date = self.issuanceDate;
        
        // Remove any existing proof before signing
        let temp_credential = Self {
            context: self.context.clone(),
            id: self.id.clone(),
            credential_type: self.credential_type.clone(),
            issuer: self.issuer.clone(),
            issuanceDate: issuance_date,
            credentialSubject: self.credentialSubject.clone(),
            proof: None,
        };
        
        // Convert to canonical form for signing
        let canonical_bytes = serde_json::to_vec(&temp_credential)
            .context("Failed to serialize credential for signing")?;
        
        // Sign the credential
        let signature = did_key.sign(&canonical_bytes);
        
        // Create proof
        self.proof = Some(DispatchCredentialProof {
            proof_type: "Ed25519Signature2020".to_string(),
            verificationMethod: format!("{}#keys-1", did_key.did()),
            created: issuance_date,
            proofValue: hex::encode(signature.to_bytes()),
        });
        
        Ok(())
    }
    
    /// Verify the credential's signature using DID resolution
    pub fn verify(&self) -> Result<VerificationStatus> {
        if self.proof.is_none() {
            return Ok(VerificationStatus::Unsigned);
        }
        
        let proof = self.proof.as_ref().unwrap();
        
        // Extract DID from the issuer
        let issuer_did = Did::from(self.issuer.clone());
        
        // Create temporary credential without proof for verification
        let temp_credential = Self {
            context: self.context.clone(),
            id: self.id.clone(),
            credential_type: self.credential_type.clone(),
            issuer: self.issuer.clone(),
            issuanceDate: self.issuanceDate,
            credentialSubject: self.credentialSubject.clone(),
            proof: None,
        };
        
        // Get canonical form for verification
        let canonical_bytes = serde_json::to_vec(&temp_credential)
            .context("Failed to serialize credential for verification")?;
        
        // Extract public key from issuer DID
        // In a real implementation, this would use a DID resolver
        // Here we do a basic check for did:key format
        if !self.issuer.starts_with("did:key:z") {
            return Err(anyhow!("Only did:key DIDs are supported for verification"));
        }
        
        // Extract the key part
        let key_part = self.issuer.trim_start_matches("did:key:");
        
        // Decode the multibase encoding
        let multibase_decoded = multibase::decode(key_part)
            .map_err(|e| anyhow!("Failed to decode key part: {}", e))?;
        
        // Check for Ed25519 prefix (0xed01)
        if multibase_decoded.len() < 2 || multibase_decoded[0] != 0xed || multibase_decoded[1] != 0x01 {
            return Err(anyhow!("Unsupported key type, expected Ed25519"));
        }
        
        // Extract public key bytes
        let key_bytes = &multibase_decoded[2..];
        if key_bytes.len() != 32 {
            return Err(anyhow!("Invalid key length"));
        }
        
        // Create verifying key
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(key_bytes.try_into().unwrap())
            .map_err(|e| anyhow!("Invalid public key: {}", e))?;
        
        // Decode signature
        let signature_bytes = hex::decode(&proof.proofValue)
            .context("Failed to decode signature")?;
        
        if signature_bytes.len() != 64 {
            return Err(anyhow!("Invalid signature length"));
        }
        
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes)
            .map_err(|_| anyhow!("Invalid signature format"))?;
        
        // Verify signature
        match verifying_key.verify(&canonical_bytes, &signature) {
            Ok(_) => Ok(VerificationStatus::Valid),
            Err(_) => Ok(VerificationStatus::Invalid),
        }
    }
    
    /// Verify the credential against a DAG record
    pub async fn verify_against_dag(
        &self,
        dag_store: &Arc<Box<dyn DagStore>>,
        cid: &Cid,
    ) -> Result<VerificationStatus> {
        // First verify the signature
        let sig_status = self.verify()?;
        if sig_status != VerificationStatus::Valid {
            return Ok(sig_status);
        }
        
        // Get the node from the DAG
        let node = dag_store.get_node(cid).await
            .context("Failed to get dispatch record from DAG")?;
        
        // Check if it's a DispatchAuditRecord
        if let DagPayload::Json(payload) = &node.node.payload {
            if payload.get("type").and_then(|t| t.as_str()) == Some("DispatchAuditRecord") {
                // Extract the embedded credential
                if let Some(credential) = payload.get("credential") {
                    // Compare the credential to what we have
                    let dag_credential: DispatchCredential = serde_json::from_value(credential.clone())
                        .context("Failed to parse credential from DAG")?;
                    
                    // Compare critical fields
                    if self.id != dag_credential.id ||
                       self.issuer != dag_credential.issuer ||
                       self.credentialSubject.id != dag_credential.credentialSubject.id ||
                       self.credentialSubject.selectedNode != dag_credential.credentialSubject.selectedNode {
                        return Ok(VerificationStatus::DagMismatch);
                    }
                    
                    return Ok(VerificationStatus::MatchesDag);
                }
            }
        }
        
        Err(anyhow!("Node is not a DispatchAuditRecord or lacks credential"))
    }
}

/// REST API handler to fetch the latest dispatch credentials
pub async fn get_latest_dispatch_credentials(
    dag_store: Arc<Box<dyn DagStore>>,
    federation_id: String,
    limit: usize,
) -> Result<Vec<(Cid, DispatchCredential)>> {
    let nodes = dag_store.get_ordered_nodes().await
        .context("Failed to get DAG nodes")?;
    
    let mut credentials = Vec::new();
    
    for node in nodes {
        // Skip nodes from different federations
        if node.node.federation_id != federation_id {
            continue;
        }
        
        if let DagPayload::Json(payload) = &node.node.payload {
            if payload.get("type").and_then(|t| t.as_str()) == Some("DispatchAuditRecord") {
                if let Some(credential) = payload.get("credential") {
                    if let Ok(dispatch_cred) = serde_json::from_value(credential.clone()) {
                        if let Some(cid) = &node.cid {
                            credentials.push((cid.clone(), dispatch_cred));
                            
                            if credentials.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Sort by issuance date (newest first)
    credentials.sort_by(|(_, a), (_, b)| b.issuanceDate.cmp(&a.issuanceDate));
    
    Ok(credentials)
}

/// Create a simple HTTP handler for serving dispatch credentials
#[cfg(feature = "http-api")]
pub mod http_api {
    use super::*;
    use hyper::{Body, Request, Response, StatusCode};
    use hyper::service::{make_service_fn, service_fn};
    use std::net::SocketAddr;
    use std::convert::Infallible;
    use url::Url;
    
    /// Start a simple HTTP API server for dispatches
    pub async fn start_dispatch_api_server(
        addr: SocketAddr,
        dag_store: Arc<Box<dyn DagStore>>,
        federation_id: String,
    ) -> Result<()> {
        info!("Starting dispatch API server on http://{}", addr);
        
        let service = make_service_fn(move |_| {
            let dag_store = dag_store.clone();
            let federation_id = federation_id.clone();
            
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let dag_store = dag_store.clone();
                    let federation_id = federation_id.clone();
                    
                    async move {
                        handle_request(req, dag_store, federation_id).await
                    }
                }))
            }
        });
        
        let server = hyper::Server::bind(&addr).serve(service);
        
        info!("Dispatch API server listening on http://{}", addr);
        
        server.await
            .map_err(|e| anyhow!("API server error: {}", e))?;
            
        Ok(())
    }
    
    /// Handle an HTTP request
    async fn handle_request(
        req: Request<Body>,
        dag_store: Arc<Box<dyn DagStore>>,
        federation_id: String,
    ) -> Result<Response<Body>, Infallible> {
        let path = req.uri().path();
        
        match (req.method().as_str(), path) {
            ("GET", "/api/dispatches/latest") => {
                // Extract limit parameter if present
                let query = req.uri().query().unwrap_or("");
                let parsed_url = Url::parse(&format!("http://example.com{}?{}", path, query))
                    .unwrap_or_else(|_| Url::parse("http://example.com").unwrap());
                
                let limit = parsed_url.query_pairs()
                    .find(|(key, _)| key == "limit")
                    .and_then(|(_, value)| value.parse::<usize>().ok())
                    .unwrap_or(10);
                
                match get_latest_dispatch_credentials(dag_store, federation_id, limit).await {
                    Ok(credentials) => {
                        let response = Response::builder()
                            .header("Content-Type", "application/json")
                            .body(Body::from(serde_json::to_string(&credentials).unwrap()))
                            .unwrap();
                        Ok(response)
                    },
                    Err(e) => {
                        let response = Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from(format!("{{\"error\":\"{}\"}}", e)))
                            .unwrap();
                        Ok(response)
                    }
                }
            },
            ("GET", path) if path.starts_with("/api/dispatches/") => {
                // Extract CID from path
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() != 4 {
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(r#"{"error":"Invalid dispatch CID path"}"#))
                        .unwrap());
                }
                
                let cid_str = parts[3];
                
                // Parse CID
                match icn_types::cid::Cid::from_str(cid_str) {
                    Ok(cid) => {
                        // Get node from DAG
                        match dag_store.get_node(&cid).await {
                            Ok(node) => {
                                if let DagPayload::Json(payload) = &node.node.payload {
                                    if let Some(credential) = payload.get("credential") {
                                        let response = Response::builder()
                                            .header("Content-Type", "application/json")
                                            .body(Body::from(serde_json::to_string(credential).unwrap()))
                                            .unwrap();
                                        return Ok(response);
                                    }
                                }
                                
                                // Node doesn't have a credential
                                let response = Response::builder()
                                    .status(StatusCode::NOT_FOUND)
                                    .body(Body::from(r#"{"error":"Node is not a dispatch record"}"#))
                                    .unwrap();
                                Ok(response)
                            },
                            Err(_) => {
                                let response = Response::builder()
                                    .status(StatusCode::NOT_FOUND)
                                    .body(Body::from(r#"{"error":"Dispatch not found"}"#))
                                    .unwrap();
                                Ok(response)
                            }
                        }
                    },
                    Err(_) => {
                        let response = Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from(r#"{"error":"Invalid CID format"}"#))
                            .unwrap();
                        Ok(response)
                    }
                }
            },
            _ => {
                // 404 for other paths
                let response = Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from(r#"{"error":"Endpoint not found"}"#))
                    .unwrap();
                Ok(response)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use icn_identity_core::did::DidKey;
    
    fn create_test_credential() -> (DispatchCredential, DidKey) {
        let did_key = DidKey::new();
        let did_str = did_key.did().to_string();
        
        let subject = DispatchCredentialSubject {
            id: "did:icn:requestor123".to_string(),
            taskRequest: TaskRequestDetails {
                wasm_hash: "0xabcdef1234567890".to_string(),
                wasm_size: 1048576,
                inputs: vec!["ipfs://QmData1".to_string(), "ipfs://QmData2".to_string()],
                max_latency_ms: 500,
                memory_mb: 2048,
                cores: 4,
                priority: 50,
                timestamp: Utc::now(),
                federation_id: "test-federation".to_string(),
            },
            capabilities: CapabilitySelector::default(),
            selectedNode: "did:icn:node456".to_string(),
            score: 0.92,
            dispatchTime: Utc::now(),
            matchingNodeCount: 5,
            bid: BidDetails {
                bidCid: "QmBidHash1234".to_string(),
                latency: 25,
                memory: 16384,
                cores: 8,
                reputation: 95,
                renewable: 80,
            },
        };
        
        let credential = DispatchCredential::new(
            format!("urn:icn:dispatch:{}", uuid::Uuid::new_v4()),
            did_str,
            subject,
        );
        
        (credential, did_key)
    }
    
    #[test]
    fn test_credential_serialization() {
        let (credential, _) = create_test_credential();
        
        // Serialize to JSON
        let json = serde_json::to_string_pretty(&credential).unwrap();
        
        // Should include all required fields
        assert!(json.contains("@context"));
        assert!(json.contains("VerifiableCredential"));
        assert!(json.contains("DispatchReceipt"));
        assert!(json.contains("credentialSubject"));
        assert!(json.contains("selectedNode"));
        
        // Deserialize back
        let deserialized: DispatchCredential = serde_json::from_str(&json).unwrap();
        
        // Check key fields
        assert_eq!(credential.id, deserialized.id);
        assert_eq!(credential.issuer, deserialized.issuer);
        assert_eq!(credential.credentialSubject.id, deserialized.credentialSubject.id);
        assert_eq!(credential.credentialSubject.selectedNode, deserialized.credentialSubject.selectedNode);
    }
    
    #[test]
    fn test_credential_signing_and_verification() {
        let (mut credential, did_key) = create_test_credential();
        
        // Initially unsigned
        assert!(credential.proof.is_none());
        
        // Sign the credential
        credential.sign(&did_key).unwrap();
        
        // Now should have a proof
        assert!(credential.proof.is_some());
        
        // Verify the signature
        let result = credential.verify().unwrap();
        assert_eq!(result, VerificationStatus::Valid);
        
        // Tamper with the credential
        let mut tampered = credential.clone();
        tampered.credentialSubject.selectedNode = "did:icn:attacker".to_string();
        
        // Verification should fail
        let result = tampered.verify().unwrap();
        assert_eq!(result, VerificationStatus::Invalid);
    }
    
    #[test]
    fn test_unsigned_credential_verification() {
        let (credential, _) = create_test_credential();
        
        // Verify without signing
        let result = credential.verify().unwrap();
        assert_eq!(result, VerificationStatus::Unsigned);
    }
} 