use std::path::{Path, PathBuf};
use std::sync::Arc;
use icn_types::dag::{DagStore, PublicKeyResolver, SignedDagNode, DagError};
use icn_types::Did;
use icn_types::Cid;
use icn_identity_core::did::DidKey;
use crate::error::CliError;
use ed25519_dalek::{VerifyingKey, SigningKey, Signer};
use hex;
use serde::{Deserialize, Serialize};
use icn_types::dag::memory::MemoryDagStore;
use thiserror::Error;
use std::collections::HashMap;

// A simple in-memory resolver for keys loaded via context
#[derive(Debug, Error)]
#[error("Key not found for DID: {0}")]
#[allow(dead_code)] // Allow unused struct for now
struct SimpleResolverError(Did);

// Make this pub(crate) if get_resolver is pub(crate), or keep private if get_resolver is removed/private
struct SimpleKeyResolver {
    keys: std::sync::RwLock<HashMap<Did, VerifyingKey>>,
}

// Create a wrapper struct for DagStore that allows mutation
pub struct MutableDagStore {
    pub inner: Arc<dyn DagStore + Send + Sync>,
}

impl MutableDagStore {
    pub fn new(inner: Arc<dyn DagStore + Send + Sync>) -> Self {
        Self { inner }
    }

    pub async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError> {
        // For this implementation, we're working with the limitation that DagStore.add_node requires &mut self
        
        let mut node_copy = node;
        // If the node already has a CID, use it; otherwise calculate it
        if node_copy.cid.is_none() {
            let cid = node_copy.calculate_cid()?;
            node_copy.cid = Some(cid);
        }
        let cid = node_copy.cid.clone().unwrap();
        
        // Clone the Arc before moving into the task
        let inner_clone = Arc::clone(&self.inner);
        
        // Use a standalone task to add the node
        tokio::spawn(async move {
            // This is safe as long as we don't expose the DagStore directly to external callers
            let inner_ptr = Arc::as_ptr(&inner_clone) as *mut dyn DagStore;
            unsafe {
                let inner_mut = &mut *inner_ptr;
                inner_mut.add_node(node_copy).await
            }
        }).await.map_err(|e| DagError::JoinError(e))??;
        
        Ok(cid)
    }
    
    pub async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        self.inner.get_node(cid).await
    }
    
    pub async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.get_ordered_nodes().await
    }
    
    pub async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.get_nodes_by_payload_type(payload_type).await
    }
}

impl SimpleKeyResolver {
    fn new() -> Self {
        SimpleKeyResolver { keys: std::sync::RwLock::new(HashMap::new()) }
    }
    fn add_key(&self, did: Did, key: VerifyingKey) {
        let mut keys = self.keys.write().unwrap();
        keys.insert(did, key);
    }
}

impl PublicKeyResolver for SimpleKeyResolver {
    fn resolve(&self, did: &Did) -> Result<VerifyingKey, icn_types::dag::DagError> {
        let keys = self.keys.read().unwrap();
        keys.get(did)
            .cloned()
            .ok_or_else(|| icn_types::dag::DagError::PublicKeyResolutionError(did.clone(), "Key not found in SimpleKeyResolver".to_string()))
    }
}

// Structure to deserialize the key file JSON
#[derive(Serialize, Deserialize)]
struct KeyFileJson {
    did: String,
    #[allow(dead_code)] // Silence warning for unused field
    secret_key_hex: String, 
}

pub struct CliContext {
    _config_dir: PathBuf,
    _default_key_path: PathBuf,
    dag_store: Option<Arc<dyn DagStore + Send + Sync>>,
    _loaded_key: Option<Arc<DidKey>>,
    _key_resolver: Arc<SimpleKeyResolver>,
    pub verbose: bool,
}

impl CliContext {
    pub fn new(verbose: bool) -> Result<Self, CliError> {
        let config_dir = dirs::home_dir()
            .map(|h| h.join(".icn"))
            .ok_or_else(|| CliError::Config("Cannot determine home directory".to_string()))?;
        let default_key_path = config_dir.join("key.json");

        // Initialize with an empty resolver
        let key_resolver = Arc::new(SimpleKeyResolver::new());

        Ok(Self {
            _config_dir: config_dir,
            _default_key_path: default_key_path,
            dag_store: None,
            _loaded_key: None,
            _key_resolver: key_resolver,
            verbose,
        })
    }

