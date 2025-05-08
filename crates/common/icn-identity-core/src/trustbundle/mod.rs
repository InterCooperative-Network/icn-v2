use serde::{Deserialize, Serialize};
use thiserror::Error;
use icn_types::dag::{DagEvent, EventId};
use icn_core_types::{Did, Cid};
use std::collections::HashMap;
use ed25519_dalek::{VerifyingKey, Signature, Verifier, SIGNATURE_LENGTH};
use sha2::{Sha256, Digest};
use std::convert::TryInto;

pub mod storage;

/// Errors related to TrustBundle operations
#[derive(Error, Debug)]
pub enum TrustError {
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    
    #[error("insufficient quorum: required {required}, found {found}")]
    InsufficientQuorum { required: usize, found: usize },
    
    #[error("invalid event reference: {0}")]
    InvalidEvent(String),
    
    #[error("serialization error: {0}")]
    SerializationError(String),
    
    #[error("public key not found for DID: {0}")]
    PublicKeyNotFound(Did),
    
    #[error("unknown error: {0}")]
    Unknown(String),
    
    #[error("duplicate signature: {0}")]
    DuplicateSignature(String),
}

/// Types of quorum requirements
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum QuorumType {
    /// Simple majority (>50%) of participants
    Majority,
    
    /// Specific percentage threshold (0-100)
    Threshold(u8),
    
    /// Weighted voting where each DID has an assigned weight
    Weighted(Vec<(Did, u64)>),
    
    /// All participants must sign
    All,
}

/// Configuration for quorum validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuorumConfig {
    /// The type of quorum required
    pub quorum_type: QuorumType,
    
    /// DIDs of the participants
    pub participants: Vec<Did>,
}

impl QuorumConfig {
    /// Calculate the required number of signatures based on the quorum type
    pub fn required_signatures(&self) -> usize {
        match &self.quorum_type {
            QuorumType::Majority => (self.participants.len() / 2) + 1,
            QuorumType::Threshold(percent) => {
                let threshold = (*percent as usize * self.participants.len()) / 100;
                if threshold == 0 && !self.participants.is_empty() {
                    1 // At least one signature if there are participants
                } else {
                    threshold
                }
            },
            QuorumType::Weighted(_) => {
                // For weighted, we'll check the actual weights during verification
                1 // Return minimum here, actual check is more complex
            },
            QuorumType::All => self.participants.len(),
        }
    }
}

/// Cryptographic proof that a quorum of participants signed a bundle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuorumProof {
    /// (DID, signature) pairs
    pub signatures: Vec<(Did, Vec<u8>)>,
}

impl QuorumProof {
    /// Create a new empty QuorumProof
    pub fn new() -> Self {
        QuorumProof {
            signatures: Vec::new(),
        }
    }
    
    /// Add a signature to the proof
    pub fn add_signature(&mut self, did: Did, signature: Vec<u8>) {
        self.signatures.push((did, signature));
    }
    
    /// Verify the proof against a quorum config and message digest
    pub fn verify(
        &self, 
        config: &QuorumConfig, 
        message_digest: &[u8],
        public_keys: &HashMap<Did, VerifyingKey>,
    ) -> Result<(), TrustError> {
        let mut valid_signers = Vec::new();
        
        // Verify each signature
        for (did, sig_bytes) in &self.signatures {
            // Skip signatures from DIDs that aren't in the participants list
            if !config.participants.contains(did) {
                continue;
            }
            
            // Get the public key for this DID
            let verifying_key = public_keys.get(did)
                .ok_or_else(|| TrustError::PublicKeyNotFound(did.clone()))?;
            
            // Convert to ed25519_dalek::Signature
            let signature_array: [u8; SIGNATURE_LENGTH] = sig_bytes.as_slice().try_into()
                .map_err(|_| TrustError::InvalidSignature("Signature is not 64 bytes long".to_string()))?;
            let signature = Signature::from_bytes(&signature_array);
            
            // Verify the signature
            verifying_key.verify(message_digest, &signature)
                .map_err(|e| TrustError::InvalidSignature(e.to_string()))?;
            
            if valid_signers.contains(&did) {
                return Err(TrustError::DuplicateSignature(did.to_string()));
            }
            
            valid_signers.push(did);
        }
        
        // Check if we have enough valid signatures based on the quorum type
        match &config.quorum_type {
            QuorumType::Majority | QuorumType::Threshold(_) | QuorumType::All => {
                let required = config.required_signatures();
                let found = valid_signers.len();
                
                if found < required {
                    return Err(TrustError::InsufficientQuorum { required, found });
                }
            },
            QuorumType::Weighted(weights) => {
                // Calculate the total weight of valid signers
                let mut total_weight = 0;
                let mut max_possible_weight = 0;
                
                for (did, weight) in weights {
                    max_possible_weight += weight;
                    if valid_signers.contains(&did) {
                        total_weight += weight;
                    }
                }
                
                // Require majority of total weight
                let required_weight = (max_possible_weight / 2) + 1;
                if total_weight < required_weight {
                    return Err(TrustError::InsufficientQuorum { 
                        required: required_weight as usize, 
                        found: total_weight as usize 
                    });
                }
            }
        }
        
        Ok(())
    }
}

