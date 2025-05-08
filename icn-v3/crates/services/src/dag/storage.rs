use crate::error::ServiceError;
use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
use icn_common::identity::ScopedIdentity;
use icn_common::verification::Verifiable;

use async_trait::async_trait;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, trace, warn};

/// Column families used in the RocksDB database
const CF_NODES: &str = "nodes";
const CF_METADATA: &str = "metadata";
const CF_SCOPE_INDEX: &str = "scope_index";
const CF_TYPE_INDEX: &str = "type_index";

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

/// Storage backend trait for DAG storage implementations
#[async_trait]
pub trait DagStorageBackend: Send + Sync + 'static {
    /// Initialize the storage backend
    async fn init(&self) -> Result<(), ServiceError>;
    
    /// Store a node in the database
    async fn store_node(&self, node: &DAGNode) -> Result<DAGNodeID, ServiceError>;
    
    /// Retrieve a node by its ID
    async fn get_node(&self, id: &DAGNodeID) -> Result<DAGNode, ServiceError>;
    
    /// Check if a node exists
    async fn node_exists(&self, id: &DAGNodeID) -> Result<bool, ServiceError>;
    
    /// Get nodes by type
    async fn get_nodes_by_type(
        &self,
        node_type: DAGNodeType,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, ServiceError>;
    
    /// Get nodes by scope
    async fn get_nodes_by_scope(
        &self,
        scope: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, ServiceError>;
    
    /// Get child nodes of a given node
    async fn get_children(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError>;
    
    /// Get parent nodes of a given node
    async fn get_parents(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError>;
    
    /// Get the DAG metadata
    async fn get_metadata(&self) -> Result<DagMetadata, ServiceError>;
    
    /// Update the DAG metadata
    async fn update_metadata(&self, metadata: &DagMetadata) -> Result<(), ServiceError>;
    
    /// Compact the database to optimize storage
    async fn compact(&self) -> Result<(), ServiceError>;
    
    /// Close the database properly
    async fn close(&self) -> Result<(), ServiceError>;
}

/// The main DAG storage interface
pub struct DagStorage {
    backend: Arc<dyn DagStorageBackend>,
}

impl DagStorage {
    /// Create a new DAG storage with the given backend
    pub fn new(backend: Arc<dyn DagStorageBackend>) -> Self {
        Self { backend }
    }
    
    /// Initialize the storage
    pub async fn init(&self) -> Result<(), ServiceError> {
        self.backend.init().await
    }
    
    /// Add a node to the DAG, verifying its integrity and lineage
    pub async fn add_node(&self, node: &DAGNode) -> Result<DAGNodeID, ServiceError> {
        // First verify the node's signature
        if !node.verify()? {
            return Err(ServiceError::Verification("Node signature verification failed".into()));
        }
        
        // Calculate node ID
        let node_id = node.id()?;
        
        // Check if node already exists
        if self.backend.node_exists(&node_id).await? {
            return Ok(node_id); // Node already exists, just return its ID
        }
        
        // Verify that all parent nodes exist
        for parent_id in &node.header.parents {
            if !self.backend.node_exists(parent_id).await? {
                return Err(ServiceError::DagLineage(
                    format!("Parent node {} does not exist", parent_id.as_str())
                ));
            }
        }
        
        // If this is a cooperative or federation operation, verify the scope authority
        // (This would normally check membership attestations, will be implemented in detail later)
        
        // Store the node
        let node_id = self.backend.store_node(node).await?;
        
        // Update metadata
        let mut metadata = self.backend.get_metadata().await?;
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
        self.backend.update_metadata(&metadata).await?;
        
        Ok(node_id)
    }
    
    /// Get a node by its ID
    pub async fn get_node(&self, id: &DAGNodeID) -> Result<DAGNode, ServiceError> {
        self.backend.get_node(id).await
    }
    
    /// Check if a node exists
    pub async fn node_exists(&self, id: &DAGNodeID) -> Result<bool, ServiceError> {
        self.backend.node_exists(id).await
    }
    
    /// Get nodes by type
    pub async fn get_nodes_by_type(
        &self,
        node_type: DAGNodeType,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, ServiceError> {
        self.backend.get_nodes_by_type(node_type, limit, offset).await
    }
    
    /// Get nodes by scope
    pub async fn get_nodes_by_scope(
        &self,
        scope: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, ServiceError> {
        self.backend.get_nodes_by_scope(scope, limit, offset).await
    }
    
    /// Get child nodes of a given node
    pub async fn get_children(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError> {
        self.backend.get_children(id).await
    }
    
    /// Get parent nodes of a given node
    pub async fn get_parents(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError> {
        self.backend.get_parents(id).await
    }
    
    /// Get all root nodes (nodes with no parents)
    pub async fn get_roots(&self) -> Result<Vec<DAGNode>, ServiceError> {
        let metadata = self.backend.get_metadata().await?;
        let mut roots = Vec::with_capacity(metadata.roots.len());
        
        for root_id in &metadata.roots {
            match self.backend.get_node(root_id).await {
                Ok(node) => roots.push(node),
                Err(e) => {
                    error!("Failed to get root node {}: {:?}", root_id.as_str(), e);
                    // Continue with other roots
                }
            }
        }
        
        Ok(roots)
    }
    
    /// Get all tip nodes (nodes with no children)
    pub async fn get_tips(&self) -> Result<Vec<DAGNode>, ServiceError> {
        let metadata = self.backend.get_metadata().await?;
        let mut tips = Vec::with_capacity(metadata.tips.len());
        
        for tip_id in &metadata.tips {
            match self.backend.get_node(tip_id).await {
                Ok(node) => tips.push(node),
                Err(e) => {
                    error!("Failed to get tip node {}: {:?}", tip_id.as_str(), e);
                    // Continue with other tips
                }
            }
        }
        
        Ok(tips)
    }
    
    /// Get metadata about the DAG
    pub async fn get_metadata(&self) -> Result<DagMetadata, ServiceError> {
        self.backend.get_metadata().await
    }
    
    /// Compact the database
    pub async fn compact(&self) -> Result<(), ServiceError> {
        self.backend.compact().await
    }
    
    /// Close the database
    pub async fn close(&self) -> Result<(), ServiceError> {
        self.backend.close().await
    }
}

/// Implementation of DagStorageBackend using RocksDB
pub struct RocksDbDagStorage {
    path: PathBuf,
    db: Arc<Mutex<Option<DB>>>,
}

impl RocksDbDagStorage {
    /// Create a new RocksDB storage backend at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            db: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Create RocksDB options
    fn create_options() -> Options {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_keep_log_file_num(10);
        options.set_max_open_files(32);
        options.set_use_fsync(false);
        options.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        options.set_write_buffer_size(64 * 1024 * 1024); // 64MB
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
        ]
    }
    
    /// Get a column family handle
    fn get_cf<'a>(&self, db: &'a DB, name: &str) -> Result<&'a ColumnFamily, ServiceError> {
        db.cf_handle(name)
            .ok_or_else(|| ServiceError::Database(format!("Column family {} not found", name)))
    }
    
    /// Initialize metadata if it doesn't exist
    async fn init_metadata(&self) -> Result<(), ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        
        if db.get_cf(cf_metadata, "metadata")?.is_none() {
            let metadata = DagMetadata::default();
            let metadata_bytes = serde_json::to_vec(&metadata)?;
            db.put_cf(cf_metadata, "metadata", metadata_bytes)?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl DagStorageBackend for RocksDbDagStorage {
    async fn init(&self) -> Result<(), ServiceError> {
        info!("Initializing RocksDB DAG storage at {:?}", self.path);
        
        // Create column families if they don't exist
        let cf_names = [CF_NODES, CF_METADATA, CF_SCOPE_INDEX, CF_TYPE_INDEX];
        
        let mut db_guard = self.db.lock().unwrap();
        
        let options = Self::create_options();
        let cf_descriptors = Self::create_cf_descriptors();
        
        match DB::open_cf_descriptors(&options, &self.path, cf_descriptors) {
            Ok(db) => {
                *db_guard = Some(db);
                drop(db_guard); // Release lock before async call
                self.init_metadata().await?;
                Ok(())
            }
            Err(e) => Err(ServiceError::Database(format!("Failed to open database: {}", e))),
        }
    }
    
    async fn store_node(&self, node: &DAGNode) -> Result<DAGNodeID, ServiceError> {
        let node_id = node.id()?;
        let node_bytes = serde_json::to_vec(node)?;
        
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
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
        
        debug!("Stored node {} of type {:?} in scope {}", 
            node_id.as_str(), 
            node.header.node_type, 
            node.header.scope
        );
        
        Ok(node_id)
    }
    
    async fn get_node(&self, id: &DAGNodeID) -> Result<DAGNode, ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        
        match db.get_cf(cf_nodes, id.as_str())? {
            Some(node_bytes) => {
                let node: DAGNode = serde_json::from_slice(&node_bytes)?;
                Ok(node)
            }
            None => Err(ServiceError::NodeNotFound(id.as_str().to_string())),
        }
    }
    
    async fn node_exists(&self, id: &DAGNodeID) -> Result<bool, ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        
        match db.get_cf(cf_nodes, id.as_str())? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
    
    async fn get_nodes_by_type(
        &self,
        node_type: DAGNodeType,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DAGNode>, ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
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
            let key_str = std::str::from_utf8(&key)?;
            
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
                ServiceError::Database(format!("Invalid type index key: {}", key_str))
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
    ) -> Result<Vec<DAGNode>, ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
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
            let key_str = std::str::from_utf8(&key)?;
            
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
                ServiceError::Database(format!("Invalid scope index key: {}", key_str))
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
    
    async fn get_children(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError> {
        // Since we don't store direct child references, we need to scan all nodes
        // and check if they have this node as a parent
        // This is inefficient for large DAGs but works for now
        // In a production implementation, we would maintain a children index
        
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        
        let mut result = Vec::new();
        let iter = db.iterator_cf(cf_nodes, rocksdb::IteratorMode::Start);
        
        for item in iter {
            let (_, node_bytes) = item?;
            let node: DAGNode = serde_json::from_slice(&node_bytes)?;
            
            if node.header.parents.contains(id) {
                result.push(node);
            }
        }
        
        Ok(result)
    }
    
    async fn get_parents(&self, id: &DAGNodeID) -> Result<Vec<DAGNode>, ServiceError> {
        let node = self.get_node(id).await?;
        
        let mut parents = Vec::with_capacity(node.header.parents.len());
        
        for parent_id in &node.header.parents {
            match self.get_node(parent_id).await {
                Ok(parent) => parents.push(parent),
                Err(e) => {
                    error!("Failed to get parent node {}: {:?}", parent_id.as_str(), e);
                    // Continue with other parents
                }
            }
        }
        
        Ok(parents)
    }
    
    async fn get_metadata(&self) -> Result<DagMetadata, ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        
        match db.get_cf(cf_metadata, "metadata")? {
            Some(metadata_bytes) => {
                let metadata: DagMetadata = serde_json::from_slice(&metadata_bytes)?;
                Ok(metadata)
            }
            None => {
                // This should not happen if init_metadata was called
                let metadata = DagMetadata::default();
                Ok(metadata)
            }
        }
    }
    
    async fn update_metadata(&self, metadata: &DagMetadata) -> Result<(), ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        
        let metadata_bytes = serde_json::to_vec(metadata)?;
        db.put_cf(cf_metadata, "metadata", metadata_bytes)?;
        
        Ok(())
    }
    
    async fn compact(&self) -> Result<(), ServiceError> {
        let db_guard = self.db.lock().unwrap();
        let db = db_guard.as_ref().ok_or_else(|| ServiceError::Database("Database not initialized".into()))?;
        
        let cf_nodes = self.get_cf(db, CF_NODES)?;
        let cf_metadata = self.get_cf(db, CF_METADATA)?;
        let cf_scope_index = self.get_cf(db, CF_SCOPE_INDEX)?;
        let cf_type_index = self.get_cf(db, CF_TYPE_INDEX)?;
        
        // Compact each column family
        db.compact_range_cf(cf_nodes, None::<&[u8]>, None::<&[u8]>);
        db.compact_range_cf(cf_metadata, None::<&[u8]>, None::<&[u8]>);
        db.compact_range_cf(cf_scope_index, None::<&[u8]>, None::<&[u8]>);
        db.compact_range_cf(cf_type_index, None::<&[u8]>, None::<&[u8]>);
        
        Ok(())
    }
    
    async fn close(&self) -> Result<(), ServiceError> {
        let mut db_guard = self.db.lock().unwrap();
        
        // Close the database by dropping it
        *db_guard = None;
        
        Ok(())
    }
}

/// Create a temporary database for testing
#[cfg(test)]
pub fn create_temp_db() -> (tempfile::TempDir, Arc<RocksDbDagStorage>) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(RocksDbDagStorage::new(temp_dir.path()));
    (temp_dir, storage)
} 