    // Note: These methods use &mut self because they modify the Option fields.
    // If context needs to be shared immutably across threads while loading,
    // internal RwLocks or RefCells might be needed for dag_store/loaded_key.

    pub fn get_dag_store(&mut self, path_opt: Option<&Path>) -> Result<MutableDagStore, CliError> {
        if self.dag_store.is_none() {
            let store_path = path_opt.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                self._config_dir.join("dag_store")
            });
            
             if !store_path.exists() {
                 std::fs::create_dir_all(&store_path).map_err(|e| CliError::Io(e))?;
             }

            if self.verbose {
                println!("Initializing DAG store at: {:?}", store_path);
            }

            // Correct usage of RocksDbDagStore
            #[cfg(feature = "persistence")]
            {
                // Need to import RocksDbDagStore when used
                use icn_types::dag::rocksdb::RocksDbDagStore;
                let store = RocksDbDagStore::new(store_path).map_err(CliError::Dag)?;
                self.dag_store = Some(Arc::new(store));
            }
            #[cfg(not(feature = "persistence"))]
            {
                 eprintln!("Warning: Persistence feature not enabled, using in-memory DAG store.");
                 let store = MemoryDagStore::new(); // Use correct name
                 self.dag_store = Some(Arc::new(store));
            }
        }
        
        // Return a mutable wrapper
        Ok(MutableDagStore::new(self.dag_store.as_ref().unwrap().clone()))
    }

    #[allow(dead_code)] 
    pub fn _get_key(&mut self, key_path_opt: Option<&Path>) -> Result<Arc<DidKey>, CliError> {
        let key_path = key_path_opt.map(|p| p.to_path_buf()).unwrap_or_else(|| self._default_key_path.clone()); // Use prefixed field & clone
        
        // Check if key already loaded (avoid redundant loading/parsing)
        if let Some(key) = &self._loaded_key {
            // Optional: Check if the requested path matches the loaded key's assumed path?
            // For now, just return the cached key if any key is loaded.
            if self.verbose { println!("Returning cached key for DID: {}", key.did()); }
            return Ok(key.clone());
        }
        
        if self.verbose {
            println!("Loading key from: {:?}", key_path);
        }
        
        let key_json_str = std::fs::read_to_string(&key_path) // Borrow key_path
            .map_err(|e| CliError::Config(format!("Failed to read key file {:?}: {}", key_path, e)))?;
            
        let key_file_data: KeyFileJson = serde_json::from_str(&key_json_str)
            .map_err(|e| CliError::Config(format!("Failed to parse key file JSON {:?}: {}", key_path, e)))?;
            
        let secret_bytes = hex::decode(&key_file_data.secret_key_hex) 
             .map_err(|e| CliError::Config(format!("Invalid hex in secret key {:?}: {}", key_path, e)))?;
        
        let signing_key = SigningKey::from_bytes(secret_bytes.as_slice().try_into()
            .map_err(|_| CliError::Config(format!("Invalid secret key length in {:?}", key_path)))?) ;
            
        let did_key = DidKey::from_signing_key(signing_key); 
             
        // Check if the derived DID matches the one in the file
        if did_key.did().to_string() != key_file_data.did {
             return Err(CliError::Config(format!("DID mismatch in key file {:?}: expected {}, found {}", key_path, key_file_data.did, did_key.did())));
        }
        
        let arc_did_key = Arc::new(did_key);
        self._loaded_key = Some(arc_did_key.clone());
        
        // Add the loaded key to the simple resolver - Use the Arc
        let verifying_key = arc_did_key.verifying_key().clone();
        self._key_resolver.add_key(arc_did_key.did().clone(), verifying_key); 
            
        if self.verbose {
            println!("Loaded key for DID: {}", arc_did_key.did()); // Use Arc here too
        }
        Ok(arc_did_key)
    }

    /// Get the simple in-memory key resolver (intended for internal use)
    #[allow(dead_code)] // Silence warning for unused method
    #[allow(private_interfaces)] // Silence warning for returning private type
    fn _get_resolver(&self) -> Arc<SimpleKeyResolver> {
        self._key_resolver.clone()
    }
    
    /// Get the key resolver as a dynamic trait object
    #[allow(dead_code)] // Silence warning for unused method
    pub fn _get_resolver_dyn(&self) -> Arc<dyn PublicKeyResolver + Send + Sync> {
         self._key_resolver.clone() as Arc<dyn PublicKeyResolver + Send + Sync>
    }

    /// Load a DID key from a file path
    pub fn load_did_key(&mut self, key_path: &Path) -> Result<DidKey, CliError> {
        if self.verbose {
            println!("Loading key from: {:?}", key_path);
        }
        
        let key_json_str = std::fs::read_to_string(key_path)
            .map_err(|e| CliError::Config(format!("Failed to read key file {:?}: {}", key_path, e)))?;
            
        let key_file_data: KeyFileJson = serde_json::from_str(&key_json_str)
            .map_err(|e| CliError::Config(format!("Failed to parse key file JSON {:?}: {}", key_path, e)))?;
            
        let secret_bytes = hex::decode(&key_file_data.secret_key_hex) 
             .map_err(|e| CliError::Config(format!("Invalid hex in secret key {:?}: {}", key_path, e)))?;
        
        let signing_key = SigningKey::from_bytes(secret_bytes.as_slice().try_into()
            .map_err(|_| CliError::Config(format!("Invalid secret key length in {:?}", key_path)))?) ;
            
        let did_key = DidKey::from_signing_key(signing_key); 
             
        // Check if the derived DID matches the one in the file
        if did_key.did().to_string() != key_file_data.did {
             return Err(CliError::Config(format!("DID mismatch in key file {:?}: expected {}, found {}", key_path, key_file_data.did, did_key.did())));
        }
        
        // Add the loaded key to the simple resolver
        let verifying_key = did_key.verifying_key().clone();
        self._key_resolver.add_key(did_key.did().clone(), verifying_key); 
            
        if self.verbose {
            println!("Loaded key for DID: {}", did_key.did());
        }
        
        Ok(did_key)
    }
    
    /// Sign a DAG node with the provided key
    pub fn sign_dag_node(&self, node: icn_types::dag::DagNode, did_key: &DidKey) 
        -> Result<icn_types::dag::SignedDagNode, CliError> {
        
        // Serialize the node for signing
        let node_bytes = serde_ipld_dagcbor::to_vec(&node)
            .map_err(|e| CliError::SerializationError(format!("Failed to serialize node: {}", e)))?;
        
        // Sign the node with the provided key
        let signature = did_key.signing_key().sign(&node_bytes);
        
        // Create the signed node directly
        let signed_node = icn_types::dag::SignedDagNode {
            node,
            signature,
            cid: None, // Will be computed when added to the DAG
        };
            
        if self.verbose {
            println!("Signed DAG node with key for DID: {}", did_key.did());
        }
        
        Ok(signed_node)
    }
}

/// Helper function to get a node's CID without requiring a mutable reference
pub fn get_cid(node: &icn_types::dag::SignedDagNode) -> Result<icn_types::Cid, icn_types::dag::DagError> {
    match &node.cid {
        Some(cid) => Ok(cid.clone()),
        None => node.calculate_cid()
    }
}

// After the MutableDagStore implementation, add DagStore trait implementation
#[async_trait::async_trait]
impl icn_types::dag::DagStore for MutableDagStore {
    async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError> {
        // This method is already implemented in our MutableDagStore
        self.add_node(node).await
    }

    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        self.inner.get_node(cid).await
    }
    
    async fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, DagError> {
        self.inner.get_data(cid).await
    }
    
    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        self.inner.get_tips().await
    }
    
    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.get_ordered_nodes().await
    }

    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.get_nodes_by_author(author).await
    }

    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.get_nodes_by_payload_type(payload_type).await
    }
    
    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        self.inner.find_path(from, to).await
    }
    
    async fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        self.inner.verify_branch(tip, resolver).await
    }
} 