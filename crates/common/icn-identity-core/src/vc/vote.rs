use crate::did::DidKey;
use crate::vc::Proof;
use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE;
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to Vote VCs
#[derive(Error, Debug)]
pub enum VoteError {
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
    
    #[error("Base64 decoding error: {0}")]
    Base64Decoding(#[from] base64::DecodeError),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Signature error: {0}")]
    Signature(String),
    
    #[error("Proof validation failed: {0}")]
    ProofValidation(String),
    
    #[error("Invalid vote: {0}")]
    InvalidVote(String),
}

/// Vote decision
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VoteDecision {
    /// Vote in favor
    Yes,
    
    /// Vote against
    No,
    
    /// Abstain from voting
    Abstain,
    
    /// Vote to veto (if supported by governance rules)
    Veto,
}

/// The credentialSubject payload for a vote VC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteSubject {
    /// Voter DID (person who cast the vote)
    pub id: String,
    
    /// Federation DID this vote belongs to
    pub federation_id: String,
    
    /// Reference to the proposal ID being voted on
    pub proposal_id: String,
    
    /// The vote decision
    pub decision: VoteDecision,
    
    /// Optional voting power if weighted voting is used
    pub voting_power: Option<u64>,
    
    /// Optional justification or comment
    pub justification: Option<String>,
    
    /// Optional delegate DID if voting on behalf of someone else
    pub delegate_for: Option<String>,
    
    /// Is this vote replacing a previous vote by the same voter?
    pub is_amendment: bool,
    
    /// Reference to previous vote if this is an amendment
    pub previous_vote_id: Option<String>,
    
    /// Vote cast timestamp (Unix timestamp)
    pub cast_at: u64,
}

/// The Vote Verifiable Credential structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteCredential {
    /// JSON‑LD contexts
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (a unique URI or CID)
    pub id: String,
    
    /// Type of credential
    #[serde(rename = "type")]
    pub types: Vec<String>,
    
    /// Issuer DID (federation DID)
    pub issuer: String,
    
    /// ISO8601 issuance date
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    
    /// The subject payload containing vote details
    #[serde(rename = "credentialSubject")]
    pub credential_subject: VoteSubject,
    
    /// Cryptographic proof section
    pub proof: Option<Proof>,
}

impl VoteCredential {
    /// Create a new VoteCredential without a proof
    pub fn new(
        id: impl Into<String>,
        issuer: impl Into<String>,
        subject: VoteSubject,
    ) -> Self {
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://schema.intercooperative.network/2023/credentials/vote/v1".to_string(),
            ],
            id: id.into(),
            types: vec![
                "VerifiableCredential".to_string(),
                "VoteCredential".to_string(),
            ],
            issuer: issuer.into(),
            issuance_date: Utc::now(),
            credential_subject: subject,
            proof: None,
        }
    }
    
    /// Sign the VoteCredential with the provided DID key
    pub fn sign(mut self, did_key: &DidKey) -> Result<Self, VoteError> {
        // Validate that voter and signer match
        if did_key.did().to_string() != self.credential_subject.id {
            return Err(VoteError::InvalidVote(
                "Vote must be signed by the voter's key".to_string()
            ));
        }
        
        let mut temp = self.clone();
        temp.proof = None;
        let to_sign = serde_json::to_vec(&temp)
            .map_err(VoteError::JsonSerialization)?;
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
    
    /// Verify the VoteCredential's proof
    pub fn verify(&self) -> Result<bool, VoteError> {
        let proof = self.proof.as_ref().ok_or(
            VoteError::MissingField("proof".to_string())
        )?;
        
        // Verify that the voter DID matches the signature verification method
        let verification_did = proof.verification_method.split('#').next()
            .ok_or_else(|| VoteError::Signature("Invalid verification method format".to_string()))?;
            
        if verification_did != self.credential_subject.id {
            return Err(VoteError::InvalidVote(
                format!("Vote credential subject ID ({}) doesn't match signature verification method ({})",
                    self.credential_subject.id, verification_did)
            ));
        }
        
        let mut temp = self.clone();
        temp.proof = None;
        let data = serde_json::to_vec(&temp)
            .map_err(VoteError::JsonSerialization)?;
        let verifying_key = DidKey::verifying_key_from_did(&proof.verification_method)
            .map_err(|e| VoteError::Signature(e.to_string()))?;
        let sig_bytes = BASE64_ENGINE.decode(&proof.proof_value)?;
        if sig_bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
            return Err(VoteError::Signature(format!(
                "Invalid signature length: expected {}, got {}",
                ed25519_dalek::SIGNATURE_LENGTH,
                sig_bytes.len()
            )));
        }
        let mut sig_array = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);
        verifying_key.verify(&data, &signature)
            .map_err(|e| VoteError::ProofValidation(e.to_string()))?;
        
        Ok(true)
    }
    
    /// Export the VoteCredential to JSON string
    pub fn to_json(&self) -> Result<String, VoteError> {
        serde_json::to_string_pretty(self)
            .map_err(VoteError::JsonSerialization)
    }
    
    /// Import a VoteCredential from JSON string
    pub fn from_json(json: &str) -> Result<Self, VoteError> {
        serde_json::from_str(json)
            .map_err(VoteError::JsonSerialization)
    }
} 