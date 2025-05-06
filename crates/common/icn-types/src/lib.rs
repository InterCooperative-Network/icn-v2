//! icn-types placeholder
//! Defines common data structures used across the ICN v2 workspace.

pub mod anchor;
pub mod bundle;
pub mod cid;
pub mod dag;
pub mod identity;
pub mod quorum;
pub mod receipts;
pub mod resources;

// Re-export core types for easier access
pub use anchor::AnchorRef;
// pub use bundle::TrustBundle; // Commented out due to cfg issues
pub use cid::Cid;
pub use dag::{DagError, DagNode, DagStore, SignedDagNode, PublicKeyResolver};
pub use icn_identity_core::Did; // Re-export Did from core
pub use quorum::QuorumProof;
// pub use receipts::ExecutionReceipt; // Commented out due to cfg issues
pub use resources::ResourceEnvelope;

// Re-export sync types
pub use dag::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};
