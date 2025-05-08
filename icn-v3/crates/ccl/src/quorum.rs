use crate::error::CclError;
use icn_common::dag::{DAGNode, DAGNodeID};
use icn_common::identity::{ScopedIdentity, Credential};
use icn_common::verification::{Signature, Verifiable};

use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Types of quorum policies
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuorumType {
    /// Simple majority (more than 50%)
    SimpleMajority,
    
    /// Qualified majority (typically 2/3 or 3/4)
    QualifiedMajority(u8), // percentage required
    
    /// Unanimous consent from all members
    Unanimous,
    
    /// Threshold of signatures required
    Threshold(u32),
    
    /// Weighted voting based on member attributes
    Weighted,
}

/// A quorum policy that defines how decisions are made
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumPolicy {
    /// Unique ID for this policy
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// The scope this policy applies to
    pub scope: String,
    
    /// The type of quorum required
    pub quorum_type: QuorumType,
    
    /// Optional scope-specific parameters
    pub parameters: Option<serde_json::Value>,
    
    /// Optional description
    pub description: Option<String>,
}

impl QuorumPolicy {
    /// Create a new quorum policy
    pub fn new(
        name: String,
        scope: String,
        quorum_type: QuorumType,
        parameters: Option<serde_json::Value>,
        description: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            scope,
            quorum_type,
            parameters,
            description,
        }
    }
    
    /// Calculate the required number of votes for a quorum
    pub fn calculate_required_votes(&self, total_members: u32) -> u32 {
        match self.quorum_type {
            QuorumType::SimpleMajority => {
                (total_members / 2) + 1
            }
            QuorumType::QualifiedMajority(percentage) => {
                let required = (total_members as f64 * (percentage as f64 / 100.0)).ceil() as u32;
                required
            }
            QuorumType::Unanimous => {
                total_members
            }
            QuorumType::Threshold(threshold) => {
                std::cmp::min(threshold, total_members)
            }
            QuorumType::Weighted => {
                // For weighted voting, we'll need weights from the parameters
                // Default to simple majority if not specified
                (total_members / 2) + 1
            }
        }
    }
    
    /// Check if a quorum has been reached
    pub fn is_quorum_reached(
        &self,
        votes: &HashMap<String, bool>, // identity ID -> vote (true = yes)
        weights: Option<&HashMap<String, u32>>, // identity ID -> weight
        total_members: u32,
    ) -> bool {
        // Handle weighted voting if applicable
        if let QuorumType::Weighted = self.quorum_type {
            if let Some(weights) = weights {
                let mut total_weight = 0;
                let mut yes_weight = 0;
                
                for (id, &voted_yes) in votes {
                    if let Some(&weight) = weights.get(id) {
                        total_weight += weight;
                        if voted_yes {
                            yes_weight += weight;
                        }
                    }
                }
                
                // Extract threshold from parameters, default to 50%
                let threshold = self.parameters.as_ref()
                    .and_then(|p| p.get("threshold"))
                    .and_then(|t| t.as_u64())
                    .unwrap_or(50) as u32;
                
                // Check if yes votes exceed the threshold
                return yes_weight * 100 > total_weight * threshold;
            }
            
            // Fall back to simple majority if weights not provided
            return votes.values().filter(|&&v| v).count() > (total_members as usize) / 2;
        }
        
        // Handle non-weighted voting
        let yes_votes = votes.values().filter(|&&v| v).count() as u32;
        let required_votes = self.calculate_required_votes(total_members);
        
        yes_votes >= required_votes
    }
}

/// A cryptographic proof that a quorum has been reached
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumProof {
    /// The policy that was applied
    pub policy: QuorumPolicy,
    
    /// The signatures from members
    pub signatures: HashMap<String, Signature>, // identity ID -> signature
    
    /// The data that was signed
    pub signed_data: Vec<u8>,
    
    /// Timestamp when the quorum was reached
    pub timestamp: u64,
    
    /// The DAG node that anchors this proof
    pub anchor: Option<DAGNodeID>,
}

