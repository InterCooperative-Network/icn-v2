//! ICN Wallet SDK for dispatch credential verification
//!
//! Provides functionality for verifying ICN dispatch credentials against
//! trust policies and revocation notices in a local or remote DAG.

mod verification;

pub use verification::{
    VerificationReport,
    verify_dispatch_credential,
    verify_dispatch_credential_json,
    TrustPolicyStore,
    TrustedDidEntry,
    TrustLevel,
    RevocationEntry,
    RevocationType,
};

/// Verify a dispatch credential using the wallet SDK
pub fn verify_credential(json: &str) -> String {
    match verification::verify_dispatch_credential_json(json) {
        Ok(result) => result,
        Err(e) => format!("{{\"error\": \"{}\"}}", e),
    }
}
