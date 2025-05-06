use super::{TrustBundleStore, StorageError};
use crate::trustbundle::TrustBundle;
use async_trait::async_trait;
use rocksdb::{DB, Options, ColumnFamilyDescriptor, IteratorMode};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// RocksDB column families for TrustBundle storage
const CF_BUNDLES: &str = "bundles";
const CF_FEDERATION_INDEX: &str = "federation_index";

/// RocksDB implementation of TrustBundleStore
pub struct RocksDbTrustBundleStore {
    db: Arc<RwLock<DB>>,
}

impl RocksDbTrustBundleStore {
    /// Create a new RocksDB-backed TrustBundle store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        // Create necessary column families if they don't exist
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_BUNDLES, Options::default()),
            ColumnFamilyDescriptor::new(CF_FEDERATION_INDEX, Options::default()),
        ];
        
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(Self {
            db: Arc::new(RwLock::new(db)),
        })
    }
    
    /// Helper to serialize a TrustBundle
    fn serialize_bundle(bundle: &TrustBundle) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(bundle).map_err(StorageError::Serialization)
    }
    
    /// Helper to deserialize a TrustBundle
    fn deserialize_bundle(bytes: &[u8]) -> Result<TrustBundle, StorageError> {
        serde_json::from_slice(bytes).map_err(StorageError::Serialization)
    }
    
    /// Helper to generate a bundle ID if none exists
    fn get_bundle_id(bundle: &TrustBundle) -> String {
        if let Some(cid) = &bundle.bundle_cid {
            cid.clone()
        } else {
            // If no CID, use federation ID + timestamp
            format!("{}:{}", bundle.federation_id, bundle.timestamp)
        }
    }
    
    /// Helper to create the key for the federation index
    fn federation_key(federation_id: &str) -> Vec<u8> {
        federation_id.as_bytes().to_vec()
    }
    
    /// Helper to serialize a list of bundle IDs
    fn serialize_bundle_ids(bundle_ids: &[String]) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(bundle_ids).map_err(StorageError::Serialization)
    }
    
    /// Helper to deserialize a list of bundle IDs
    fn deserialize_bundle_ids(bytes: &[u8]) -> Result<Vec<String>, StorageError> {
        serde_json::from_slice(bytes).map_err(StorageError::Serialization)
    }
    
    /// Helper to update the federation index
    async fn update_federation_index(
        &self,
        federation_id: &str,
        bundle_id: &str,
        add: bool,
    ) -> Result<(), StorageError> {
        let db = self.db.write().await;
        let cf_federation = db.cf_handle(CF_FEDERATION_INDEX)
            .ok_or_else(|| StorageError::Backend("Federation index column family not found".to_string()))?;
        
        let federation_key = Self::federation_key(federation_id);
        
        // Get the current index
        let mut bundle_ids = match db.get_cf(&cf_federation, &federation_key) {
            Ok(Some(bytes)) => Self::deserialize_bundle_ids(&bytes)?,
            Ok(None) => Vec::new(),
            Err(e) => return Err(StorageError::Backend(e.to_string())),
        };
        
        // Update the index based on add/remove
        if add {
            bundle_ids.push(bundle_id.to_string());
        } else {
            bundle_ids.retain(|id| id != bundle_id);
        }
        
        // Write the updated index
        let serialized = Self::serialize_bundle_ids(&bundle_ids)?;
        db.put_cf(&cf_federation, &federation_key, serialized)
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        Ok(())
    }
}

#[async_trait]
impl TrustBundleStore for RocksDbTrustBundleStore {
    async fn store(&self, bundle: TrustBundle) -> Result<String, StorageError> {
        let bundle_id = Self::get_bundle_id(&bundle);
        let serialized = Self::serialize_bundle(&bundle)?;
        
        let db = self.db.write().await;
        let cf_bundles = db.cf_handle(CF_BUNDLES)
            .ok_or_else(|| StorageError::Backend("Bundles column family not found".to_string()))?;
        
        // Store the bundle
        db.put_cf(&cf_bundles, bundle_id.as_bytes(), serialized)
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        // Update the federation index
        drop(db); // Release the lock before another async call
        self.update_federation_index(&bundle.federation_id, &bundle_id, true).await?;
        
        Ok(bundle_id)
    }
    