impl QuorumProof {
    /// Create a new quorum proof
    pub fn new(
        policy: QuorumPolicy,
        signed_data: Vec<u8>,
        anchor: Option<DAGNodeID>,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            policy,
            signatures: HashMap::new(),
            signed_data,
            timestamp,
            anchor,
        }
    }
    
    /// Add a signature to the proof
    pub fn add_signature(&mut self, identity_id: String, signature: Signature) {
        self.signatures.insert(identity_id, signature);
    }
    
    /// Check if a quorum has been reached based on the policy
    pub fn is_quorum_reached(
        &self,
        weights: Option<&HashMap<String, u32>>,
        total_members: u32,
    ) -> bool {
        // Create a map of votes (all signatures are considered "yes" votes)
        let votes: HashMap<String, bool> = self.signatures.keys()
            .map(|id| (id.clone(), true))
            .collect();
            
        self.policy.is_quorum_reached(&votes, weights, total_members)
    }
    
    /// Verify all signatures in the proof
    pub fn verify_signatures(
        &self,
        public_keys: &HashMap<String, Vec<u8>>, // identity ID -> public key bytes
    ) -> Result<bool, CclError> {
        for (identity_id, signature) in &self.signatures {
            if let Some(public_key_bytes) = public_keys.get(identity_id) {
                // Verify this signature
                let public_key = PublicKey::from_bytes(public_key_bytes)
                    .map_err(|_| CclError::Quorum("Invalid public key".into()))?;
                    
                let signature_bytes = ed25519_dalek::Signature::from_bytes(&signature.0)
                    .map_err(|_| CclError::Quorum("Invalid signature format".into()))?;
                    
                if let Err(e) = public_key.verify_strict(&self.signed_data, &signature_bytes) {
                    return Err(CclError::Quorum(
                        format!("Signature verification failed for {}: {:?}", identity_id, e)
                    ));
                }
            } else {
                return Err(CclError::Quorum(
                    format!("Public key not found for identity {}", identity_id)
                ));
            }
        }
        
        Ok(true)
    }
}

/// MembershipJoinRequest represents a request to join a federation or cooperative
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipJoinRequest {
    /// Unique ID for this request
    pub id: String,
    
    /// The identity requesting to join
    pub requester: ScopedIdentity,
    
    /// The scope being requested to join
    pub target_scope: String,
    
    /// Optional credentials proving eligibility
    pub credentials: Vec<Credential>,
    
    /// Timestamp of the request
    pub timestamp: u64,
    
    /// Optional metadata about the request
    pub metadata: Option<serde_json::Value>,
    
    /// Signature of the requester
    pub signature: Signature,
}

impl MembershipJoinRequest {
    /// Create a new membership join request
    pub fn new(
        requester: ScopedIdentity,
        target_scope: String,
        credentials: Vec<Credential>,
        metadata: Option<serde_json::Value>,
        private_key: &SecretKey,
    ) -> Result<Self, CclError> {
        let id = Uuid::new_v4().to_string();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a temporary request without signature for signing
        let temp_request = Self {
            id: id.clone(),
            requester: requester.clone(),
            target_scope: target_scope.clone(),
            credentials: credentials.clone(),
            timestamp,
            metadata: metadata.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_request)
            .map_err(|e| CclError::Common(e.into()))?;
        
        // Sign the request
        let public_key = PublicKey::from_bytes(requester.public_key())
            .map_err(|_| CclError::Quorum("Invalid public key".into()))?;
            
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            id,
            requester,
            target_scope,
            credentials,
            timestamp,
            metadata,
            signature,
        })
    }
}

impl Verifiable for MembershipJoinRequest {
    fn verify(&self) -> Result<bool, icn_common::error::CommonError> {
        // Create a temporary request without signature for verification
        let temp_request = Self {
            id: self.id.clone(),
            requester: self.requester.clone(),
            target_scope: self.target_scope.clone(),
            credentials: self.credentials.clone(),
            timestamp: self.timestamp,
            metadata: self.metadata.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_request)?;
        
        // Verify the signature
        let public_key = PublicKey::from_bytes(self.requester.public_key())
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Err(icn_common::error::CommonError::SignatureVerification),
        }
    }
}

