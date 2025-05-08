use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
use icn_common::verification::Verifiable;
use icn_services::ServiceError;

use async_trait::async_trait;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

/// Column families used in the RocksDB database
const CF_NODES: &str = "nodes";
const CF_METADATA: &str = "metadata";
const CF_SCOPE_INDEX: &str = "scope_index";
const CF_TYPE_INDEX: &str = "type_index";
const CF_LINEAGE_INDEX: &str = "lineage_index";

/// Errors that can occur in the DAG store
#[derive(Error, Debug)]
pub enum DagStoreError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Verification error: {0}")]
    Verification(String),

    #[error("Lineage error: {0}")]
    Lineage(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<rocksdb::Error> for DagStoreError {
    fn from(err: rocksdb::Error) -> Self {
        DagStoreError::Database(err.to_string())
    }
}

impl From<ServiceError> for DagStoreError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Database(msg) => DagStoreError::Database(msg),
            ServiceError::Serialization(err) => DagStoreError::Serialization(err),
            ServiceError::NodeNotFound(node_id) => DagStoreError::NodeNotFound(node_id),
            ServiceError::Verification(msg) => DagStoreError::Verification(msg),
            ServiceError::DagLineage(msg) => DagStoreError::Lineage(msg),
            ServiceError::Unauthorized(msg) => DagStoreError::Unauthorized(msg),
            _ => DagStoreError::Other(err.to_string()),
        }
    }
}

/// Metadata stored about the DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagMetadata {
    /// Version of the database format
    pub version: u32,
    /// Number of nodes in the DAG
    pub node_count: u64,
    /// Root nodes of the DAG (nodes with no parents)
    pub roots: HashSet<DAGNodeID>,
    /// Tips of the DAG (nodes with no children)
    pub tips: HashSet<DAGNodeID>,
    /// Creation timestamp of the DAG
    pub created_at: u64,
    /// Last update timestamp of the DAG
    pub updated_at: u64,
}

impl Default for DagMetadata {
    fn default() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            version: 1,
            node_count: 0,
            roots: HashSet::new(),
            tips: HashSet::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Configuration for connecting to the database
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Path to the database
    pub path: PathBuf,
    
    /// Maximum size of the write buffer (in bytes)
    pub write_buffer_size: Option<usize>,
    
    /// Maximum number of open files
    pub max_open_files: Option<i32>,
    
    /// Whether to create the database if it doesn't exist
    pub create_if_missing: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./dag_storage"),
            write_buffer_size: Some(64 * 1024 * 1024), // 64MB
            max_open_files: Some(1000),
            create_if_missing: true,
        }
    }
}

/// Scope for node authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeScope {
    /// The scope identifier
    pub scope_id: String,
    
    /// Authorized identities for this scope
    pub authorized_identities: HashSet<String>,
    
    /// Parent scopes
    pub parent_scopes: Option<HashSet<String>>,
    
    /// Additional constraints (like time-based or condition-based)
    pub constraints: Option<HashMap<String, serde_json::Value>>,
}

impl NodeScope {
    /// Create a new scope
    pub fn new(scope_id: String) -> Self {
        Self {
            scope_id,
            authorized_identities: HashSet::new(),
            parent_scopes: None,
            constraints: None,
        }
    }
    
    /// Add an authorized identity to this scope
    pub fn add_identity(&mut self, identity_id: String) -> &mut Self {
        self.authorized_identities.insert(identity_id);
        self
    }
    
    /// Set parent scopes
    pub fn with_parent_scopes(&mut self, parent_scopes: HashSet<String>) -> &mut Self {
        self.parent_scopes = Some(parent_scopes);
        self
    }
    
    /// Add a constraint
    pub fn add_constraint(&mut self, key: String, value: serde_json::Value) -> &mut Self {
        let constraints = self.constraints.get_or_insert_with(HashMap::new);
        constraints.insert(key, value);
        self
    }
    
    /// Check if an identity is authorized for this scope
    pub fn is_authorized(&self, identity_id: &str) -> bool {
        self.authorized_identities.contains(identity_id)
    }
}

/// DAG storage implementation using RocksDB
#[derive(Clone)]
pub struct RocksDbDagStore {
    /// Path to the database
    path: PathBuf,
    
