use thiserror::Error;
use icn_types::dag::DagError;
// TODO: Add imports for other error types used (DidKeyError, CborError, NetworkError etc.)
// use icn_identity_core::did::DidKeyError; 
use std::io;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON Serialization/Deserialization Error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("DAG Operation Error: {0}")]
    Dag(#[from] DagError),

    // #[error("DID Key Error: {0}")]
    // DidKey(#[from] DidKeyError), // Uncomment when DidKeyError is importable/defined
    #[error("DID Key Error: {0}")]
    DidKey(String),

    #[error("CBOR Serialization/Deserialization Error: {0}")]
    Cbor(String), // Can refine this based on cbor error types

    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("Invalid Input: {0}")]
    Input(String),

    #[error("Network Error: {0}")]
    Network(String), // For sync/libp2p errors

    #[error("Verification Error: {0}")]
    Verification(String),

    #[error("Prometheus Metric Error: {0}")]
    Metrics(#[from] prometheus::Error),

    #[error("Generic Error: {0}")]
    Any(#[from] anyhow::Error), // Catch-all for other anyhow errors
}

// Define the standard Result type alias
pub type CliResult<T = ()> = Result<T, CliError>; 