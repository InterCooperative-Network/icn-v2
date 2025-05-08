use icn_common::error::CommonError;
use icn_services::ServiceError;
use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Common error: {0}")]
    Common(#[from] CommonError),

    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("WASM error: {0}")]
    Wasm(String),

    #[error("WASM validation error: {0}")]
    WasmValidation(String),

    #[error("WASM compilation error: {0}")]
    WasmCompilation(String),

    #[error("WASM instantiation error: {0}")]
    WasmInstantiation(String),

    #[error("WASM execution error: {0}")]
    WasmExecution(String),

    #[error("Host function error: {0}")]
    HostFunction(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Resource allocation error: {0}")]
    ResourceAllocation(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Timeout: execution exceeded time limit")]
    Timeout,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for RuntimeError {
    fn from(error: anyhow::Error) -> Self {
        RuntimeError::Unknown(error.to_string())
    }
}

impl From<wasmtime::Error> for RuntimeError {
    fn from(error: wasmtime::Error) -> Self {
        RuntimeError::Wasm(error.to_string())
    }
} 