    /// Database connection
    db: Arc<Mutex<Option<DB>>>,
    
    /// Known scopes
    scopes: Arc<Mutex<HashMap<String, NodeScope>>>,
}

/// DAG storage trait
#[async_trait]
pub trait DagStore: Send + Sync + 'static {
    /// Initialize the DAG store
    async fn init(&self) -> Result<(), DagStoreError>;
    
    /// Add a node to the DAG, verifying lineage
    async fn append_node(&self, node: DAGNode) -> Result<DAGNodeID, DagStoreError>;
    
    /// Get a node by its ID
    async fn get_node(&self, cid: &DAGNodeID) -> Result<Option<DAGNode>, DagStoreError>;
    
    /// Check if a node exists
    async fn node_exists(&self, cid: &DAGNodeID) -> Result<bool, DagStoreError>;
    
    /// Verify the lineage of a node against a scope
    async fn verify_lineage(&self, cid: &DAGNodeID, scope: &NodeScope) -> Result<bool, DagStoreError>;
    
    /// Get nodes by type
    async fn get_nodes_by_type(
        &self,
        node_type: DAGNodeType,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, DagStoreError>;
    
    /// Get nodes by scope
    async fn get_nodes_by_scope(
        &self,
        scope: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, DagStoreError>;
    
    /// Get child nodes of a given node
    async fn get_children(&self, cid: &DAGNodeID) -> Result<Vec<DAGNode>, DagStoreError>;
    
    /// Get parent nodes of a given node
    async fn get_parents(&self, cid: &DAGNodeID) -> Result<Vec<DAGNode>, DagStoreError>;
    
    /// Get the DAG metadata
    async fn get_metadata(&self) -> Result<DagMetadata, DagStoreError>;
    
    /// Register a scope
    async fn register_scope(&self, scope: NodeScope) -> Result<(), DagStoreError>;
    
    /// Get a scope
    async fn get_scope(&self, scope_id: &str) -> Result<Option<NodeScope>, DagStoreError>;
    
    /// Compact the database
    async fn compact(&self) -> Result<(), DagStoreError>;
    
    /// Close the database connection
    async fn close(&self) -> Result<(), DagStoreError>;
}

impl RocksDbDagStore {
    /// Create a new RocksDB DAG store
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            path: config.path,
            db: Arc::new(Mutex::new(None)),
            scopes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Create RocksDB options
    fn create_options(config: &ConnectionConfig) -> Options {
        let mut options = Options::default();
        options.create_if_missing(config.create_if_missing);
        options.create_missing_column_families(true);
        
        if let Some(write_buffer_size) = config.write_buffer_size {
            options.set_write_buffer_size(write_buffer_size);
        }
        
        if let Some(max_open_files) = config.max_open_files {
            options.set_max_open_files(max_open_files);
        }
        
        options.set_keep_log_file_num(10);
        options.set_use_fsync(false);
        options.set_max_write_buffer_number(4);
        
        options
    }
    
    /// Create column family descriptors
    fn create_cf_descriptors() -> Vec<ColumnFamilyDescriptor> {
        let options = Options::default();
        vec![
            ColumnFamilyDescriptor::new(CF_NODES, options.clone()),
            ColumnFamilyDescriptor::new(CF_METADATA, options.clone()),
            ColumnFamilyDescriptor::new(CF_SCOPE_INDEX, options.clone()),
            ColumnFamilyDescriptor::new(CF_TYPE_INDEX, options.clone()),
            ColumnFamilyDescriptor::new(CF_LINEAGE_INDEX, options.clone()),
        ]
    }
    
    /// Get a column family handle
    fn get_cf<'a>(&self, db: &'a DB, name: &str) -> Result<&'a ColumnFamily, DagStoreError> {
        db.cf_handle(name)
            .ok_or_else(|| DagStoreError::Database(format!("Column family {} not found", name)))
    }
    
    /// Initialize metadata if it doesn't exist
    async fn init_metadata(&self) -> Result<(), DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        
        if db.get_cf(cf_metadata, "metadata")?.is_none() {
            let metadata = DagMetadata::default();
            let metadata_bytes = serde_json::to_vec(&metadata)?;
            db.put_cf(cf_metadata, "metadata", metadata_bytes)?;
            
            info!("Initialized new DAG metadata");
        }
        
        Ok(())
    }
    
    /// Build the lineage index for a node
    fn build_lineage_index(&self, db: &DB, node: &DAGNode) -> Result<(), DagStoreError> {
        let node_id = node.id().map_err(|e| DagStoreError::Other(e.to_string()))?;
        let cf_lineage = self.get_cf(db, CF_LINEAGE_INDEX)?;
        
        // Store parent-child relationships
        for parent_id in &node.header.parents {
            let key = format!("parent:{}:child:{}", parent_id.as_str(), node_id.as_str());
            db.put_cf(cf_lineage, key, &[])?;
            
            let key = format!("child:{}:parent:{}", node_id.as_str(), parent_id.as_str());
            db.put_cf(cf_lineage, key, &[])?;
        }
        
        // Store scope-node relationships
        let scope_key = format!("scope:{}:node:{}", node.header.scope, node_id.as_str());
        db.put_cf(cf_lineage, scope_key, &[])?;
        
        Ok(())
    }
    
    /// Check if a scope exists in the database
    async fn scope_exists(&self, scope_id: &str) -> Result<bool, DagStoreError> {
        let scopes = self.scopes.lock().unwrap();
        Ok(scopes.contains_key(scope_id))
    }
    
    /// Validate that a node's lineage is consistent with a scope
    async fn validate_node_lineage_for_scope(
        &self,
        node: &DAGNode,
        scope: &NodeScope
    ) -> Result<bool, DagStoreError> {
        // First check if the node's creator is authorized for this scope
        if !scope.is_authorized(&node.header.creator.id()) {
            return Ok(false);
        }
        
        // For nodes with parents, validate that all parents are valid within this scope
        if !node.header.parents.is_empty() {
            for parent_id in &node.header.parents {
                if let Some(parent_node) = self.get_node(parent_id).await? {
                    // Check if the parent is in the same scope
                    if parent_node.header.scope != node.header.scope {
                        // If parent is in a different scope, check if that scope is a parent of this scope
                        if let Some(parent_scopes) = &scope.parent_scopes {
                            if !parent_scopes.contains(&parent_node.header.scope) {
                                debug!("Parent node {} is in scope {}, which is not a parent of scope {}",
                                    parent_id.as_str(),
                                    parent_node.header.scope,
                                    scope.scope_id
                                );
                                return Ok(false);
                            }
                        } else {
                            // No parent scopes defined, so cross-scope references are not allowed
                            debug!("Cross-scope reference not allowed: {} -> {}",
                                parent_node.header.scope,
                                scope.scope_id
                            );
                            return Ok(false);
                        }
                    }
                    
                    // Recursively validate the parent's lineage
                    if let Some(parent_scope) = self.get_scope(&parent_node.header.scope).await? {
                        if !self.validate_node_lineage_for_scope(&parent_node, &parent_scope).await? {
                            return Ok(false);
                        }
                    } else {
                        // Parent scope not found, lineage is invalid
                        debug!("Parent scope not found: {}", parent_node.header.scope);
                        return Ok(false);
                    }
                } else {
                    // Parent node not found, lineage is broken
                    debug!("Parent node not found: {}", parent_id.as_str());
                    return Ok(false);
                }
            }
        }
        
        // All checks passed
        Ok(true)
    }
    
    /// Get all nodes in a lineage chain
    async fn get_lineage_chain(&self, node_id: &DAGNodeID) -> Result<Vec<DAGNode>, DagStoreError> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        
        // Start with the given node
        if let Some(node) = self.get_node(node_id).await? {
            queue.push_back(node);
            visited.insert(node_id.clone());
        } else {
            return Ok(result);
        }
        
        // Breadth-first traversal of the lineage
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            
            // Add parents to the queue
            for parent_id in &node.header.parents {
                if !visited.contains(parent_id) {
                    if let Some(parent_node) = self.get_node(parent_id).await? {
                        queue.push_back(parent_node);
                        visited.insert(parent_id.clone());
                    }
                }
            }
        }
        
        Ok(result)
    }
}

