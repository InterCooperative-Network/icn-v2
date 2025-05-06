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
    
    /// Create a Custom payload
    pub fn custom(fields: serde_json::Value) -> Self {
        EventPayload::Custom { fields }
    }
} 