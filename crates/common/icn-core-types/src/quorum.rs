use crate::Cid;
use crate::did::Did;
// use ed25519_dalek::Signature; // Removed unused import
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf; // Import ByteBuf for potential signature serialization

/// Represents a set of signatures that satisfy a quorum policy for a specific data CID.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QuorumProof {
    /// The Content ID of the data being attested to.
    pub data_cid: Cid,
    /// The policy identifier or version that defines the quorum requirements.
    pub policy_id: String, // Could be a CID or a predefined identifier
    
    // Using ByteBuf for signatures might be more flexible for serde
    /// The list of DIDs and their corresponding signatures.
    pub signatures: Vec<(Did, ByteBuf)>,
    
    /// Optional metadata relevant to the proof (e.g., timestamp, context).
    pub metadata: Option<serde_json::Value>,
}

// Note: ed25519_dalek::Signature doesn't directly support Serialize/Deserialize.
// Common practice is to serialize the bytes using serde_bytes::ByteBuf or similar.
// We might need helper methods or different types if direct Signature serialization is needed. 