#[async_trait]
impl DagStore for RocksDbDagStore {
    async fn init(&self) -> Result<(), DagStoreError> {
        info!("Initializing RocksDB DAG storage at {:?}", self.path);
        
        // Create a configuration for this store
        let config = ConnectionConfig {
            path: self.path.clone(),
            write_buffer_size: Some(64 * 1024 * 1024), // 64MB
            max_open_files: Some(1000),
            create_if_missing: true,
        };
        
        // Create column families if they don't exist
        let cf_names = [CF_NODES, CF_METADATA, CF_SCOPE_INDEX, CF_TYPE_INDEX, CF_LINEAGE_INDEX];
        
        let mut db_guard = self.db.lock().unwrap();
        
        let options = Self::create_options(&config);
        let cf_descriptors = Self::create_cf_descriptors();
        
        match DB::open_cf_descriptors(&options, &self.path, cf_descriptors) {
            Ok(db) => {
                *db_guard = Some(db);
                drop(db_guard); // Release lock before async call
                self.init_metadata().await?;
                Ok(())
            }
            Err(e) => Err(DagStoreError::Database(format!("Failed to open database: {}", e))),
        }
    }
    
    async fn append_node(&self, node: DAGNode) -> Result<DAGNodeID, DagStoreError> {
        // Verify the node's signature
        if !node.verify().map_err(|e| DagStoreError::Verification(e.to_string()))? {
            return Err(DagStoreError::Verification("Node signature verification failed".into()));
        }
        
        // Calculate node ID
        let node_id = node.id().map_err(|e| DagStoreError::Other(e.to_string()))?;
        
        // Check if the node already exists
        if self.node_exists(&node_id).await? {
            return Ok(node_id);
        }
        
        // Check if the scope exists
        let scope_exists = self.scope_exists(&node.header.scope).await?;
        if !scope_exists {
            // Create a default scope for this node
            let mut scope = NodeScope::new(node.header.scope.clone());
            scope.add_identity(node.header.creator.id().to_string());
            self.register_scope(scope).await?;
        }
        
        // Verify that all parent nodes exist
        for parent_id in &node.header.parents {
            if !self.node_exists(parent_id).await? {
                return Err(DagStoreError::Lineage(
                    format!("Parent node {} does not exist", parent_id.as_str())
                ));
            }
        }
        
        // Store the node
        let node_bytes = serde_json::to_vec(&node)?;
        
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_scope_index = self.get_cf(db, CF_SCOPE_INDEX)?;
        let cf_type_index = self.get_cf(db, CF_TYPE_INDEX)?;
        
        // Store the node
        db.put_cf(cf_nodes, node_id.as_str(), &node_bytes)?;
        
        // Update scope index
        let scope_key = format!("{}:{}", node.header.scope, node_id.as_str());
        db.put_cf(cf_scope_index, scope_key, &[])?;
        
        // Update type index
        let type_key = format!("{}:{}", 
            serde_json::to_string(&node.header.node_type)?, 
            node_id.as_str()
        );
        db.put_cf(cf_type_index, type_key, &[])?;
        
        // Build lineage index
        self.build_lineage_index(db, &node)?;
        
        // Update metadata
        let mut metadata = self.get_metadata().await?;
        metadata.node_count += 1;
        metadata.updated_at = node.header.timestamp;
        
        // Update roots (if this is a root node)
        if node.header.parents.is_empty() {
            metadata.roots.insert(node_id.clone());
        }
        
        // Update tips
        // Remove any parents from tips as they now have children
        for parent_id in &node.header.parents {
            metadata.tips.remove(parent_id);
        }
        
        // Add this node to tips
        metadata.tips.insert(node_id.clone());
        
        // Save updated metadata
        let metadata_bytes = serde_json::to_vec(&metadata)?;
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        db.put_cf(cf_metadata, "metadata", metadata_bytes)?;
        
        debug!("Stored node {} of type {:?} in scope {}", 
            node_id.as_str(), 
            node.header.node_type, 
            node.header.scope
        );
        
        Ok(node_id)
    }
    
