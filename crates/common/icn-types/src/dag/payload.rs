use serde::{Deserialize, Serialize};
use crate::Cid;
use crate::Did;
use crate::policy::ScopePolicyConfig;
use crate::receipts::QuorumProof;

/// Trait for DAG payload types that can provide their action type for policy enforcement
pub trait ActionType {
    /// Returns the action type string used for policy enforcement, or None if
    /// this payload doesn't require policy enforcement
    fn action_type(&self) -> Option<String>;
    
    /// Returns true if this payload requires authorization
    fn requires_authorization(&self) -> bool {
        self.action_type().is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum EventPayload {
    Genesis { federation_id: String },
    Proposal { proposal_id: String, content_cid: String },
    Vote { proposal_id: String, choice: String },
    Execution { receipt_cid: String },
    Receipt { receipt_cid: Cid },
    // New federation join flow payloads
    JoinRequest { 
        scope_type: String,  // "Cooperative" or "Community"
        scope_id: String, 
        scope_genesis_cid: String,
        federation_id: String,
        federation_genesis_cid: String 
    },
    JoinVote { 
        join_request_cid: String, 
        choice: String, 
        reason: Option<String> 
    },
    JoinApproval { 
        join_request_cid: String,
        attestation_cid: String,
        lineage_cid: String 
    },
    // Policy update flow payloads
    PolicyUpdateProposal {
        scope_type: String,        // "Federation", "Cooperative", or "Community"
        scope_id: String,          // ID of the scope being updated
        proposed_policy: String,   // JSON serialized ScopePolicyConfig
        proposer_did: String,      // DID of the proposer
        description: String,       // Description of the policy change
    },
    PolicyUpdateVote {
        proposal_cid: String,      // CID of the PolicyUpdateProposal
        choice: String,            // "approve" or "reject"
        reason: Option<String>,    // Optional reason for the vote
        voter_did: String,         // DID of the voter
    },
    PolicyUpdateApproval {
        proposal_cid: String,      // CID of the PolicyUpdateProposal
        quorum_proof: QuorumProof, // Proof of sufficient votes
        approver_did: String,      // DID of the approver (federation authority)
    },
    Custom { fields: serde_json::Value },
}

impl EventPayload {
    /// Create a Genesis payload
    pub fn genesis(federation_id: impl Into<String>) -> Self {
        EventPayload::Genesis {
            federation_id: federation_id.into(),
        }
    }
    
    /// Create a Proposal payload
    pub fn proposal(proposal_id: impl Into<String>, content_cid: impl Into<String>) -> Self {
        EventPayload::Proposal {
            proposal_id: proposal_id.into(),
            content_cid: content_cid.into(),
        }
    }
    
    /// Create a Vote payload
    pub fn vote(proposal_id: impl Into<String>, choice: impl Into<String>) -> Self {
        EventPayload::Vote {
            proposal_id: proposal_id.into(),
            choice: choice.into(),
        }
    }
    
    /// Create an Execution payload
    pub fn execution(receipt_cid: impl Into<String>) -> Self {
        EventPayload::Execution {
            receipt_cid: receipt_cid.into(),
        }
    }
    
    /// Create a Receipt payload
    pub fn receipt(receipt_cid: Cid) -> Self {
        EventPayload::Receipt { receipt_cid }
    }
    
    /// Create a join request payload
    pub fn join_request(
        scope_type: impl Into<String>,
        scope_id: impl Into<String>, 
        scope_genesis_cid: impl Into<String>,
        federation_id: impl Into<String>,
        federation_genesis_cid: impl Into<String>
    ) -> Self {
        EventPayload::JoinRequest {
            scope_type: scope_type.into(),
            scope_id: scope_id.into(),
            scope_genesis_cid: scope_genesis_cid.into(),
            federation_id: federation_id.into(),
            federation_genesis_cid: federation_genesis_cid.into(),
        }
    }
    
    /// Create a join vote payload
    pub fn join_vote(
        join_request_cid: impl Into<String>, 
        choice: impl Into<String>,
        reason: Option<String>
    ) -> Self {
        EventPayload::JoinVote {
            join_request_cid: join_request_cid.into(),
            choice: choice.into(),
            reason,
        }
    }
    
    /// Create a join approval payload
    pub fn join_approval(
        join_request_cid: impl Into<String>,
        attestation_cid: impl Into<String>,
        lineage_cid: impl Into<String>
    ) -> Self {
        EventPayload::JoinApproval {
            join_request_cid: join_request_cid.into(),
            attestation_cid: attestation_cid.into(),
            lineage_cid: lineage_cid.into(),
        }
    }
    
    /// Create a PolicyUpdateProposal payload
    pub fn policy_update_proposal(
        scope_type: impl Into<String>,
        scope_id: impl Into<String>,
        proposed_policy: impl Into<String>,
        proposer_did: impl Into<String>,
        description: impl Into<String>
    ) -> Self {
        EventPayload::PolicyUpdateProposal {
            scope_type: scope_type.into(),
            scope_id: scope_id.into(),
            proposed_policy: proposed_policy.into(),
            proposer_did: proposer_did.into(),
            description: description.into(),
        }
    }
    
    /// Create a PolicyUpdateVote payload
    pub fn policy_update_vote(
        proposal_cid: impl Into<String>,
        choice: impl Into<String>,
        reason: Option<String>,
        voter_did: impl Into<String>
    ) -> Self {
        EventPayload::PolicyUpdateVote {
            proposal_cid: proposal_cid.into(),
            choice: choice.into(),
            reason,
            voter_did: voter_did.into(),
        }
    }
    
    /// Create a PolicyUpdateApproval payload
    pub fn policy_update_approval(
        proposal_cid: impl Into<String>,
        quorum_proof: QuorumProof,
        approver_did: impl Into<String>
    ) -> Self {
        EventPayload::PolicyUpdateApproval {
            proposal_cid: proposal_cid.into(),
            quorum_proof,
            approver_did: approver_did.into(),
        }
    }
    
    /// Create a Custom payload
    pub fn custom(fields: serde_json::Value) -> Self {
        EventPayload::Custom { fields }
    }
}

impl ActionType for EventPayload {
    fn action_type(&self) -> Option<String> {
        match self {
            // Core governance action types
            EventPayload::Proposal { .. } => Some("submit_proposal".to_string()),
            EventPayload::Vote { .. } => Some("submit_vote".to_string()),
            EventPayload::Execution { .. } => Some("execute_proposal".to_string()),
            
            // Join flow action types
            EventPayload::JoinRequest { .. } => Some("submit_join_request".to_string()),
            EventPayload::JoinVote { .. } => Some("submit_join_vote".to_string()),
            EventPayload::JoinApproval { .. } => Some("approve_join_request".to_string()),
            
            // Policy update flow action types
            EventPayload::PolicyUpdateProposal { .. } => Some("submit_policy_update_proposal".to_string()),
            EventPayload::PolicyUpdateVote { .. } => Some("submit_policy_update_vote".to_string()),
            EventPayload::PolicyUpdateApproval { .. } => Some("approve_policy_update_proposal".to_string()),
            
            // Custom and other payloads - would need to be handled in their individual contexts
            EventPayload::Custom { fields } => {
                // Extract action_type field from custom payload if available
                fields.get("action_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            },
            
            // Other payload types that don't require explicit authorization
            _ => None,
        }
    }
} 