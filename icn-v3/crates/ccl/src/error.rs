use icn_common::error::CommonError;
use icn_services::ServiceError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CclError {
    #[error("Common error: {0}")]
    Common(#[from] CommonError),

    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    #[error("Federation error: {0}")]
    Federation(String),

    #[error("Cooperative error: {0}")]
    Cooperative(String),

    #[error("Membership error: {0}")]
    Membership(String),

    #[error("Governance error: {0}")]
    Governance(String),

    #[error("Quorum error: {0}")]
    Quorum(String),

    #[error("Attestation error: {0}")]
    Attestation(String),

    #[error("Recovery error: {0}")]
    Recovery(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Unauthorized access: {0}")]
    Unauthorized(String),
} 