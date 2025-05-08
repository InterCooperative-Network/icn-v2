#![doc = "Defines the AsyncStorage trait for pluggable storage backends."]

use async_trait::async_trait;
use cid::Cid;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::error::AgoraError;
use icn_core_types::Cid as AgCid;
#[cfg(feature = "persistence")]
use crate::storage::rocks::RocksDbStorage;

/// Trait for asynchronous storage of IPLD blocks (message bodies, anchors, etc.).
#[async_trait]
pub trait AsyncStorage: Send + Sync {
    /// Puts a raw block of bytes into storage, identified by its CID.
    async fn put_raw(&self, cid: &AgCid, bytes: Arc<Vec<u8>>) -> Result<(), AgoraError>;

    /// Gets a raw block of bytes from storage by its CID.
    async fn get_raw(&self, cid: &AgCid) -> Result<Option<Arc<Vec<u8>>>, AgoraError>;

    /// Checks if a block exists in storage.
    async fn exists(&self, cid: &AgCid) -> Result<bool, AgoraError> {
        Ok(self.get_raw(cid).await?.is_some())
    }

    /// Serializes an IPLD-compatible object to DAG-CBOR and stores it.
    /// Returns the CID (wrapper type) of the stored object.
    async fn put_ipld<T: Serialize + Send + Sync>(&self, data: &T) -> Result<AgCid, AgoraError> {
        let bytes = serde_ipld_dagcbor::to_vec(data)
            .map_err(|e| AgoraError::Serialization(format!("DAG-CBOR encoding: {}", e)))?;
        
        // Calculate CID from the serialized bytes.
        // Hash: SHA2-256 (multicodec 0x12)
        // Codec: DagCbor (multicodec 0x71)
        // Version: V1

        // 1. Calculate hash of the CBOR bytes using sha2 crate
        let digest = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hasher.finalize() // Returns GenericArray
        };

        // 2. Wrap digest in a Multihash object
        let hash_algorithm_code = 0x12; // SHA2-256 multicodec
        // Pass a slice of the digest to wrap.
        let multihash = cid::multihash::Multihash::wrap(hash_algorithm_code, digest.as_slice())
            .map_err(|e| AgoraError::Ipld(format!("Multihash wrap error: {}", e)))?;

        // 3. Define IPLD codec for DagCbor
        let dag_cbor_codec_code = 0x71u64;

        // 4. Create external CID v1
        let external_cid = cid::Cid::new_v1(dag_cbor_codec_code, multihash);
        
        // 5. Create wrapper CID
        let wrapper_cid = AgCid::from(external_cid);

        self.put_raw(&wrapper_cid, Arc::new(bytes)).await?;
        Ok(wrapper_cid) // Return the wrapper CID
    }

    /// Retrieves an IPLD-compatible object from storage by its CID and deserializes it.
    async fn get_ipld<T: DeserializeOwned + Send + Sync>(&self, cid: &AgCid) -> Result<Option<T>, AgoraError> {
        match self.get_raw(cid).await? {
            Some(bytes) => {
                let data: T = serde_ipld_dagcbor::from_slice(&bytes)
                    .map_err(|e| AgoraError::Serialization(format!("DAG-CBOR decoding: {}", e)))?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }
}

/// An in-memory implementation of `AsyncStorage` for testing and prototyping.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorage {
    store: Arc<RwLock<HashMap<AgCid, Arc<Vec<u8>>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Default::default()
    }
}

#[async_trait]
impl AsyncStorage for InMemoryStorage {
    async fn put_raw(&self, cid: &AgCid, bytes: Arc<Vec<u8>>) -> Result<(), AgoraError> {
        let mut store_guard = self.store.write().await;
        store_guard.insert(cid.clone(), bytes);
        Ok(())
    }

    async fn get_raw(&self, cid: &AgCid) -> Result<Option<Arc<Vec<u8>>>, AgoraError> {
        let store_guard = self.store.read().await;
        Ok(store_guard.get(cid).cloned())
    }
}

/// Enum to represent different storage backend implementations.
pub enum StorageBackend {
    InMemory(InMemoryStorage),
    #[cfg(feature = "persistence")]
    Rocks(RocksDbStorage),
}

#[async_trait]
impl AsyncStorage for StorageBackend {
    async fn put_raw(&self, cid: &AgCid, bytes: Arc<Vec<u8>>) -> Result<(), AgoraError> {
        match self {
            StorageBackend::InMemory(s) => s.put_raw(cid, bytes).await,
            #[cfg(feature = "persistence")]
            StorageBackend::Rocks(s) => s.put_raw(cid, bytes).await,
        }
    }

    async fn get_raw(&self, cid: &AgCid) -> Result<Option<Arc<Vec<u8>>>, AgoraError> {
        match self {
            StorageBackend::InMemory(s) => s.get_raw(cid).await,
            #[cfg(feature = "persistence")]
            StorageBackend::Rocks(s) => s.get_raw(cid).await,
        }
    }

    // Default implementations for put_ipld/get_ipld will delegate to put_raw/get_raw
}

// Re-introduce the module structure if needed
#[cfg(feature = "persistence")]
pub mod rocks; // Contains RocksDbStorage 