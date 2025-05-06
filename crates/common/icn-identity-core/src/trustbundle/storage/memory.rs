use super::{TrustBundleStore, StorageError, TrustBundle};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// An in-memory implementation of TrustBundleStore for testing
#[derive(Debug, Default)]
pub struct MemoryTrustBundleStore {
    /// Map of bundle_id -> TrustBundle
    bundles: Arc<RwLock<HashMap<String, TrustBundle>>>,
    
    /// Map of federation_id -> Vec<bundle_id> ordered by timestamp (newest last)
    federation_bundles: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl MemoryTrustBundleStore {
    /// Create a new empty in-memory TrustBundle store
    pub fn new() -> Self {
        Self {
            bundles: Arc::new(RwLock::new(HashMap::new())),
            federation_bundles: Arc::new(RwLock::new(HashMap::new())),
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
    async fn store(&self, bundle: TrustBundle) -> Result<String, StorageError> {
        let bundle_id = Self::get_bundle_id(&bundle);
        
        // Acquire write locks
        let mut bundles = self.bundles.write().await;
        let mut federation_bundles = self.federation_bundles.write().await;
        
        // Store the bundle
        bundles.insert(bundle_id.clone(), bundle.clone());
        
        // Update the federation index
        federation_bundles
            .entry(bundle.federation_id.clone())
            .or_insert_with(Vec::new)
            .push(bundle_id.clone());
        
        Ok(bundle_id)
    }
    
    async fn get(&self, bundle_id: &str) -> Result<TrustBundle, StorageError> {
        // Acquire read lock
        let bundles = self.bundles.read().await;
        
        bundles
            .get(bundle_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(bundle_id.to_string()))
    }
    
    async fn delete(&self, bundle_id: &str) -> Result<(), StorageError> {
        // Acquire write locks
        let mut bundles = self.bundles.write().await;
        let mut federation_bundles = self.federation_bundles.write().await;
        
        // Get the bundle to find its federation
        let bundle = bundles
            .get(bundle_id)
            .ok_or_else(|| StorageError::NotFound(bundle_id.to_string()))?;
        
        let federation_id = &bundle.federation_id;
        
        // Remove from bundles map
        bundles.remove(bundle_id);
        
        // Remove from federation index
        if let Some(federation_bundle_list) = federation_bundles.get_mut(federation_id) {
            federation_bundle_list.retain(|id| id != bundle_id);
        }
        
        Ok(())
    }
    
    async fn list_by_federation(&self, federation_id: &str) -> Result<Vec<TrustBundle>, StorageError> {
        // Acquire read locks
        let bundles = self.bundles.read().await;
        let federation_bundles = self.federation_bundles.read().await;
        
        // Get the list of bundle IDs for this federation
        let bundle_ids = federation_bundles
            .get(federation_id)
            .ok_or_else(|| StorageError::InvalidFederation(federation_id.to_string()))?;
        
        // Collect the bundles
        let mut result = Vec::new();
        for id in bundle_ids {
            if let Some(bundle) = bundles.get(id) {
                result.push(bundle.clone());
            }
        }
        
        Ok(result)
    }
    
    async fn get_latest(&self, federation_id: &str) -> Result<TrustBundle, StorageError> {
        // Acquire read locks
        let bundles = self.bundles.read().await;
        let federation_bundles = self.federation_bundles.read().await;
        
        // Get the list of bundle IDs for this federation
        let bundle_ids = federation_bundles
            .get(federation_id)
            .ok_or_else(|| StorageError::InvalidFederation(federation_id.to_string()))?;
        
        if bundle_ids.is_empty() {
            return Err(StorageError::NotFound(federation_id.to_string()));
        }
        
        // Get the last (newest) bundle ID
        let latest_id = bundle_ids.last().unwrap();
        
        // Get the bundle
        bundles
            .get(latest_id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(latest_id.to_string()))
    }
} 