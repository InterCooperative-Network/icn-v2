//! icn-types placeholder
//! Defines common data structures used across the ICN v2 workspace.

pub mod anchor;
pub mod bundle;
// pub mod cid; // Removed: types moved to icn-core-types
pub mod dag;
// pub mod identity; // Removed: types moved to icn-core-types
// pub mod quorum; // Removed: types moved to icn-core-types
pub mod receipts;
pub mod resources;

// Re-export core types for easier access
pub use anchor::AnchorRef;
// pub use bundle::TrustBundle; // Commented out due to cfg issues
// pub use cid::Cid; // Removed: Use re-export from icn_core_types
pub use dag::{DagError, DagNode, DagStore, SignedDagNode, PublicKeyResolver};
// pub use identity::{Did, DidKey, DidKeyError}; // Removed: Use re-export from icn_core_types
// pub use quorum::QuorumProof; // Removed: Use re-export from icn_core_types
pub use receipts::ExecutionReceipt; // Uncommented

// Re-export sync types
pub use dag::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};

// Re-export core types from icn-core-types for convenience
pub use icn_core_types::{Cid, CidError, Did, QuorumProof};
pub use icn_core_types::did::DidParseError;

// Re-export types from modules
pub use bundle::{TrustBundle, TrustBundleError};
pub use dag::{DagNodeBuilder, DagPayload};

// Add mesh module and re-exports
pub mod mesh;
pub use mesh::{JobManifest, Bid, NodeCapability, ResourceType};

// Conditional exports based on features
#[cfg(feature = "async")]
pub use dag::sync::*;

#[cfg(feature = "persistence")]
pub use dag::rocksdb::RocksDbDagStore;