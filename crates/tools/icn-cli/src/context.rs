use std::path::PathBuf;
use std::sync::Arc;
use icn_types::dag::{DagStore, memory::MemoryDagStore, rocksdb::RocksDbDagStore, DagError, PublicKeyResolver};
use icn_types::identity::Did;
use icn_identity_core::did::DidKey;
use crate::error::CliError;
use ed25519_dalek::VerifyingKey;

// Simple resolver using only the loaded key (if applicable)
#[derive(Clone)] // Clone needed for Arc
struct SimpleKeyResolver {
    key: Option<VerifyingKey>, 
}

impl PublicKeyResolver for SimpleKeyResolver {
    fn resolve(&self, did: &Did) -> Result<VerifyingKey, DagError> {
        // TODO: Implement actual DID resolution logic.
        // This placeholder only returns the key if one is loaded.
        // It doesn't validate if the DID actually matches the key.
        // A real implementation would likely involve did:key parsing 
        // (e.g., using did_key crate) or registry lookup.
        if let Some(ref key) = self.key {
            // Placeholder check: Assume the loaded key is the one being asked for.
            // This is NOT safe in general.
            println!("Warning: Using placeholder DID resolution. Resolving {} to loaded key.", did);
             Ok(key.clone())
        } else {
             Err(DagError::PublicKeyResolutionError(did.clone(), "No key loaded in context for resolution".to_string()))
        }
    }
}


pub struct CliContext {
    config_dir: PathBuf,
    default_key_path: PathBuf,
    dag_store: Option<Arc<dyn DagStore + Send + Sync>>,
    loaded_key: Option<DidKey>,
    key_resolver: Arc<SimpleKeyResolver>, // Use concrete type inside Arc
    verbose: bool,
}

impl CliContext {
    pub fn new(verbose: bool) -> Result<Self, CliError> {
        let config_dir = dirs::home_dir()
            .map(|h| h.join(".icn"))
            .ok_or_else(|| CliError::Config("Cannot determine home directory".to_string()))?;
        let default_key_path = config_dir.join("key.json");

        // Initialize with an empty resolver
        let key_resolver = Arc::new(SimpleKeyResolver { key: None });

        Ok(Self {
            config_dir,
            default_key_path,
            dag_store: None,
            loaded_key: None,
            key_resolver,
            verbose,
        })
    }

    // Note: These methods use &mut self because they modify the Option fields.
    // If context needs to be shared immutably across threads while loading,
    // internal RwLocks or RefCells might be needed for dag_store/loaded_key.

    pub fn get_dag_store(&mut self, dag_path: Option<&PathBuf>) -> Result<Arc<dyn DagStore + Send + Sync>, CliError> {
        if self.dag_store.is_none() {
            let path = dag_path.cloned().unwrap_or_else(|| self.config_dir.join("dag"));
            if self.verbose { println!("Loading DAG store from: {}", path.display()); }
            
            if let Some(parent) = path.parent() {
                 std::fs::create_dir_all(parent)?;
            }
            
            // TODO: Add option for MemoryDagStore based on flag/path? ("memory" or similar)
            let store = RocksDbDagStore::open(&path)
                .map_err(|e| CliError::Dag(e))?;
            self.dag_store = Some(Arc::new(store));
        }
        Ok(self.dag_store.as_ref().unwrap().clone())
    }

    pub fn get_key(&mut self, key_path_opt: Option<&PathBuf>) -> Result<DidKey, CliError> {
         if self.loaded_key.is_none() {
            let key_path = key_path_opt.unwrap_or(&self.default_key_path);
            if self.verbose { println!("Loading key from: {}", key_path.display()); }
             
            let key_json = std::fs::read_to_string(key_path)
                .map_err(|e| CliError::Io(e))?; // Map IO error
                
            let key: DidKey = serde_json::from_str(&key_json)
                .map_err(|e| CliError::Json(e))?; // Map JSON error
             
            self.loaded_key = Some(key.clone());

            // Update the resolver now that the key is loaded
            // Note: This assumes the key uses Ed25519 - might need checks/generalization
             let public_bytes = key.public_key_bytes();
             let verifying_key = VerifyingKey::from_bytes(&public_bytes)
                .map_err(|e| CliError::DidKey(format!("Failed to create verifying key from loaded key: {}", e)))?;
             self.key_resolver = Arc::new(SimpleKeyResolver { key: Some(verifying_key) });

             Ok(key)
         } else {
            Ok(self.loaded_key.as_ref().unwrap().clone())
         }
    }

    // Returns the specific resolver implementation wrapped in Arc
    pub fn get_resolver(&self) -> Arc<SimpleKeyResolver> {
        self.key_resolver.clone()
    }

    // Helper to get a dyn trait object if needed by some functions
    pub fn get_resolver_dyn(&self) -> Arc<dyn PublicKeyResolver + Send + Sync> {
        self.key_resolver.clone() as Arc<dyn PublicKeyResolver + Send + Sync>
    }
} 