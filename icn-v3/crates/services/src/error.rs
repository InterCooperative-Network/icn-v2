use icn_common::error::CommonError;
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Common error: {0}")]
    Common(#[from] CommonError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("DAG lineage error: {0}")]
    DagLineage(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Scope violation: {0}")]
    ScopeViolation(String),

    #[error("Verification error: {0}")]
    Verification(String),

    #[error("Replay verification failed: {0}")]
    ReplayVerification(String),

    #[error("Unauthorized operation: {0}")]
    Unauthorized(String),
} 