/// A container for federation-verified DAG events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustBundle {
    /// Federation identifier
    pub federation_id: String,
    
    /// References to the events included in this bundle
    pub referenced_events: Vec<EventId>,
    
    /// Optional CID for this bundle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_cid: Option<String>,
    
    /// Quorum configuration for this federation
    pub quorum_config: QuorumConfig,
    
    /// Cryptographic proof of quorum
    pub proof: QuorumProof,
    
    /// Timestamp when this bundle was created
    pub timestamp: u64,
    
    /// Additional metadata as key-value pairs
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl TrustBundle {
    /// Create a new TrustBundle
    pub fn new(
        federation_id: String,
        referenced_events: Vec<EventId>,
        quorum_config: QuorumConfig,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        TrustBundle {
            federation_id,
            referenced_events,
            bundle_cid: None,
            quorum_config,
            proof: QuorumProof::new(),
            timestamp,
            metadata: HashMap::new(),
        }
    }
    
    /// Add metadata to this bundle
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Calculate the hash of this bundle (excluding proof)
    pub fn calculate_hash(&self) -> [u8; 32] {
        // Create a clone without the proof for hashing
        let bundle_for_hash = TrustBundleForHash {
            federation_id: &self.federation_id,
            referenced_events: &self.referenced_events,
            quorum_config: &self.quorum_config,
            timestamp: self.timestamp,
            metadata: &self.metadata,
        };
        
        // Serialize and hash
        let serialized = serde_json::to_vec(&bundle_for_hash)
            .expect("Serialization of bundle for hashing should not fail");
            
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        
        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }
    
    /// Verify this bundle against the provided events
    pub fn verify(
        &self,
        referenced_dag_events: &[DagEvent],
        public_keys: &HashMap<Did, VerifyingKey>,
    ) -> Result<(), TrustError> {
        // 1. Check that all referenced events exist in the provided list
        let event_ids: Vec<EventId> = referenced_dag_events.iter()
            .map(|e| {
                use icn_types::dag::merkle::calculate_event_hash;
                calculate_event_hash(e)
            })
            .collect();
            
        for referenced_id in &self.referenced_events {
            if !event_ids.contains(referenced_id) {
                return Err(TrustError::InvalidEvent(format!(
                    "Referenced event {} not found in provided events",
                    referenced_id
                )));
            }
        }
        
        // 2. Calculate the bundle hash for verification
        let bundle_hash = self.calculate_hash();
        
        // 3. Verify the proof against the hash
        self.proof.verify(&self.quorum_config, &bundle_hash, public_keys)?;
        
        Ok(())
    }
    
    /// Sign this bundle with the provided key
    pub fn sign(&mut self, did: Did, signing_function: impl FnOnce(&[u8]) -> Vec<u8>) {
        let hash = self.calculate_hash();
        let signature = signing_function(&hash);
        self.proof.add_signature(did, signature);
    }
    
    /// Set a CID for this bundle
    pub fn with_cid(mut self, cid: impl Into<String>) -> Self {
        self.bundle_cid = Some(cid.into());
        self
    }
}

/// A version of TrustBundle for hashing (excluding the proof)
#[derive(Serialize)]
struct TrustBundleForHash<'a> {
    federation_id: &'a str,
    referenced_events: &'a [EventId],
    quorum_config: &'a QuorumConfig,
    timestamp: u64,
    metadata: &'a HashMap<String, String>,
} 