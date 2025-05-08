use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Signature verification failed")]
    SignatureVerification,

    #[error("Invalid scope: {0}")]
    InvalidScope(String),

    #[error("DAG reference error: {0}")]
    DAGReference(String),

    #[error("Unauthorized operation: {0}")]
    Unauthorized(String),

    #[error("Resource allocation exceeded: {0}")]
    ResourceExceeded(String),

    #[error("Invalid credential: {0}")]
    InvalidCredential(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
} 