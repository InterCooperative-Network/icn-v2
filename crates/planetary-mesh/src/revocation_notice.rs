use anyhow::{Result, anyhow, Context};
use icn_core_types::Did;
use icn_identity_core::did::DidKey;
use icn_core_types::Cid;
use icn_types::dag::{DagStore, DagPayload, SignedDagNode, DagNodeBuilder, SharedDagStore};
use serde::{Serialize, Deserialize};
use log::{debug, info, warn, error};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use multibase::{Base, encode, decode};
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// W3C Verifiable Credential for DID or credential revocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationNoticeCredential {
    /// Credential context for JSON-LD
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (unique identifier)
    pub id: String,
    
    /// Credential type
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    
    /// Issuer DID (authority issuing the revocation)
    pub issuer: String,
    
    /// Issuance date
    pub issuanceDate: DateTime<Utc>,
    
    /// Credential subject (the details of the revocation)
    pub credentialSubject: RevocationSubject,
    
    /// Credential proof
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<CredentialProof>,
}

/// Revocation subject information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationSubject {
    /// Federation ID
    pub federationId: String,
    
    /// DID being revoked (if revoking an entire DID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revokedDid: Option<String>,
    
    /// CID of the credential being revoked (if revoking a specific credential)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revokedCredentialCid: Option<String>,
    
    /// Revocation reason
    pub reason: String,
    
    /// Revocation effective date
    pub effectiveDate: DateTime<Utc>,
}

/// Credential proof information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialProof {
    /// Type of proof
    #[serde(rename = "type")]
    pub proof_type: String,
    
    /// Creation date of the proof
    pub created: DateTime<Utc>,
    
    /// Verification method
    pub verificationMethod: String,
    
    /// Proof purpose
    pub proofPurpose: String,
    
    /// Proof value (signature)
    pub proofValue: String,
}

/// DAG record for a revocation notice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationRecord {
    /// Record type
    #[serde(rename = "type")]
    pub record_type: String,
    
    /// Federation ID
    pub federation_id: String,
    
    /// The revocation credential
    pub notice: RevocationNoticeCredential,
}

impl RevocationRecord {
    /// Create a new revocation record
    pub fn new(federation_id: String, notice: RevocationNoticeCredential) -> Self {
        Self {
            record_type: "RevocationRecord".to_string(),
            federation_id,
            notice,
        }
    }
    
    /// Convert to DAG payload
    pub fn to_dag_payload(&self) -> Result<DagPayload> {
        let json = serde_json::to_value(self)?;
        Ok(DagPayload::Json(json))
    }
}

