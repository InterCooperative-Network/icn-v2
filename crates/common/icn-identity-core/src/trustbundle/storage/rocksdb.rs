use super::{TrustBundleStore, StorageError, StoredTrustBundle};
use crate::trustbundle::TrustBundle;
use async_trait::async_trait;
use rocksdb::{DB, Options, ColumnFamilyDescriptor};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// RocksDB column families for TrustBundle storage
const CF_BUNDLES: &str = "bundles";
const CF_FEDERATION_INDEX: &str = "federation_index";

/// RocksDB implementation of TrustBundleStore
pub struct RocksDbTrustBundleStore {
    db: Arc<DB>,
}

impl RocksDbTrustBundleStore {
    /// Create a new RocksDB-backed TrustBundle store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_BUNDLES, Options::default()),
            ColumnFamilyDescriptor::new(CF_FEDERATION_INDEX, Options::default()),
        ];
        
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| StorageError::Backend(format!("Failed to open DB: {}", e)))?;
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
    
    /// Helper to serialize StoredTrustBundle (which contains TrustBundle)
    fn serialize_stored_bundle(bundle: &StoredTrustBundle) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(bundle).map_err(StorageError::Serialization)
    }
    
    /// Helper to deserialize StoredTrustBundle
    fn deserialize_stored_bundle(bytes: &[u8]) -> Result<StoredTrustBundle, StorageError> {
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
    async fn update_federation_index_internal(
        db_arc: Arc<DB>,
        federation_id: String,
        bundle_id: String,
        add: bool,
    ) -> Result<(), StorageError> {
        tokio::task::spawn_blocking(move || {
            let cf_federation = db_arc.cf_handle(CF_FEDERATION_INDEX)
                .ok_or_else(|| StorageError::Backend("Federation index CF not found".to_string()))?;
            
            let federation_key = Self::federation_key(&federation_id);
            
            let mut bundle_ids = match db_arc.get_cf(cf_federation, &federation_key) {
                Ok(Some(bytes)) => Self::deserialize_bundle_ids(&bytes)?,
                Ok(None) => Vec::new(),
                Err(e) => return Err(StorageError::Backend(format!("DB get_cf error: {}", e))),
            };
            
            if add {
                if !bundle_ids.contains(&bundle_id) {
                    bundle_ids.push(bundle_id);
                }
            } else {
                bundle_ids.retain(|id| id != &bundle_id);
            }
            
            let serialized = Self::serialize_bundle_ids(&bundle_ids)?;
            db_arc.put_cf(cf_federation, &federation_key, serialized)
                .map_err(|e| StorageError::Backend(format!("DB put_cf error: {}", e)))?;
            Ok(())
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))?
    }
}