    async fn get_node(&self, cid: &DAGNodeID) -> Result<Option<DAGNode>, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        
        match db.get_cf(cf_nodes, cid.as_str())? {
            Some(node_bytes) => {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }
    
    async fn node_exists(&self, cid: &DAGNodeID) -> Result<bool, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        
        match db.get_cf(cf_nodes, cid.as_str())? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
    
    async fn verify_lineage(&self, cid: &DAGNodeID, scope: &NodeScope) -> Result<bool, DagStoreError> {
        // Get the node
        let node = match self.get_node(cid).await? {
            Some(node) => node,
            None => return Err(DagStoreError::NodeNotFound(cid.as_str().to_string())),
        };
        
        // If the node is in a different scope than the one we're checking against,
        // check if the node's scope is a valid parent of the given scope
        if node.header.scope != scope.scope_id {
            if let Some(parent_scopes) = &scope.parent_scopes {
                if !parent_scopes.contains(&node.header.scope) {
                    debug!("Node {} is in scope {}, which is not a parent of scope {}",
                        cid.as_str(),
                        node.header.scope,
                        scope.scope_id
                    );
                    return Ok(false);
                }
            } else {
                // No parent scopes defined, so cross-scope references are not allowed
                debug!("Cross-scope reference not allowed: {} -> {}",
                    node.header.scope,
                    scope.scope_id
                );
                return Ok(false);
            }
        }
        
        // Validate the node's lineage
        self.validate_node_lineage_for_scope(&node, scope).await
    }
    
    async fn get_nodes_by_type(
        &self,
        node_type: DAGNodeType,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_type_index = self.get_cf(db, CF_TYPE_INDEX)?;
        
        let type_prefix = format!("{}:", serde_json::to_string(&node_type)?);
        
        let mut iter = db.iterator_cf(cf_type_index, rocksdb::IteratorMode::From(
            type_prefix.as_bytes(), 
            rocksdb::Direction::Forward
        ));
        
        let mut result = Vec::new();
        let limit = limit.unwrap_or(usize::MAX);
        let offset = offset.unwrap_or(0);
        let mut count = 0;
        
        while let Some(Ok((key, _))) = iter.next() {
            let key_str = std::str::from_utf8(&key)
                .map_err(|e| DagStoreError::Other(format!("Invalid UTF-8 key: {}", e)))?;
            
            // Check if we've moved past this type
            if !key_str.starts_with(&type_prefix) {
                break;
            }
            
            // Skip entries until we reach the offset
            if count < offset {
                count += 1;
                continue;
            }
            
            // Extract node ID from key
            let node_id_str = key_str.split(':').nth(1).ok_or_else(|| 
                DagStoreError::Database(format!("Invalid type index key: {}", key_str))
            )?;
            
            // Get the node
            if let Some(node_bytes) = db.get_cf(cf_nodes, node_id_str)? {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                result.push(node);
                
                // Check if we've reached the limit
                if result.len() >= limit {
                    break;
                }
            }
            
            count += 1;
        }
        
        Ok(result)
    }
    
    async fn get_nodes_by_scope(
        &self,
        scope: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_scope_index = self.get_cf(db, CF_SCOPE_INDEX)?;
        
        let scope_prefix = format!("{}:", scope);
        
        let mut iter = db.iterator_cf(cf_scope_index, rocksdb::IteratorMode::From(
            scope_prefix.as_bytes(), 
            rocksdb::Direction::Forward
        ));
        
        let mut result = Vec::new();
        let limit = limit.unwrap_or(usize::MAX);
        let offset = offset.unwrap_or(0);
        let mut count = 0;
        
        while let Some(Ok((key, _))) = iter.next() {
            let key_str = std::str::from_utf8(&key)
                .map_err(|e| DagStoreError::Other(format!("Invalid UTF-8 key: {}", e)))?;
            
            // Check if we've moved past this scope
            if !key_str.starts_with(&scope_prefix) {
                break;
            }
            
            // Skip entries until we reach the offset
            if count < offset {
                count += 1;
                continue;
            }
            
            // Extract node ID from key
            let node_id_str = key_str.split(':').nth(1).ok_or_else(|| 
                DagStoreError::Database(format!("Invalid scope index key: {}", key_str))
            )?;
            
            // Get the node
            if let Some(node_bytes) = db.get_cf(cf_nodes, node_id_str)? {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                result.push(node);
                
                // Check if we've reached the limit
                if result.len() >= limit {
                    break;
                }
            }
            
            count += 1;
        }
        
        Ok(result)
    }
    
    async fn get_children(&self, cid: &DAGNodeID) -> Result<Vec<DAGNode>, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_lineage = self.get_cf(db, CF_LINEAGE_INDEX)?;
        
        let child_prefix = format!("parent:{}:child:", cid.as_str());
        
        let mut iter = db.iterator_cf(cf_lineage, rocksdb::IteratorMode::From(
            child_prefix.as_bytes(), 
            rocksdb::Direction::Forward
        ));
        
        let mut result = Vec::new();
        
        while let Some(Ok((key, _))) = iter.next() {
            let key_str = std::str::from_utf8(&key)
                .map_err(|e| DagStoreError::Other(format!("Invalid UTF-8 key: {}", e)))?;
            
            // Check if we've moved past children of this node
            if !key_str.starts_with(&child_prefix) {
                break;
            }
            
            // Extract child ID from key
            let child_id_str = key_str.split(':').nth(3).ok_or_else(|| 
                DagStoreError::Database(format!("Invalid lineage index key: {}", key_str))
            )?;
            
            // Get the child node
            if let Some(node_bytes) = db.get_cf(cf_nodes, child_id_str)? {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                result.push(node);
            }
        }
        
        Ok(result)
    }
    
