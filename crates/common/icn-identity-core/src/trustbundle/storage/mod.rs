use super::{TrustBundle, TrustError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;

mod memory;
#[cfg(feature = "rocksdb")]
pub mod rocksdb;

pub use memory::MemoryTrustBundleStore;
#[cfg(feature = "rocksdb")]
pub use rocksdb::RocksDbTrustBundleStore;

/// Errors that can occur when working with TrustBundle storage
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Bundle not found with ID: {0}")]
    NotFound(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Trust bundle error: {0}")]
    TrustError(#[from] TrustError),
    
    #[error("Storage backend error: {0}")]
    Backend(String),
    
    #[error("Invalid federation ID: {0}")]
    InvalidFederation(String),
}

/// Trait defining the interface for TrustBundle storage backends
#[async_trait]
pub trait TrustBundleStore: Send + Sync {
    /// Store a TrustBundle, returning its CID (or an internally generated ID if no CID exists)
    async fn store(&self, bundle: TrustBundle) -> Result<String, StorageError>;
    
    /// Retrieve a TrustBundle by its CID or ID
    async fn get(&self, bundle_id: &str) -> Result<TrustBundle, StorageError>;
    
    /// Delete a TrustBundle (if supported by the storage backend)
    async fn delete(&self, bundle_id: &str) -> Result<(), StorageError>;
    
    /// List all TrustBundles for a federation
    async fn list_by_federation(&self, federation_id: &str) -> Result<Vec<TrustBundle>, StorageError>;
    
    /// Get the latest TrustBundle for a federation
    async fn get_latest(&self, federation_id: &str) -> Result<TrustBundle, StorageError>;
} 