    async fn get(&self, bundle_id: &str) -> Result<TrustBundle, StorageError> {
        let db = self.db.read().await;
        let cf_bundles = db.cf_handle(CF_BUNDLES)
            .ok_or_else(|| StorageError::Backend("Bundles column family not found".to_string()))?;
        
        // Get the bundle
        let bytes = db.get_cf(&cf_bundles, bundle_id.as_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound(bundle_id.to_string()))?;
        
        // Deserialize
        Self::deserialize_bundle(&bytes)
    }
    
    async fn delete(&self, bundle_id: &str) -> Result<(), StorageError> {
        // First, get the bundle to find its federation
        let bundle = self.get(bundle_id).await?;
        let federation_id = bundle.federation_id.clone();
        
        // Now delete the bundle
        let db = self.db.write().await;
        let cf_bundles = db.cf_handle(CF_BUNDLES)
            .ok_or_else(|| StorageError::Backend("Bundles column family not found".to_string()))?;
        
        db.delete_cf(&cf_bundles, bundle_id.as_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        
        // Update the federation index
        drop(db); // Release the lock before another async call
        self.update_federation_index(&federation_id, bundle_id, false).await?;
        
        Ok(())
    }
    
    async fn list_by_federation(&self, federation_id: &str) -> Result<Vec<TrustBundle>, StorageError> {
        let db = self.db.read().await;
        let cf_federation = db.cf_handle(CF_FEDERATION_INDEX)
            .ok_or_else(|| StorageError::Backend("Federation index column family not found".to_string()))?;
        let cf_bundles = db.cf_handle(CF_BUNDLES)
            .ok_or_else(|| StorageError::Backend("Bundles column family not found".to_string()))?;
        
        // Get the list of bundle IDs for this federation
        let federation_key = Self::federation_key(federation_id);
        let bytes = db.get_cf(&cf_federation, &federation_key)
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .ok_or_else(|| StorageError::InvalidFederation(federation_id.to_string()))?;
        
        let bundle_ids = Self::deserialize_bundle_ids(&bytes)?;
        
        // Collect the bundles
        let mut result = Vec::new();
        for id in bundle_ids {
            if let Ok(Some(bytes)) = db.get_cf(&cf_bundles, id.as_bytes()) {
                match Self::deserialize_bundle(&bytes) {
                    Ok(bundle) => result.push(bundle),
                    Err(e) => return Err(e),
                }
            }
        }
        
        Ok(result)
    }
    
    async fn get_latest(&self, federation_id: &str) -> Result<TrustBundle, StorageError> {
        let db = self.db.read().await;
        let cf_federation = db.cf_handle(CF_FEDERATION_INDEX)
            .ok_or_else(|| StorageError::Backend("Federation index column family not found".to_string()))?;
        let cf_bundles = db.cf_handle(CF_BUNDLES)
            .ok_or_else(|| StorageError::Backend("Bundles column family not found".to_string()))?;
        
        // Get the list of bundle IDs for this federation
        let federation_key = Self::federation_key(federation_id);
        let bytes = db.get_cf(&cf_federation, &federation_key)
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .ok_or_else(|| StorageError::InvalidFederation(federation_id.to_string()))?;
        
        let bundle_ids = Self::deserialize_bundle_ids(&bytes)?;
        
        if bundle_ids.is_empty() {
            return Err(StorageError::NotFound(federation_id.to_string()));
        }
        
        // Get the last (newest) bundle ID
        let latest_id = bundle_ids.last().unwrap();
        
        // Get the bundle
        let bytes = db.get_cf(&cf_bundles, latest_id.as_bytes())
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound(latest_id.to_string()))?;
        
        // Deserialize
        Self::deserialize_bundle(&bytes)
    }
} 