    async fn get_parents(&self, cid: &DAGNodeID) -> Result<Vec<DAGNode>, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_lineage = self.get_cf(db, CF_LINEAGE_INDEX)?;
        
        let parent_prefix = format!("child:{}:parent:", cid.as_str());
        
        let mut iter = db.iterator_cf(cf_lineage, rocksdb::IteratorMode::From(
            parent_prefix.as_bytes(), 
            rocksdb::Direction::Forward
        ));
        
        let mut result = Vec::new();
        
        while let Some(Ok((key, _))) = iter.next() {
            let key_str = std::str::from_utf8(&key)
                .map_err(|e| DagStoreError::Other(format!("Invalid UTF-8 key: {}", e)))?;
            
            // Check if we've moved past parents of this node
            if !key_str.starts_with(&parent_prefix) {
                break;
            }
            
            // Extract parent ID from key
            let parent_id_str = key_str.split(':').nth(3).ok_or_else(|| 
                DagStoreError::Database(format!("Invalid lineage index key: {}", key_str))
            )?;
            
            // Get the parent node
            if let Some(node_bytes) = db.get_cf(cf_nodes, parent_id_str)? {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                result.push(node);
            }
        }
        
        Ok(result)
    }
    
    async fn get_metadata(&self) -> Result<DagMetadata, DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        
        match db.get_cf(cf_metadata, "metadata")? {
            Some(metadata_bytes) => {
                let metadata: DagMetadata = serde_json::from_slice(&metadata_bytes)?;
                Ok(metadata)
            }
            None => {
                // This should not happen if init_metadata was called
                Ok(DagMetadata::default())
            }
        }
    }
    
    async fn register_scope(&self, scope: NodeScope) -> Result<(), DagStoreError> {
        let mut scopes = self.scopes.lock().unwrap();
        scopes.insert(scope.scope_id.clone(), scope);
        Ok(())
    }
    
    async fn get_scope(&self, scope_id: &str) -> Result<Option<NodeScope>, DagStoreError> {
        let scopes = self.scopes.lock().unwrap();
        Ok(scopes.get(scope_id).cloned())
    }
    
    async fn compact(&self) -> Result<(), DagStoreError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| 
            DagStoreError::Database("Database not initialized".into())
        )?;
        
        // Compact all column families
        for cf_name in &[CF_NODES, CF_METADATA, CF_SCOPE_INDEX, CF_TYPE_INDEX, CF_LINEAGE_INDEX] {
            if let Some(cf) = db.cf_handle(cf_name) {
                db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
            }
        }
        
        info!("Database compaction complete");
        
        Ok(())
    }
    
    async fn close(&self) -> Result<(), DagStoreError> {
        let mut db_guard = self.db.lock().unwrap();
        *db_guard = None;
        
        info!("Database closed");
        
        Ok(())
    }
} 