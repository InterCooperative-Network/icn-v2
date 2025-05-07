use serde::{Deserialize, Serialize};
use crate::Cid;

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
    
    /// Create a Custom payload
    pub fn custom(fields: serde_json::Value) -> Self {
        EventPayload::Custom { fields }
    }
} 