impl RevocationNoticeCredential {
    /// Create a new revocation notice for a DID
    pub fn new_did_revocation(
        federation_id: String,
        revoked_did: String,
        reason: String,
        issuer: String,
    ) -> Self {
        let now = Utc::now();
        
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://w3id.org/security/suites/ed25519-2020/v1".to_string(),
                "https://intercooperative.org/credentials/revocation/v1".to_string(),
            ],
            id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "RevocationNotice".to_string(),
            ],
            issuer,
            issuanceDate: now,
            credentialSubject: RevocationSubject {
                federationId: federation_id,
                revokedDid: Some(revoked_did),
                revokedCredentialCid: None,
                reason,
                effectiveDate: now,
            },
            proof: None,
        }
    }
    
    /// Create a new revocation notice for a credential
    pub fn new_credential_revocation(
        federation_id: String,
        revoked_credential_cid: String,
        reason: String,
        issuer: String,
    ) -> Self {
        let now = Utc::now();
        
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://w3id.org/security/suites/ed25519-2020/v1".to_string(),
                "https://intercooperative.org/credentials/revocation/v1".to_string(),
            ],
            id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "RevocationNotice".to_string(),
            ],
            issuer,
            issuanceDate: now,
            credentialSubject: RevocationSubject {
                federationId: federation_id,
                revokedDid: None,
                revokedCredentialCid: Some(revoked_credential_cid),
                reason,
                effectiveDate: now,
            },
            proof: None,
        }
    }
    
    /// Sign this credential
    pub fn sign(&mut self, did_key: &DidKey) -> Result<()> {
        // Create a canonical form for signing
        let temp = Self {
            context: self.context.clone(),
            id: self.id.clone(),
            credential_type: self.credential_type.clone(),
            issuer: self.issuer.clone(),
            issuanceDate: self.issuanceDate,
            credentialSubject: self.credentialSubject.clone(),
            proof: None,
        };
        
        // Convert to bytes for signing
        let canonical_bytes = serde_json::to_vec(&temp)?;
        
        // Sign the bytes
        let signature = did_key.sign(&canonical_bytes);
        
        // For proofValue, convert signature to hex string
        let signature_bytes = signature.to_bytes();
        let proof = CredentialProof {
            proof_type: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verificationMethod: format!("{}#key-1", did_key.did()),
            proofPurpose: "assertionMethod".to_string(),
            proofValue: hex::encode(signature_bytes),
        };
        
        // Set the proof
        self.proof = Some(proof);
        
        Ok(())
    }
    
    /// Verify the signature on this credential
    pub fn verify(&self) -> Result<bool> {
        if self.proof.is_none() {
            return Ok(false);
        }
        
        let proof = self.proof.as_ref().unwrap();
        
        // Create a canonical form for verification
        let temp = Self {
            context: self.context.clone(),
            id: self.id.clone(),
            credential_type: self.credential_type.clone(),
            issuer: self.issuer.clone(),
            issuanceDate: self.issuanceDate,
            credentialSubject: self.credentialSubject.clone(),
            proof: None,
        };
        
        // Convert to bytes for verification
        let canonical_bytes = serde_json::to_vec(&temp)?;
        
        // Extract the DID key from the issuer
        if !self.issuer.starts_with("did:key:") {
            return Err(anyhow!("Only did:key DIDs are supported for verification"));
        }
        
        // Extract the key part
        let key_part = self.issuer.trim_start_matches("did:key:");
        
        // Decode the multibase encoding
        let multibase_decoded = multibase::decode(key_part)
            .map_err(|e| anyhow!("Failed to decode key part: {}", e))?;
        
        // Check for Ed25519 prefix (0xed01)
        if multibase_decoded.1.len() < 2 || multibase_decoded.1[0] != 0xed || multibase_decoded.1[1] != 0x01 {
            return Err(anyhow!("Unsupported key type, expected Ed25519"));
        }
        
        // Extract public key bytes
        let key_bytes = &multibase_decoded.1[2..];
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
        
        // Create a 64-byte array for Signature::from_bytes
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&signature_bytes);
        let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        
        // Verify signature
        match verifying_key.verify(&canonical_bytes, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Anchor this revocation notice to the DAG
    pub async fn anchor_to_dag(
        &self,
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        did_key: &DidKey,
    ) -> Result<Cid> {
        // Create a record for DAG storage
        let record = RevocationRecord::new(
            self.credentialSubject.federationId.clone(),
            self.clone(),
        );
        
        // Convert to DAG payload
        let payload = record.to_dag_payload()?;
        
        // Create a DAG node for the revocation notice
        let node = DagNodeBuilder::new()
            .with_payload(payload)
            .with_author(did_key.did().clone())
            .with_federation_id(self.credentialSubject.federationId.clone())
            .with_label("RevocationNotice".to_string())
            .build()?;
        
        // Serialize the node for signing
        let node_bytes = serde_json::to_vec(&node)
            .context("Failed to serialize node")?;
        
        // Sign the node
        let signature = did_key.sign(&node_bytes);
        
        // Create a signed node
        let mut signed_node = SignedDagNode {
            node,
            signature,
            cid: None,
        };
        
        // Ensure CID is calculated
        signed_node.ensure_cid()?;
        
        // Add to the DAG store to get its CID
        let shared_store = SharedDagStore::from_arc(dag_store.clone())?;
        let cid = shared_store.add_node(signed_node)
            .await?;
            
        Ok(cid)
    }
}

