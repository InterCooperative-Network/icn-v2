use crate::dag::NodeScope;
use crate::Cid;
use crate::Did;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// Represents a signature from a specific scope (cooperative, community, or federation)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScopeSignature {
    /// DID of the signer
    pub signer: Did,
    /// Scope of the signer
    pub scope: NodeScope,
    /// Scope ID (if applicable)
    pub scope_id: Option<String>,
    /// Signature over the canonical representation of the attested data
    pub signature: Vec<u8>,
    /// Timestamp when the signature was created
    pub timestamp: DateTime<Utc>,
}

/// Attests to the linkage between a node in a cooperative/community DAG and a node in the federation DAG
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LineageAttestation {
    /// Parent scope (usually Federation)
    pub parent_scope: NodeScope,
    /// Parent scope ID (federation_id)
    pub parent_scope_id: String,
    /// CID of the parent node in the parent scope's DAG
    pub parent_cid: Cid,
    
    /// Child scope (Cooperative or Community)
    pub child_scope: NodeScope,
    /// Child scope ID (coop_id or community_id)
    pub child_scope_id: String,
    /// CID of the child node in the child scope's DAG
    pub child_cid: Cid,
    
    /// Description of the relationship
    pub description: Option<String>,
    
    /// Timestamp when the attestation was created
    pub timestamp: DateTime<Utc>,
    
    /// Signatures from both scopes, validating the attestation
    pub signatures: Vec<ScopeSignature>,
    
    /// If this attestation is for a join flow, reference to the membership attestation
    pub membership_attestation_cid: Option<Cid>,
}

impl LineageAttestation {
    /// Creates a new LineageAttestation linking a cooperative/community DAG node to a federation DAG node
    pub fn new(
        parent_scope: NodeScope,
        parent_scope_id: &str,
        parent_cid: Cid,
        child_scope: NodeScope,
        child_scope_id: &str,
        child_cid: Cid,
        description: Option<String>,
    ) -> Self {
        Self {
            parent_scope,
            parent_scope_id: parent_scope_id.to_string(),
            parent_cid,
            child_scope,
            child_scope_id: child_scope_id.to_string(),
            child_cid,
            description,
            timestamp: Utc::now(),
            signatures: Vec::new(),
            membership_attestation_cid: None,
        }
    }
    
    /// Creates a new LineageAttestation for a federation join
    pub fn new_join_attestation(
        federation_id: &str,
        federation_cid: Cid,
        scope_type: NodeScope,
        scope_id: &str,
        scope_cid: Cid,
        membership_attestation_cid: Cid,
    ) -> Self {
        let mut attestation = Self::new(
            NodeScope::Federation,
            federation_id,
            federation_cid, 
            scope_type,
            scope_id,
            scope_cid,
            Some(format!("Join attestation for {} to federation {}", scope_id, federation_id))
        );
        
        attestation.membership_attestation_cid = Some(membership_attestation_cid);
        attestation
    }
    
    /// Adds a signature to the attestation
    pub fn add_signature(&mut self, signature: ScopeSignature) {
        self.signatures.push(signature);
    }
    
    /// Returns true if the attestation has signatures from both scopes
    pub fn is_complete(&self) -> bool {
        let mut has_parent_sig = false;
        let mut has_child_sig = false;
        
        for sig in &self.signatures {
            if sig.scope == self.parent_scope {
                has_parent_sig = true;
            }
            if sig.scope == self.child_scope {
                has_child_sig = true;
            }
        }
        
        has_parent_sig && has_child_sig
    }
}

/// Represents the proof of reaching a quorum threshold for decision making
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QuorumProof {
    /// Total number of eligible voters
    pub total_members: u32,
    
    /// Required threshold (number of votes needed)
    pub threshold: u32,
    
    /// Actual number of votes received
    pub votes_received: u32,
    
    /// Number of yes votes
    pub yes_votes: u32,
    
    /// Number of no votes
    pub no_votes: u32,
    
    /// DIDs of all eligible voters
    pub eligible_voters: Vec<Did>,
    
    /// DIDs that voted yes
    pub yes_voters: Vec<Did>,
    
    /// DIDs that voted no
    pub no_voters: Vec<Did>,
    
    /// Timestamp when quorum was achieved
    pub timestamp: DateTime<Utc>,
}

impl QuorumProof {
    /// Creates a new QuorumProof
    pub fn new(
        total_members: u32,
        threshold: u32,
        eligible_voters: Vec<Did>,
    ) -> Self {
        Self {
            total_members,
            threshold,
            votes_received: 0,
            yes_votes: 0,
            no_votes: 0,
            eligible_voters,
            yes_voters: Vec::new(),
            no_voters: Vec::new(),
            timestamp: Utc::now(),
        }
    }
    
    /// Adds a vote to the quorum proof
    pub fn add_vote(&mut self, voter: Did, vote: bool) -> Result<bool, AttestationError> {
        // Verify the voter is eligible
        if !self.eligible_voters.iter().any(|did| did == &voter) {
            return Err(AttestationError::IneligibleVoter(voter));
        }
        
        // Check if this voter has already voted
        if self.yes_voters.contains(&voter) || self.no_voters.contains(&voter) {
            return Err(AttestationError::DuplicateVote(voter));
        }
        
        // Add the vote
        self.votes_received += 1;
        
        if vote {
            self.yes_votes += 1;
            self.yes_voters.push(voter);
        } else {
            self.no_votes += 1;
            self.no_voters.push(voter);
        }
        
        // Update timestamp
        self.timestamp = Utc::now();
        
        // Check if quorum is reached
        Ok(self.is_quorum_reached())
    }
    
