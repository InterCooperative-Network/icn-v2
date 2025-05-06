use crate::anchor::AnchorRef;
use crate::cid::Cid;
use crate::quorum::QuorumProof;
use serde::{Deserialize, Serialize};

/// A core data structure in ICN, representing a stateful object anchored to the DAG
/// and secured by a quorum proof.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrustBundle {
    /// The Content ID of the current state data associated with this bundle.
    pub state_cid: Cid,
    /// Proof that the current state is valid according to the governing policy.
    pub state_proof: QuorumProof,
    /// References to previous TrustBundles or other relevant DAG nodes this bundle builds upon.
    pub previous_anchors: Vec<AnchorRef>,
    /// Optional metadata about the bundle itself.
    pub metadata: Option<serde_json::Value>,
} 