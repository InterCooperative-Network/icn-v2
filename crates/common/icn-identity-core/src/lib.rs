#![deny(unsafe_code)]
//! icn-identity-core placeholder
//! Handles core identity operations, VC issuance, and quorum validation.

pub mod did;

pub mod quorum;
pub mod vc;
// pub mod signature; // Assuming these were commented out
pub mod manifest;
pub mod trustbundle;
// pub mod policy; // Assuming these were commented out or didn't exist yet
// pub mod did_type;

// Re-export key structs/functions
// pub use did::{DidKey, DidKeyError};
// pub use quorum::{QuorumValidator, QuorumError}; // Commenting out unresolved re-exports
pub use quorum::{QuorumEngine, QuorumTally, QuorumOutcome, QuorumEngineError};
pub use vc::{VerifiableCredential, VcIssuer};
// pub use trustbundle::{TrustBundle, QuorumConfig, QuorumType, QuorumProof, TrustError};
// pub use trustbundle::storage::{TrustBundleStore, MemoryTrustBundleStore, StorageError};
// pub use signature::Signature;
// pub use manifest::{AgentManifest, ManifestError};
// pub use did_type::Did;

// Re-export VC types
pub use vc::execution_receipt::{
    ExecutionReceipt, 
    ExecutionSubject, 
    ExecutionScope,
    ExecutionStatus,
    Proof as ExecutionReceiptProof,
    ExecutionReceiptError
};

pub use vc::{
    ProposalCredential,
    ProposalSubject,
    ProposalType,
    ProposalStatus,
    VotingThreshold,
    VotingDuration,
    ProposalError,
    VoteCredential,
    VoteSubject,
    VoteDecision,
    VoteError
};

// pub use crate::did::Did; // Still keep commented
// pub use crate::did::DidKey; // Still keep commented

pub use ed25519_dalek::VerifyingKey; 
pub use ed25519_dalek::Signature;
pub use ed25519_dalek::Signer;
pub use ed25519_dalek::Verifier;

pub fn placeholder() {}