    /// Returns true if quorum threshold is reached
    pub fn is_quorum_reached(&self) -> bool {
        self.votes_received >= self.threshold
    }
    
    /// Returns true if the proposal is approved (quorum reached and majority yes)
    pub fn is_approved(&self) -> bool {
        self.is_quorum_reached() && self.yes_votes > self.no_votes
    }
    
    /// Verify the quorum proof is internally consistent
    pub fn verify(&self) -> Result<bool, AttestationError> {
        // Check total votes match individual vote counts
        if self.votes_received != self.yes_votes + self.no_votes {
            return Err(AttestationError::InconsistentVoteCounts);
        }
        
        // Check voter list lengths match vote counts
        if (self.yes_voters.len() as u32) != self.yes_votes ||
           (self.no_voters.len() as u32) != self.no_votes {
            return Err(AttestationError::InconsistentVoterLists);
        }
        
        // Check there's no overlap between yes_voters and no_voters
        for voter in &self.yes_voters {
            if self.no_voters.contains(voter) {
                return Err(AttestationError::DuplicateVoter(voter.clone()));
            }
        }
        
        // Check threshold is sensible
        if self.threshold > self.total_members {
            return Err(AttestationError::InvalidThreshold);
        }
        
        Ok(true)
    }
}

/// Federation membership attestation documenting the acceptance of a scope into a federation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FederationMembershipAttestation {
    /// Scope type (Cooperative or Community)
    pub scope_type: NodeScope,
    
    /// Scope ID (cooperative ID or community ID)
    pub scope_id: String,
    
    /// Scope's genesis CID
    pub scope_genesis_cid: Cid,
    
    /// Federation ID
    pub federation_id: String,
    
    /// Federation's genesis CID
    pub federation_genesis_cid: Cid,
    
    /// CID of the join proposal
    pub join_proposal_cid: Cid,
    
    /// CIDs of all votes on this join proposal
    pub vote_cids: Vec<Cid>,
    
    /// Quorum proof validating that sufficient votes were received
    pub quorum_proof: QuorumProof,
    
    /// Description or context for the membership
    pub description: Option<String>,
    
    /// Timestamp when attestation was created
    pub timestamp: DateTime<Utc>,
    
    /// Signatures validating this attestation (from federation and joining scope)
    pub signatures: Vec<ScopeSignature>,
}

impl FederationMembershipAttestation {
    /// Creates a new FederationMembershipAttestation
    pub fn new(
        scope_type: NodeScope,
        scope_id: &str,
        scope_genesis_cid: Cid,
        federation_id: &str,
        federation_genesis_cid: Cid,
        join_proposal_cid: Cid,
        vote_cids: Vec<Cid>,
        quorum_proof: QuorumProof,
        description: Option<String>,
    ) -> Self {
        Self {
            scope_type,
            scope_id: scope_id.to_string(),
            scope_genesis_cid,
            federation_id: federation_id.to_string(),
            federation_genesis_cid,
            join_proposal_cid,
            vote_cids,
            quorum_proof,
            description,
            timestamp: Utc::now(),
            signatures: Vec::new(),
        }
    }
    
    /// Adds a signature to the attestation
    pub fn add_signature(&mut self, signature: ScopeSignature) {
        self.signatures.push(signature);
    }
    
    /// Returns true if the attestation has signatures from both the federation and the joining scope
    pub fn is_complete(&self) -> bool {
        let mut has_federation_sig = false;
        let mut has_scope_sig = false;
        
        for sig in &self.signatures {
            if sig.scope == NodeScope::Federation {
                has_federation_sig = true;
            }
            if sig.scope == self.scope_type {
                has_scope_sig = true;
            }
        }
        
        has_federation_sig && has_scope_sig
    }
    
    /// Verify the attestation is valid
    pub fn verify(&self) -> Result<bool, AttestationError> {
        // Verify quorum proof
        self.quorum_proof.verify()?;
        
        // Check that the proposal was approved
        if !self.quorum_proof.is_approved() {
            return Err(AttestationError::JoinRequestRejected);
        }
        
        // Check signatures
        if !self.is_complete() {
            return Err(AttestationError::MissingSignature(NodeScope::Federation));
        }
        
        // Perform other verification as needed
        // In a full implementation, we would verify signatures against DIDs, etc.
        
        Ok(true)
    }
}

/// Error types for attestation operations
#[derive(thiserror::Error, Debug)]
pub enum AttestationError {
    #[error("Missing required signature from scope: {0:?}")]
    MissingSignature(NodeScope),
    
    #[error("Invalid signature from {0}")]
    InvalidSignature(Did),
    
    #[error("Mismatched scope in attestation")]
    MismatchedScope,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Ineligible voter: {0}")]
    IneligibleVoter(Did),
    
    #[error("Duplicate vote from: {0}")]
    DuplicateVote(Did),
    
    #[error("Duplicate voter found: {0}")]
    DuplicateVoter(Did),
    
    #[error("Inconsistent vote counts")]
    InconsistentVoteCounts,
    
    #[error("Inconsistent voter lists")]
    InconsistentVoterLists,
    
    #[error("Invalid threshold (greater than total members)")]
    InvalidThreshold,
    
    #[error("Join request was rejected by federation")]
    JoinRequestRejected,
} 