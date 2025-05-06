use crate::cid::Cid;
use crate::identity::Did;
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};

/// Represents a set of signatures that satisfy a quorum policy for a specific data CID.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QuorumProof {
    /// The Content ID of the data being attested to.
    pub data_cid: Cid,
    /// The policy identifier or version that defines the quorum requirements.
    pub policy_id: String, // Could be a CID or a predefined identifier
    /// The list of DIDs and their corresponding signatures.
    pub signatures: Vec<(Did, Signature)>,
    /// Optional metadata relevant to the proof (e.g., timestamp, context).
    pub metadata: Option<serde_json::Value>,
} 