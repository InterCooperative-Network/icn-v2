//! icn-types placeholder
//! Defines common data structures used across the ICN v2 workspace.

pub mod anchor;
pub mod bundle;
pub mod cid;
pub mod identity;
pub mod quorum;
pub mod receipts;

// Re-export core types for easier access
pub use anchor::AnchorRef;
pub use bundle::TrustBundle;
pub use cid::Cid;
pub use identity::Did;
pub use quorum::QuorumProof;
pub use receipts::ExecutionReceipt;

pub fn placeholder() {}