/// MembershipVote represents a vote on a membership join request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipVote {
    /// The request being voted on
    pub request_id: String,
    
    /// The identity casting the vote
    pub voter: ScopedIdentity,
    
    /// Whether the vote is in favor
    pub approve: bool,
    
    /// Optional justification for the vote
    pub justification: Option<String>,
    
    /// Timestamp of the vote
    pub timestamp: u64,
    
    /// Signature of the voter
    pub signature: Signature,
}

impl MembershipVote {
    /// Create a new membership vote
    pub fn new(
        request_id: String,
        voter: ScopedIdentity,
        approve: bool,
        justification: Option<String>,
        private_key: &SecretKey,
    ) -> Result<Self, CclError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a temporary vote without signature for signing
        let temp_vote = Self {
            request_id: request_id.clone(),
            voter: voter.clone(),
            approve,
            justification: justification.clone(),
            timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_vote)
            .map_err(|e| CclError::Common(e.into()))?;
        
        // Sign the vote
        let public_key = PublicKey::from_bytes(voter.public_key())
            .map_err(|_| CclError::Quorum("Invalid public key".into()))?;
            
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            request_id,
            voter,
            approve,
            justification,
            timestamp,
            signature,
        })
    }
}

impl Verifiable for MembershipVote {
    fn verify(&self) -> Result<bool, icn_common::error::CommonError> {
        // Create a temporary vote without signature for verification
        let temp_vote = Self {
            request_id: self.request_id.clone(),
            voter: self.voter.clone(),
            approve: self.approve,
            justification: self.justification.clone(),
            timestamp: self.timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_vote)?;
        
        // Verify the signature
        let public_key = PublicKey::from_bytes(self.voter.public_key())
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Err(icn_common::error::CommonError::SignatureVerification),
        }
    }
}

/// MembershipAcceptance represents the acceptance of a membership join request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipAcceptance {
    /// The request being accepted
    pub request_id: String,
    
    /// The identity accepting the request (on behalf of the federation/cooperative)
    pub acceptor: ScopedIdentity,
    
    /// The membership credentials being issued
    pub credentials: Vec<Credential>,
    
    /// The quorum proof showing this was properly approved
    pub quorum_proof: QuorumProof,
    
    /// Timestamp of the acceptance
    pub timestamp: u64,
    
    /// Signature of the acceptor
    pub signature: Signature,
}

impl MembershipAcceptance {
    /// Create a new membership acceptance
    pub fn new(
        request_id: String,
        acceptor: ScopedIdentity,
        credentials: Vec<Credential>,
        quorum_proof: QuorumProof,
        private_key: &SecretKey,
    ) -> Result<Self, CclError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a temporary acceptance without signature for signing
        let temp_acceptance = Self {
            request_id: request_id.clone(),
            acceptor: acceptor.clone(),
            credentials: credentials.clone(),
            quorum_proof: quorum_proof.clone(),
            timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_acceptance)
            .map_err(|e| CclError::Common(e.into()))?;
        
        // Sign the acceptance
        let public_key = PublicKey::from_bytes(acceptor.public_key())
            .map_err(|_| CclError::Quorum("Invalid public key".into()))?;
            
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            request_id,
            acceptor,
            credentials,
            quorum_proof,
            timestamp,
            signature,
        })
    }
}

impl Verifiable for MembershipAcceptance {
    fn verify(&self) -> Result<bool, icn_common::error::CommonError> {
        // Create a temporary acceptance without signature for verification
        let temp_acceptance = Self {
            request_id: self.request_id.clone(),
            acceptor: self.acceptor.clone(),
            credentials: self.credentials.clone(),
            quorum_proof: self.quorum_proof.clone(),
            timestamp: self.timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_acceptance)?;
        
        // Verify the signature
        let public_key = PublicKey::from_bytes(self.acceptor.public_key())
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| icn_common::error::CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => {
                // Also verify all credentials
                for credential in &self.credentials {
                    if !credential.verify()? {
                        return Err(icn_common::error::CommonError::InvalidCredential(
                            "Invalid credential in membership acceptance".into()
                        ));
                    }
                }
                
                Ok(true)
            }
            Err(_) => Err(icn_common::error::CommonError::SignatureVerification),
        }
    }
} 