use std::path::PathBuf;
use std::sync::Arc;
use icn_types::dag::{DagStore, rocksdb::RocksDbDagStore, DagError, PublicKeyResolver};
use icn_types::Did;
use icn_identity_core::did::DidKey;
use crate::error::CliError;
use ed25519_dalek::{VerifyingKey, SigningKey};
use hex;
use serde::Deserialize;

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

// Define a struct for deserializing the key file
#[derive(Deserialize)]
struct KeyFileJson {
    secret_key_hex: String,
}

pub struct CliContext {
    config_dir: PathBuf,
    default_key_path: PathBuf,
    dag_store: Option<Arc<dyn DagStore + Send + Sync>>,
    loaded_key: Option<Arc<DidKey>>,
    key_resolver: Arc<SimpleKeyResolver>,
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

    pub fn get_key(&mut self, key_path_opt: Option<&PathBuf>) -> Result<Arc<DidKey>, CliError> {
         if self.loaded_key.is_none() {
            let key_path = key_path_opt.unwrap_or(&self.default_key_path);
            if self.verbose { println!("Loading key from: {}", key_path.display()); }
             
            // Read and parse JSON
            let key_json_str = std::fs::read_to_string(key_path)
                .map_err(|e| CliError::Io(e))?; 
            let key_file: KeyFileJson = serde_json::from_str(&key_json_str)
                .map_err(|e| CliError::Json(e))?; 
            
            // Decode hex secret key
            let secret_bytes = hex::decode(&key_file.secret_key_hex)
                .map_err(|e| CliError::Config(format!("Invalid hex in key file: {}", e)))?;
            let secret_array: &[u8; 32] = secret_bytes.as_slice().try_into()
                 .map_err(|_| CliError::Config("Invalid secret key length, expected 32 bytes".to_string()))?;
            
            // Construct keys
            let signing_key = SigningKey::from_bytes(secret_array);
            let verifying_key = signing_key.verifying_key(); // Get verifying key before moving signing_key
            let did_key = DidKey::from_signing_key(signing_key); // Use new constructor
             
            let arc_did_key = Arc::new(did_key);
            self.loaded_key = Some(arc_did_key.clone());

            // Update the resolver 
             self.key_resolver = Arc::new(SimpleKeyResolver { key: Some(verifying_key) });

             Ok(arc_did_key)
         } else {
            // Return the cached Arc
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