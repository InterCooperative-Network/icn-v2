use crate::error::CclError;
use crate::quorum::{QuorumPolicy, QuorumProof};
use icn_common::identity::ScopedIdentity;
use icn_common::verification::Signature;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of governance proposals
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    /// Membership changes
    Membership,
    
    /// Resource allocation
    ResourceAllocation,
    
    /// Policy changes
    PolicyChange,
    
    /// Structural changes
    StructuralChange,
    
    /// Custom proposal
    Custom(String),
}

/// A governance proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique identifier
    pub id: String,
    
    /// Type of proposal
    pub proposal_type: ProposalType,
    
    /// The scope this proposal applies to
    pub scope: String,
    
    /// The identity proposing the action
    pub proposer: ScopedIdentity,
    
    /// Title of the proposal
    pub title: String,
    
    /// Description of the proposal
    pub description: String,
    
    /// Detailed proposal data
    pub data: serde_json::Value,
    
    /// When voting opens
    pub voting_starts_at: u64,
    
    /// When voting closes
    pub voting_ends_at: u64,
    
    /// The quorum policy to use for this proposal
    pub quorum_policy: QuorumPolicy,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Signature of the proposer
    pub signature: Signature,
}

impl Proposal {
    /// Create a new proposal
    pub fn new(
        proposal_type: ProposalType,
        scope: String,
        proposer: ScopedIdentity,
        title: String,
        description: String,
        data: serde_json::Value,
        voting_period_days: u64,
        quorum_policy: QuorumPolicy,
        private_key: &ed25519_dalek::SecretKey,
    ) -> Result<Self, CclError> {
        let id = Uuid::new_v4().to_string();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        let voting_starts_at = now;
        let voting_ends_at = now + (voting_period_days * 24 * 60 * 60 * 1000); // Convert days to ms
        
        // Placeholder implementation - would normally sign the proposal
        let signature = Signature(vec![]);
        
        Ok(Self {
            id,
            proposal_type,
            scope,
            proposer,
            title,
            description,
            data,
            voting_starts_at,
            voting_ends_at,
            quorum_policy,
            created_at: now,
            signature,
        })
    }
}

/// A vote on a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    /// The proposal being voted on
    pub proposal_id: String,
    
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

/// The outcome of a vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteOutcome {
    /// The proposal that was voted on
    pub proposal_id: String,
    
    /// Whether the proposal passed
    pub passed: bool,
    
    /// The quorum proof for this outcome
    pub quorum_proof: QuorumProof,
    
    /// Number of yes votes
    pub yes_votes: u32,
    
    /// Number of no votes
    pub no_votes: u32,
    
    /// Timestamp when the outcome was determined
    pub timestamp: u64,
} 