#[async_trait]
impl TrustBundleStore for RocksDbTrustBundleStore {
    async fn save_bundle(&self, bundle: &StoredTrustBundle) -> Result<(), StorageError> {
        let serialized_bundle = Self::serialize_stored_bundle(bundle)?;
        let bundle_id_bytes = bundle.id.as_bytes().to_vec();
        
        let db_clone = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let cf_bundles = db_clone.cf_handle(CF_BUNDLES)
                .ok_or_else(|| StorageError::Backend("Bundles CF not found".to_string()))?;
            db_clone.put_cf(cf_bundles, &bundle_id_bytes, serialized_bundle)
                .map_err(|e| StorageError::Backend(format!("DB put_cf error: {}", e)))
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))??;

        if let Some(fed_id) = &bundle.federation_id {
            Self::update_federation_index_internal(Arc::clone(&self.db), fed_id.clone(), bundle.id.clone(), true).await?;
        }
        Ok(())
    }
    
    async fn get_bundle(&self, id: &str) -> Result<Option<StoredTrustBundle>, StorageError> {
        let bundle_id_bytes = id.as_bytes().to_vec();
        let db_clone = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let cf_bundles = db_clone.cf_handle(CF_BUNDLES)
                .ok_or_else(|| StorageError::Backend("Bundles CF not found".to_string()))?;
            match db_clone.get_cf(cf_bundles, &bundle_id_bytes) {
                Ok(Some(bytes)) => Self::deserialize_stored_bundle(&bytes).map(Some),
                Ok(None) => Ok(None),
                Err(e) => Err(StorageError::Backend(format!("DB get_cf error: {}", e))),
            }
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))?
    }
    
    async fn remove_bundle(&self, bundle_id: &str) -> Result<(), StorageError> {
        // First, get the bundle to find its federation ID
        let stored_bundle_opt = self.get_bundle(bundle_id).await?;
        
        let federation_id = match stored_bundle_opt {
            Some(ref sb) => sb.federation_id.clone(),
            None => return Ok(()), // Bundle not found, nothing to delete
        };

        let bundle_id_bytes = bundle_id.as_bytes().to_vec();
        let db_clone = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let cf_bundles = db_clone.cf_handle(CF_BUNDLES)
                .ok_or_else(|| StorageError::Backend("Bundles CF not found".to_string()))?;
            db_clone.delete_cf(cf_bundles, &bundle_id_bytes)
                .map_err(|e| StorageError::Backend(format!("DB delete_cf error: {}", e)))
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))??;
        
        if let Some(fed_id) = federation_id {
            Self::update_federation_index_internal(Arc::clone(&self.db), fed_id, bundle_id.to_string(), false).await?;
        }
        Ok(())
    }
    
    async fn list_bundles_by_federation(&self, federation_id: &str) -> Result<Vec<StoredTrustBundle>, StorageError> {
        let fed_id_owned = federation_id.to_string();
        let db_clone = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let cf_federation = db_clone.cf_handle(CF_FEDERATION_INDEX)
                .ok_or_else(|| StorageError::Backend("Federation index CF not found".to_string()))?;
            let cf_bundles = db_clone.cf_handle(CF_BUNDLES)
                .ok_or_else(|| StorageError::Backend("Bundles CF not found".to_string()))?;
            
            let federation_key = Self::federation_key(&fed_id_owned);
            let bundle_ids_bytes = db_clone.get_cf(cf_federation, &federation_key)
                .map_err(|e| StorageError::Backend(format!("DB get_cf error for index: {}", e)))?
                .ok_or_else(|| StorageError::InvalidFederation(fed_id_owned.clone()))?; // Or return Ok(Vec::new()) if federation not existing is not an error
                
            let bundle_ids = Self::deserialize_bundle_ids(&bundle_ids_bytes)?;
            
            let mut result = Vec::new();
            for id in bundle_ids {
                if let Some(bytes) = db_clone.get_cf(cf_bundles, id.as_bytes())
                    .map_err(|e| StorageError::Backend(format!("DB get_cf error for bundle {}: {}", id, e)))? {
                    match Self::deserialize_stored_bundle(&bytes) {
                        Ok(bundle) => result.push(bundle),
                        Err(e) => return Err(e), // Propagate deserialization error
                    }
                }
                // If bundle for an ID in index is not found, it's a data consistency issue. Could log/error.
            }
            Ok(result)
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))?
    }

    async fn get_latest_bundle_id_by_federation(&self, federation_id: &str) -> Result<Option<String>, StorageError> {
        let fed_id_owned = federation_id.to_string();
        let db_clone = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let cf_federation = db_clone.cf_handle(CF_FEDERATION_INDEX)
                .ok_or_else(|| StorageError::Backend("Federation index CF not found".to_string()))?;
            
            let federation_key = Self::federation_key(&fed_id_owned);
            match db_clone.get_cf(cf_federation, &federation_key) {
                Ok(Some(bytes)) => {
                    let bundle_ids = Self::deserialize_bundle_ids(&bytes)?;
                    Ok(bundle_ids.last().cloned())
                }
                Ok(None) => Ok(None),
                Err(e) => Err(StorageError::Backend(format!("DB get_cf error for index: {}", e))),
            }
        }).await.map_err(|e| StorageError::Backend(format!("spawn_blocking join error: {}", e)))?
    }
    
    async fn get_latest_bundle_by_federation(&self, federation_id: &str) -> Result<Option<StoredTrustBundle>, StorageError> {
        if let Some(latest_id) = self.get_latest_bundle_id_by_federation(federation_id).await? {
            self.get_bundle(&latest_id).await
        } else {
            Ok(None)
        }
    }
} 