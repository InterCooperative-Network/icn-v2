use super::{TrustBundleStore, StorageError, StoredTrustBundle, TrustBundle};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// An in-memory implementation of TrustBundleStore for testing
#[derive(Debug, Default)]
pub struct MemoryTrustBundleStore {
    /// Map of bundle_id -> StoredTrustBundle
    bundles: Arc<RwLock<HashMap<String, StoredTrustBundle>>>,
    
    /// Map of federation_id -> Vec<bundle_id> ordered by insertion (can be used to find latest by convention)
    federation_bundles: Arc<RwLock<HashMap<String, Vec<String>>>>,

    /// Map of federation_id -> latest bundle_id (explicitly tracked)
    latest_bundle_ids: Arc<RwLock<HashMap<String, String>>>,
}

impl MemoryTrustBundleStore {
    /// Create a new empty in-memory TrustBundle store
    pub fn new() -> Self {
        Self {
            bundles: Arc::new(RwLock::new(HashMap::new())),
            federation_bundles: Arc::new(RwLock::new(HashMap::new())),
            latest_bundle_ids: Arc::new(RwLock::new(HashMap::new())),
        }
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
}

#[async_trait]
impl TrustBundleStore for MemoryTrustBundleStore {
    async fn save_bundle(&self, bundle_to_save: &StoredTrustBundle) -> Result<(), StorageError> {
        let bundle_id = bundle_to_save.id.clone();
        let mut bundles_guard = self.bundles.write().await;
        let mut federation_bundles_guard = self.federation_bundles.write().await;
        let mut latest_bundle_ids_guard = self.latest_bundle_ids.write().await;
        
        bundles_guard.insert(bundle_id.clone(), bundle_to_save.clone());
        
        if let Some(fed_id) = &bundle_to_save.federation_id {
            federation_bundles_guard.entry(fed_id.clone()).or_default().push(bundle_id.clone());
            latest_bundle_ids_guard.insert(fed_id.clone(), bundle_id.clone());
        }
        Ok(())
    }
    
    async fn get_bundle(&self, id: &str) -> Result<Option<StoredTrustBundle>, StorageError> {
        let bundles_guard = self.bundles.read().await;
        Ok(bundles_guard.get(id).cloned())
    }
    
    async fn list_bundles_by_federation(&self, federation_id: &str) -> Result<Vec<StoredTrustBundle>, StorageError> {
        let bundles_guard = self.bundles.read().await;
        let federation_bundles_guard = self.federation_bundles.read().await;

        if let Some(bundle_ids_for_fed) = federation_bundles_guard.get(federation_id) {
            let result_bundles = bundle_ids_for_fed.iter()
                .filter_map(|id| bundles_guard.get(id).cloned())
                .collect();
            Ok(result_bundles)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_latest_bundle_id_by_federation(&self, federation_id: &str) -> Result<Option<String>, StorageError> {
        let latest_ids_guard = self.latest_bundle_ids.read().await;
        Ok(latest_ids_guard.get(federation_id).cloned())
    }

    async fn get_latest_bundle_by_federation(&self, federation_id: &str) -> Result<Option<StoredTrustBundle>, StorageError> {
        let latest_id_opt = {
            let latest_ids_guard = self.latest_bundle_ids.read().await;
            latest_ids_guard.get(federation_id).cloned()
        };

        if let Some(latest_id) = latest_id_opt {
            let bundles_guard = self.bundles.read().await;
            Ok(bundles_guard.get(&latest_id).cloned())
        } else {
            Ok(None)
        }
    }
    
    async fn remove_bundle(&self, bundle_id: &str) -> Result<(), StorageError> {
        let mut federation_id_to_clear: Option<String> = None;
        let mut was_latest = false;

        {
            let bundles_guard = self.bundles.read().await;
            if let Some(bundle_to_remove) = bundles_guard.get(bundle_id) {
                federation_id_to_clear = bundle_to_remove.federation_id.clone();
                if let Some(fed_id) = &federation_id_to_clear {
                    let latest_ids_guard = self.latest_bundle_ids.read().await;
                    if latest_ids_guard.get(fed_id).map_or(false, |id| id == bundle_id) {
                        was_latest = true;
                    }
                }
            } else {
                return Err(StorageError::NotFound(bundle_id.to_string()));
            }
        }

        let removed_bundle_was_present = {
            let mut bundles_mut_guard = self.bundles.write().await;
            bundles_mut_guard.remove(bundle_id).is_some()
        };

        if !removed_bundle_was_present {
            // Potentially log or handle concurrent removal if necessary
        }

        if let Some(fed_id) = federation_id_to_clear {
            {
                let mut fed_bundles_guard = self.federation_bundles.write().await;
                if let Some(list) = fed_bundles_guard.get_mut(&fed_id) {
                    list.retain(|id| id != bundle_id);
                    if list.is_empty() {
                        fed_bundles_guard.remove(&fed_id);
                    }
                }
            }
            if was_latest {
                let mut latest_ids_guard = self.latest_bundle_ids.write().await;
                latest_ids_guard.remove(&fed_id);
            }
        }
        Ok(())
    }
} 