//! icn-identity-core placeholder
//! Handles core identity operations, VC issuance, and quorum validation.

pub mod did;
pub mod quorum;
pub mod vc;
// pub mod signature;
pub mod manifest;
pub mod trustbundle;
// pub mod did_type;

// Re-export key structs/functions
// pub use did::{DidKey, DidKeyError};
pub use quorum::{QuorumValidator, QuorumError};
pub use vc::{VerifiableCredential, VcIssuer};
pub use trustbundle::{TrustBundle, QuorumConfig, QuorumType, QuorumProof, TrustError};
pub use trustbundle::storage::{TrustBundleStore, MemoryTrustBundleStore, StorageError};
// pub use signature::Signature;
// pub use manifest::{AgentManifest, ManifestError};
// pub use did_type::Did;

pub fn placeholder() {}