/// Check if a DID has been revoked
pub async fn is_did_revoked(
    dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
    did: &str,
    federation_id: &str,
) -> Result<bool> {
    // Get all nodes from the DAG
    let nodes = dag_store.get_ordered_nodes().await?;
    
    // Filter for revocation notices in this federation
    for node in nodes {
        // Skip nodes from different federations
        if node.node.metadata.federation_id != federation_id {
            continue;
        }
        
        // Check if this is a revocation notice node
        if let Some(label) = &node.node.metadata.label {
            if label != "RevocationNotice" {
                continue;
            }
        } else {
            continue;
        }
        
        if let DagPayload::Json(payload) = &node.node.payload {
            if payload.get("type").and_then(|t| t.as_str()) != Some("RevocationRecord") {
                continue;
            }
            
            if let Some(notice_value) = payload.get("notice") {
                if let Ok(notice) = serde_json::from_value::<RevocationNoticeCredential>(notice_value.clone()) {
                    // Verify the notice
                    if !notice.verify()? {
                        continue;
                    }
                    
                    // Check if this notice revokes the DID
                    if let Some(revoked_did) = &notice.credentialSubject.revokedDid {
                        if revoked_did == did {
                            // Check if the effective date is in the past
                            if notice.credentialSubject.effectiveDate <= Utc::now() {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // No valid revocation found
    Ok(false)
}

/// Check if a credential has been revoked
pub async fn is_credential_revoked(
    dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
    credential_cid: &str,
    federation_id: &str,
) -> Result<bool> {
    // Get all nodes from the DAG
    let nodes = dag_store.get_ordered_nodes().await?;
    
    // Filter for revocation notices in this federation
    for node in nodes {
        // Skip nodes from different federations
        if node.node.metadata.federation_id != federation_id {
            continue;
        }
        
        // Check if this is a revocation notice node
        if let Some(label) = &node.node.metadata.label {
            if label != "RevocationNotice" {
                continue;
            }
        } else {
            continue;
        }
        
        if let DagPayload::Json(payload) = &node.node.payload {
            if payload.get("type").and_then(|t| t.as_str()) != Some("RevocationRecord") {
                continue;
            }
            
            if let Some(notice_value) = payload.get("notice") {
                if let Ok(notice) = serde_json::from_value::<RevocationNoticeCredential>(notice_value.clone()) {
                    // Verify the notice
                    if !notice.verify()? {
                        continue;
                    }
                    
                    // Check if this notice revokes the credential
                    if let Some(revoked_cid) = &notice.credentialSubject.revokedCredentialCid {
                        if revoked_cid == credential_cid {
                            // Check if the effective date is in the past
                            if notice.credentialSubject.effectiveDate <= Utc::now() {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // No valid revocation found
    Ok(false)
}

/// Issue a DID revocation notice
pub async fn revoke_did(
    dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
    federation_id: &str,
    did_to_revoke: &str,
    reason: &str,
    issuer_did: &str,
    issuer_key: &DidKey,
) -> Result<Cid> {
    // Create a revocation notice
    let mut notice = RevocationNoticeCredential::new_did_revocation(
        federation_id.to_string(),
        did_to_revoke.to_string(),
        reason.to_string(),
        issuer_did.to_string(),
    );
    
    // Sign the notice
    notice.sign(issuer_key)?;
    
    // Anchor to DAG
    notice.anchor_to_dag(dag_store, issuer_key).await
}

/// Issue a credential revocation notice
pub async fn revoke_credential(
    dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
    federation_id: &str,
    credential_cid: &str,
    reason: &str,
    issuer_did: &str,
    issuer_key: &DidKey,
) -> Result<Cid> {
    // Create a revocation notice
    let mut notice = RevocationNoticeCredential::new_credential_revocation(
        federation_id.to_string(),
        credential_cid.to_string(),
        reason.to_string(),
        issuer_did.to_string(),
    );
    
    // Sign the notice
    notice.sign(issuer_key)?;
    
    // Anchor to DAG
    notice.anchor_to_dag(dag_store, issuer_key).await
}

// Helper function to verify a signature
fn verify_signature(public_key_multibase: &str, message: &[u8], signature_multibase: &str) -> Result<bool> {
    // Decode the public key from multibase
    let multibase_decoded = decode(public_key_multibase)
        .map_err(|_| anyhow!("Invalid public key format"))?;
    
    // Check if it's an Ed25519 key
    if multibase_decoded.1.len() < 2 || multibase_decoded.1[0] != 0xed || multibase_decoded.1[1] != 0x01 {
        return Err(anyhow!("Invalid public key format - not Ed25519"));
    }
    
    // Extract the key bytes
    let key_bytes = &multibase_decoded.1[2..];
    
    // Create the verifying key
    let bytes32: [u8; 32] = key_bytes.try_into()
        .map_err(|_| anyhow!("Invalid key length"))?;
        
    let verifying_key = VerifyingKey::from_bytes(&bytes32)
        .map_err(|_| anyhow!("Invalid public key"))?;
    
    // Decode the signature from multibase
    let signature_bytes = decode(signature_multibase)
        .map_err(|_| anyhow!("Invalid signature format"))?.1;
    
    // Need exactly 64 bytes for an Ed25519 signature
    if signature_bytes.len() != 64 {
        return Err(anyhow!("Invalid signature length"));
    }
    
    // Create a 64-byte array from the signature bytes
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&signature_bytes[0..64]);
    
    // Create the signature object
    let signature = Signature::from_bytes(&sig_bytes);
    
    // Verify the signature
    verifying_key.verify_strict(message, &signature)
        .map(|_| true)
        .or_else(|_| Ok(false))
} 