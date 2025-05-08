#![doc = "Error types for AgoraNet operations."]

use thiserror::Error;

/// Main error type for the agoranet-core crate.
#[derive(Error, Debug)]
pub enum AgoraError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage operation failed: {0}")]
    Storage(String),

    #[error("Cryptography error: {0}")]
    Cryptography(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("IPLD error: {0}")]
    Ipld(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    // Add other specific error types as needed
} 