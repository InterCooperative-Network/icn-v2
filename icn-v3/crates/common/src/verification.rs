use crate::error::CommonError;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

/// A cryptographic signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub Vec<u8>);

/// Trait for verifiable objects
pub trait Verifiable {
    /// Verify the integrity and authenticity of this object
    fn verify(&self) -> Result<bool, CommonError>;
}

/// Trait for asynchronous verification, used for objects that may require
/// remote lookups or complex validation
#[async_trait]
pub trait AsyncVerifiable {
    /// Verify the integrity and authenticity of this object asynchronously
    async fn verify_async(&self) -> Result<bool, CommonError>;
} 