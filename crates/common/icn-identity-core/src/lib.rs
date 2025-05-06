//! icn-identity-core placeholder
//! Handles core identity operations, VC issuance, and quorum validation.

pub mod did;
pub mod quorum;
pub mod vc;
pub mod signature;
pub mod manifest;

// Re-export key structs/functions
pub use did::{DidKey, DidKeyError};
pub use quorum::{QuorumValidator, QuorumError};
pub use vc::{VerifiableCredential, VcIssuer};
pub use signature::Signature;

pub fn placeholder() {}
