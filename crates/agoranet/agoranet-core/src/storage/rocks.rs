#![cfg(feature = "persistence")]

use crate::storage::AsyncStorage;
use crate::error::AgoraError;
use icn_core_types::Cid;
use anyhow::{Context, Result};
use async_trait::async_trait;
use rocksdb::{Options, DB};
use std::path::Path;
use std::sync::Arc;

/// RocksDB-backed storage implementation.
#[derive(Debug)]
pub struct RocksDbStorage {
    // Store the DB itself. Use Arc for potential future sharing across threads if needed,
    // though RocksDB itself can be thread-safe depending on usage.
    db: Arc<DB>,
}

impl RocksDbStorage {
    /// Opens or creates a RocksDB database at the specified path.
    pub fn open(path: &Path) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        // Ensure the directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create RocksDB directory: {}", parent.display()))?;
        }
        
        let db = DB::open(&opts, path)
            .with_context(|| format!("Failed to open RocksDB database at {}", path.display()))?;
        
        Ok(Self { db: Arc::new(db) })
    }
}

#[async_trait]
impl AsyncStorage for RocksDbStorage {
    async fn put_raw(&self, cid: &Cid, bytes: Arc<Vec<u8>>) -> Result<(), AgoraError> {
        self.db
            .put(cid.to_bytes(), &*bytes)
            .map_err(|e| AgoraError::Storage(format!("RocksDB put failed: {}", e)))?;
        Ok(())
    }

    async fn get_raw(&self, cid: &Cid) -> Result<Option<Arc<Vec<u8>>>, AgoraError> {
        match self.db.get(cid.to_bytes()) {
            Ok(Some(v)) => Ok(Some(Arc::new(v))),
            Ok(None) => Ok(None),
            Err(e) => Err(AgoraError::Storage(format!("RocksDB get failed: {}", e))),
        }
    }

    // get_ipld/put_ipld are handled by the default trait methods
} 