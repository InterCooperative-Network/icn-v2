#![doc = "Defines the core Message structure and body content types."]

use icn_core_types::{Cid, Did};
use serde::{Deserialize, Serialize};

use crate::error::AgoraError;

/// Represents a message in an AgoraNet thread.
/// The body of the message is stored separately (e.g., in IPFS/S3)
/// and referenced by `body_cid`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Message {
    /// DID of the sender/author of the message.
    pub author: Did,
    /// Optional CID of the parent message in the thread, forming a DAG structure.
    pub parent: Option<Cid>,
    /// CID of the IPLD-serialized message body.
    pub body_cid: Cid,
    /// Signature of `author_did | parent_cid (or null) | body_cid | timestamp` by the author's key.
    /// The exact serialization for signing needs to be strictly defined.
    pub signature: Vec<u8>,
    /// Unix timestamp (seconds since epoch) of when the message was created/signed.
    pub timestamp: i64,
}

impl Message {
    /// Placeholder for a method to create the canonical byte representation for signing.
    pub fn to_canonical_bytes_for_signing(&self) -> Result<Vec<u8>, AgoraError> {
        // TODO: Define strict serialization. Example:
        // format!("{}{}{}{}", 
        //     self.author.as_ref(), 
        //     self.parent.map(|c| c.to_string()).unwrap_or_default(), 
        //     self.body_cid.to_string(),
        //     self.timestamp
        // ).into_bytes()
        // For now, returning a simple concatenation for structure.
        // IMPORTANT: This MUST be a canonical, deterministic serialization.
        let parent_str = self.parent.as_ref().map_or(String::new(), |c| c.to_string());
        Ok([
            self.author.to_string().as_bytes(),
            parent_str.as_bytes(),
            self.body_cid.to_string().as_bytes(),
            &self.timestamp.to_le_bytes(),
        ].concat())
    }
}

/// Enum representing the different types of content a message body can have.
/// This enum itself would be IPLD-serialized and its CID stored in `Message::body_cid`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive] // As suggested, for future-proofing
pub enum Body {
    /// A formal proposal.
    Proposal(ProposalBody),
    /// A general comment on a message or proposal.
    Comment(CommentBody),
    /// An amendment to an existing proposal.
    Amendment(AmendmentBody),
    /// A simple emoji reaction to a message.
    Reaction(String), // e.g., "👍", "🚀"
}

/// Content of a proposal message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProposalBody {
    pub title: String,
    pub description: String,
    // Example: Link to external spec, or embedded CCL/WASM CID
    pub content_cid: Option<Cid>,
    pub proposed_action: String, // e.g., "ExecuteWasm", "UpdatePolicy"
}

/// Content of a comment message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommentBody {
    pub text: String,
}

/// Content of an amendment message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AmendmentBody {
    pub target_proposal_cid: Cid, // CID of the Proposal Body being amended
    pub suggested_changes: String, // Textual description or diff
    pub new_content_cid: Option<Cid>, // Optional CID of new full content if applicable
}

/// Represents an anchor point in an AgoraNet thread, linking to the latest message
/// at the time of anchoring. This structure itself is stored via IPLD.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ThreadAnchor {
    /// CID of the latest message envelope included in this anchor.
    pub tail: Cid,
    /// Unix timestamp (seconds since epoch) when the anchor was created.
    pub timestamp: i64,
} 