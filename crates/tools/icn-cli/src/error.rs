use thiserror::Error;
use icn_types::dag::DagError;
// TODO: Add imports for other error types used (DidKeyError, CborError, NetworkError etc.)
// use icn_identity_core::did::DidKeyError; 
use std::io;
use icn_core_types::did::DidParseError;
use std::path::PathBuf;
use icn_identity_core::did::DidKeyError;
use icn_identity_core::vc::execution_receipt::ExecutionReceiptError;
use icn_identity_core::trustbundle::TrustError;
use icn_core_types::CidError;
use icn_runtime::dag_indexing::IndexError;

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
    #[allow(dead_code)]
    DidKey(String),

    #[error("CBOR Serialization/Deserialization Error: {0}")]
    #[allow(dead_code)]
    Cbor(String), // Can refine this based on cbor error types

    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("Invalid Input: {0}")]
    #[allow(dead_code)]
    Input(String),

    #[error("Network Error: {0}")]
    #[allow(dead_code)]
    Network(String), // For sync/libp2p errors

    #[error("Verification Error: {0}")]
    #[allow(dead_code)]
    Verification(String),

    #[error("Invalid CID Format: {0}")]
    InvalidCidFormat(String),

    // #[error("Prometheus Metric Error: {0}")]
    // Metrics(#[from] prometheus::Error), // Commented out

    #[error("Generic Error: {0}")]
    Any(#[from] anyhow::Error), // Catch-all for other anyhow errors
    
    // Additional error variants we need for mesh commands
    
    #[error("Invalid Argument: {0}")]
    InvalidArgument(String),
    
    #[error("Not Found: {0}")]
    NotFound(String),
    
    #[error("Verification Failed: {0}")]
    VerificationFailed(String),
    
    #[error("Invalid Key: {0}")]
    InvalidKey(String),
    
    #[error("Identity Error: {0}")]
    IdentityError(String),
    
    #[error("Serialization Error: {0}")]
    SerializationError(String),
    
    #[error("Unimplemented: {0}")]
    Unimplemented(String),
    
    #[error("Other Error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
    
    #[error("IO Error: {0}")]
    IoError(String),
    
    #[error("DAG Error: {0}")]
    DagError(String),

    #[error("Invalid DID format: {0}")]
    InvalidDidFormat(String),

    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("Invalid file path: {0}")]
    InvalidPath(PathBuf),
    
    #[error("Serialization/Deserialization error: {0}")]
    Serialization(String),

    #[error("Index error: {0}")]
    IndexError(#[from] IndexError),

    #[error("WASM execution failed: {0}")]
    WasmExecError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// Define the standard Result type alias
pub type CliResult<T = ()> = Result<T, CliError>;

// Implementation of From trait for DidParseError
impl From<DidParseError> for CliError {
    fn from(err: DidParseError) -> Self {
        CliError::IdentityError(format!("DID Parse Error: {}", err))
    }
}

// Implement From<toml::de::Error> and From<toml::ser::Error>
// impl From<toml::de::Error> for CliError { ... }
// impl From<toml::ser::Error> for CliError { ... }

// Implement From<serde_json::Error>
// impl From<serde_json::Error> for CliError { ... } 