#![deny(unsafe_code)]
//! ICN Wallet SDK for dispatch credential verification
//!
//! Provides functionality for verifying ICN dispatch credentials against
//! trust policies and revocation notices in a local or remote DAG.

pub mod mobile; // For UniFFI bindings if still structured this way
pub mod verification; // Existing verification logic
pub mod receipt_store; // NEW: For storing and managing ExecutionReceipts

// --- Top-Level Re-exports --- 

// Errors (example - define WalletError enum if you have one)
// pub use error::WalletError;

// Core Wallet API (example - define your main Wallet struct/trait)
// pub use manager::WalletManager;

// Verification functions from the existing module
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

// Receipt Store components
pub use receipt_store::{StoredReceipt, ReceiptFilter, WalletReceiptStore, InMemoryWalletReceiptStore};

/// Verify a dispatch credential using the wallet SDK
pub fn verify_credential(json: &str) -> String {
    match verification::verify_dispatch_credential_json(json) {
        Ok(result) => result,
        Err(e) => format!("{{\"error\": \"{}\"}}", e),
    }
}
