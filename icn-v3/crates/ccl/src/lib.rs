pub mod error;
pub mod federation;
pub mod cooperative;
pub mod governance;
pub mod attestation;
pub mod quorum;
pub mod recovery;
#[cfg(test)]
mod tests;

pub use error::CclError;
pub use federation::{Federation, FederationManager, FederationMembership};
pub use cooperative::{Cooperative, CooperativeManager};
pub use governance::{Proposal, Vote, VoteOutcome};
pub use attestation::{Attestation, AttestationType, AttestationManager};
pub use quorum::{QuorumPolicy, QuorumProof, QuorumType}; 