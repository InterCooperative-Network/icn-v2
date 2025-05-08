use super::{TrustBundle, TrustError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use serde::{Serialize, Deserialize};

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

/// Represents a TrustBundle as it is stored, potentially with extra store-specific metadata like an ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredTrustBundle {
    pub id: String,
    pub federation_id: Option<String>,
    pub bundle_type: String,
    pub bundle_content: TrustBundle,
    pub created_at: String,
    pub anchored_cid: Option<String>,
}

/// Trait defining the interface for TrustBundle storage backends
#[async_trait]
pub trait TrustBundleStore: Send + Sync {
    /// Save a bundle
    async fn save_bundle(&self, bundle: &StoredTrustBundle) -> Result<(), StorageError>;

    /// Get a bundle
    async fn get_bundle(&self, id: &str) -> Result<Option<StoredTrustBundle>, StorageError>;

    /// List bundles by federation
    async fn list_bundles_by_federation(&self, federation_id: &str) -> Result<Vec<StoredTrustBundle>, StorageError>;

    /// Get the latest bundle ID by federation
    async fn get_latest_bundle_id_by_federation(&self, federation_id: &str) -> Result<Option<String>, StorageError>;

    /// Get the latest bundle by federation
    async fn get_latest_bundle_by_federation(&self, federation_id: &str) -> Result<Option<StoredTrustBundle>, StorageError>;

    /// Remove a bundle
    async fn remove_bundle(&self, bundle_id: &str) -> Result<(), StorageError>;
} 