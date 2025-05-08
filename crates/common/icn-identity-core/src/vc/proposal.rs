use crate::did::DidKey;
use crate::vc::Proof;
use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE;
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier};
use icn_types::dag::EventId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to Proposal VCs
#[derive(Error, Debug)]
pub enum ProposalError {
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
    
    #[error("Invalid proposal state transition: {0}")]
    InvalidStateTransition(String),
}

/// Allowed proposal types for the federation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProposalType {
    /// Proposal to modify federation parameters
    ConfigChange,
    
    /// Proposal to add a new member
    MemberAddition,
    
    /// Proposal to remove a member
    MemberRemoval,
    
    /// Proposal to upgrade code/smart contracts
    CodeUpgrade,
    
    /// Proposal to execute custom WASM code
    CodeExecution,
    
    /// Generic text proposal (no automatic execution)
    TextProposal,
    
    /// Custom proposal type
    Custom(String),
}

/// Status of a proposal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    /// Proposal is draft/being developed
    Draft,
    
    /// Proposal is active and accepting votes
    Active,
    
    /// Proposal was accepted (met quorum requirements)
    Passed,
    
    /// Proposal was rejected (quorum requirements not met)
    Rejected,
    
    /// Proposal was executed
    Executed,
    
    /// Proposal was canceled
    Canceled,
}

/// Definition of voting threshold for the proposal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VotingThreshold {
    /// Simple majority (>50%)
    Majority,
    
    /// Specific percentage threshold (1-100)
    Percentage(u8),
    
    /// All participants must vote yes
    Unanimous,
    
    /// Custom weighted threshold where weights are provided
    Weighted { weights: Vec<(String, u64)>, threshold: u64 },
}

/// Duration of voting period
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VotingDuration {
    /// Time-based duration in seconds
    TimeBased(u64),
    
    /// Block-based duration (used in chains that have block concepts)
    BlockBased(u64),
    
    /// Open-ended duration (until explicitly closed)
    OpenEnded,
}

/// The credentialSubject payload for a proposal VC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposalSubject {
    /// Federation DID this proposal belongs to
    pub id: String,
    
    /// Proposal title
    pub title: String,
    
    /// Proposal description
    pub description: String,
    
    /// Proposal type
    pub proposal_type: ProposalType,
    
    /// Proposal current status
    pub status: ProposalStatus,
    
    /// Submitter DID (person/org who submitted)
    pub submitter: String,
    
    /// Voting threshold required
    pub voting_threshold: VotingThreshold,
    
    /// Voting duration
    pub voting_duration: VotingDuration,
    
    /// Start timestamp for voting (Unix timestamp)
    pub voting_start_time: u64,
    
    /// End timestamp for voting (Unix timestamp, optional for open-ended)
    pub voting_end_time: Option<u64>,
    
    /// Reference CID for executable code if any
    pub execution_cid: Option<String>,
    
    /// Reference to AgoraNet thread CID that contains the proposal discussion
    pub thread_cid: Option<String>,
    
    /// Additional custom parameters specific to this proposal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    
    /// Reference to previous proposal version if this is an amendment
    pub previous_version: Option<String>,
    
    /// Associated DAG event IDs for traceability
    pub event_id: Option<EventId>,
    
    /// Creation timestamp (Unix timestamp)
    pub created_at: u64,
    
    /// Last updated timestamp (Unix timestamp)
    pub updated_at: u64,
}

/// The Proposal Verifiable Credential structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposalCredential {
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
    
    /// The subject payload containing proposal details
    #[serde(rename = "credentialSubject")]
    pub credential_subject: ProposalSubject,
    
    /// Cryptographic proof section
    pub proof: Option<Proof>,
}

impl ProposalCredential {
    /// Create a new ProposalCredential without a proof
    pub fn new(
        id: impl Into<String>,
        issuer: impl Into<String>,
        subject: ProposalSubject,
    ) -> Self {
        Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://schema.intercooperative.network/2023/credentials/proposal/v1".to_string(),
            ],
            id: id.into(),
            types: vec![
                "VerifiableCredential".to_string(),
                "ProposalCredential".to_string(),
            ],
            issuer: issuer.into(),
            issuance_date: Utc::now(),
            credential_subject: subject,
            proof: None,
        }
    }
    
    /// Sign the ProposalCredential with the provided DID key
    pub fn sign(mut self, did_key: &DidKey) -> Result<Self, ProposalError> {
        let mut temp = self.clone();
        temp.proof = None;
        let to_sign = serde_json::to_vec(&temp)
            .map_err(ProposalError::JsonSerialization)?;
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
    
    /// Verify the ProposalCredential's proof
    pub fn verify(&self) -> Result<bool, ProposalError> {
        let proof = self.proof.as_ref().ok_or(
            ProposalError::MissingField("proof".to_string())
        )?;
        let mut temp = self.clone();
        temp.proof = None;
        let data = serde_json::to_vec(&temp)
            .map_err(ProposalError::JsonSerialization)?;
        let verifying_key = DidKey::verifying_key_from_did(&proof.verification_method)
            .map_err(|e| ProposalError::Signature(e.to_string()))?;
        let sig_bytes = BASE64_ENGINE.decode(&proof.proof_value)?;
        if sig_bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
            return Err(ProposalError::Signature(format!(
                "Invalid signature length: expected {}, got {}",
                ed25519_dalek::SIGNATURE_LENGTH,
                sig_bytes.len()
            )));
        }
        let mut sig_array = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);
        verifying_key.verify(&data, &signature)
            .map_err(|e| ProposalError::ProofValidation(e.to_string()))?;
        Ok(true)
    }
    
    /// Export the ProposalCredential to JSON string
    pub fn to_json(&self) -> Result<String, ProposalError> {
        serde_json::to_string_pretty(self)
            .map_err(ProposalError::JsonSerialization)
    }
    
    /// Import a ProposalCredential from JSON string
    pub fn from_json(json: &str) -> Result<Self, ProposalError> {
        serde_json::from_str(json)
            .map_err(ProposalError::JsonSerialization)
    }

    /// Update the status of a proposal (with validation)
    pub fn update_status(&mut self, new_status: ProposalStatus) -> Result<(), ProposalError> {
        use ProposalStatus::*;
        
        // Check valid state transitions
        match (&self.credential_subject.status, &new_status) {
            // Valid transitions
            (Draft, Active) => {},
            (Active, Passed) | (Active, Rejected) => {},
            (Passed, Executed) => {},
            (Draft, Canceled) | (Active, Canceled) => {},
            // Invalid transitions
            (current, new) => {
                return Err(ProposalError::InvalidStateTransition(
                    format!("Cannot transition from {:?} to {:?}", current, new)
                ));
            }
        }
        
        // Update the status
        self.credential_subject.status = new_status;
        // Update the updated_at timestamp
        self.credential_subject.updated_at = chrono::Utc::now().timestamp() as u64;
        
        Ok(())
    }
} 