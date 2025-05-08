#![deny(unsafe_code)]
//! Planetary Mesh - A decentralized compute fabric for the InterCooperative Network
//! 
//! This crate provides the core functionality for the planetary mesh including:
//! - Node capability management and advertisement
//! - Task scheduling based on node capabilities
//! - Distributed task execution
//! - Energy-aware computation

pub mod node;
pub mod cap_index;
pub mod scheduler;
pub mod manifest_verifier;
pub mod dispatch_credential;
pub mod trusted_did_policy;
pub mod revocation_notice;

// Re-export common types
pub use node::MeshNode;
pub use scheduler::{Scheduler, TaskRequest, TaskBid, MatchResult, CapabilityIndex};
pub use cap_index::CapabilitySelector;
pub use manifest_verifier::{ManifestVerifier, ManifestVerificationError};

pub mod types;
pub use types::{JobManifest, NodeCapability, NodeCapabilityInfo, Bid, JobStatus, ResourceType}; 