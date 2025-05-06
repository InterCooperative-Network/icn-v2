use crate::did::DidKey;
use icn_types::dag::EventId;
use icn_types::Cid;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, Verifier, Signature};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use thiserror::Error;
use multihash::{MultihashDigest, Code as MultihashCode};
use serde_ipld_dagcbor;

const DAG_CBOR_CODEC: u64 = 0x71;

/// Errors related to ExecutionReceipt operations
#[derive(Error, Debug)]
pub enum ExecutionReceiptError {
    #[error("JSON Serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
    
    #[error("CBOR Serialization error: {0}")]
    CborSerialization(#[from] serde_ipld_dagcbor::Error),

    #[error("Signature error: {0}")]
    Signature(String),
    
    #[error("Proof validation failed: {0}")]
    ProofValidation(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Decoding error: {0}")]
    Decoding(#[from] base64::DecodeError),

    #[error("Multihash error: {0}")]
    Multihash(#[from] multihash::Error),
}

/// The W3C‐style Proof object for cryptographic verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proof {
    /// The type of proof (typically Ed25519Signature2020)
    #[serde(rename = "type")]
    pub type_: String,
    
    /// When the proof was created
    pub created: DateTime<Utc>,
    
    /// The purpose of this proof (typically assertionMethod)
    #[serde(rename = "proofPurpose")]
    pub proof_purpose: String,
    
    /// DID URL that identifies the verification method
    #[serde(rename = "verificationMethod")]
    pub verification_method: String,
    
    /// The cryptographic signature value
    #[serde(rename = "proofValue")]
    pub proof_value: String,
}

/// The execution context/scope of this credential
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ExecutionScope {
    /// Federation governance execution
    Federation {
        /// Federation DID
        federation_id: String,
    },
    
    /// Mesh compute task execution
    MeshCompute {
        /// Task identifier
        task_id: String,
        
        /// Job identifier within the task
        job_id: String,
    },
    
    /// Cooperative multi-party computation
    Cooperative {
        /// Cooperative DID
        coop_id: String,
        
        /// Module identifier
        module: String,
    },
    
    /// Custom execution context
    Custom {
        /// Description of the custom context
        description: String,
        
        /// Additional metadata
        metadata: serde_json::Value,
    },
}

/// The credentialSubject payload for an execution receipt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionSubject {
    /// DID of the node that executed the computation
    pub id: String,
    
    /// Execution scope/context
    pub scope: ExecutionScope,
    
    /// Optional DID of the individual or organization that submitted the task
    pub submitter: Option<String>,
    
    /// Content ID of the input module
    pub module_cid: String,
    
    /// Content ID of the output result
    pub result_cid: String,
    
    /// Associated DAG event ID for traceability
    pub event_id: Option<EventId>,
    
    /// Unix timestamp of execution completion
    pub timestamp: u64,
    
    /// Execution status (success, failure, etc.)
    pub status: ExecutionStatus,
    
    /// Optional additional properties
    #[serde(flatten)]
    pub additional_properties: Option<serde_json::Value>,
}

/// Status of the execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Execution completed successfully
    Success,
    
    /// Execution failed
    Failed,
    
    /// Execution is pending
    Pending,
    
    /// Execution was canceled
    Canceled,
}

/// The ExecutionReceipt Verifiable Credential structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionReceipt {
    /// JSON‑LD contexts
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (a unique URI or CID)
    pub id: String,
    
    /// Type of credential
    #[serde(rename = "type")]
    pub types: Vec<String>,
    
    /// Issuer DID (federation or node DID)
    pub issuer: String,
    
    /// ISO8601 issuance date
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    
    /// The subject payload containing execution details
    #[serde(rename = "credentialSubject")]
    pub credential_subject: ExecutionSubject,
    
    /// Cryptographic proof section
    pub proof: Option<Proof>,
}

impl ExecutionReceipt {
    /// Create a new ExecutionReceipt without a proof
    pub fn new(
        id: impl Into<String>,
        issuer: impl Into<String>,
        subject: ExecutionSubject,
    ) -> Self {
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://schema.intercooperative.network/2023/credentials/execution-receipt/v1".to_string(),
            ],
            id: id.into(),
            types: vec![
                "VerifiableCredential".to_string(),
                "ExecutionReceipt".to_string(),
            ],
            issuer: issuer.into(),
            issuance_date: Utc::now(),
            credential_subject: subject,
            proof: None,
        }
    }
    
    /// Sign the ExecutionReceipt with the provided DID key
    pub fn sign(mut self, did_key: &DidKey) -> Result<Self, ExecutionReceiptError> {
        let mut temp = self.clone();
        temp.proof = None;
        let to_sign = serde_json::to_vec(&temp)
            .map_err(ExecutionReceiptError::JsonSerialization)?;
        let signature_bytes = did_key.sign(&to_sign).to_bytes();
        let proof = Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            proof_purpose: "assertionMethod".to_string(),
            verification_method: did_key.to_did_string(),
            proof_value: BASE64_ENGINE.encode(signature_bytes),
        };
        self.proof = Some(proof);
        Ok(self)
    }
    
    /// Verify the ExecutionReceipt's proof
    pub fn verify(&self) -> Result<bool, ExecutionReceiptError> {
        let proof = self.proof.as_ref().ok_or(
            ExecutionReceiptError::MissingField("proof".to_string())
        )?;
        let mut temp = self.clone();
        temp.proof = None;
        let data = serde_json::to_vec(&temp)
            .map_err(ExecutionReceiptError::JsonSerialization)?;
        let verifying_key = DidKey::verifying_key_from_did(&proof.verification_method)
            .map_err(|e| ExecutionReceiptError::Signature(e.to_string()))?;
        let sig_bytes = BASE64_ENGINE.decode(&proof.proof_value)?;
        if sig_bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
            return Err(ExecutionReceiptError::Signature(format!(
                "Invalid signature length: expected {}, got {}",
                ed25519_dalek::SIGNATURE_LENGTH,
                sig_bytes.len()
            )));
        }
        let mut sig_array = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);
        verifying_key.verify(&data, &signature)
            .map_err(|e| ExecutionReceiptError::ProofValidation(e.to_string()))?;
        Ok(true)
    }
    
    /// Export the ExecutionReceipt to JSON string
    pub fn to_json(&self) -> Result<String, ExecutionReceiptError> {
        serde_json::to_string_pretty(self)
            .map_err(ExecutionReceiptError::JsonSerialization)
    }
    
    /// Import an ExecutionReceipt from JSON string
    pub fn from_json(json: &str) -> Result<Self, ExecutionReceiptError> {
        serde_json::from_str(json)
            .map_err(ExecutionReceiptError::JsonSerialization)
    }

    /// Calculate a CID for the ExecutionReceipt (content-addressed, typically excluding proof).
    /// The CID is generated from the DAG-CBOR representation of the receipt.
    pub fn to_cid(&self) -> Result<Cid, ExecutionReceiptError> {
        let mut temp_receipt = self.clone();
        // For content addressing, proof is usually excluded or handled differently.
        // If the `id` field itself is intended to be a CID of the content after issuance,
        // this method might be used to generate that ID, or verify it.
        // Here, we generate a CID of the receipt *content* (proof excluded).
        temp_receipt.proof = None; 

        let bytes = serde_ipld_dagcbor::to_vec(&temp_receipt)?;
        let hash = MultihashCode::Sha2_256.digest(&bytes);
        Ok(Cid::new_v1(DAG_CBOR_CODEC, hash))
    }
} 