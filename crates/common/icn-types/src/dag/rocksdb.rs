#![cfg(feature = "persistence")]

use crate::cid::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode, PublicKeyResolver};
use crate::identity::Did;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB, WriteBatch};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock};
use ed25519_dalek::{Verifier, VerifyingKey};
use async_trait::async_trait;

// --- Prometheus Metrics --- 
use lazy_static::lazy_static;
use prometheus::{register_histogram, register_int_counter, register_int_gauge, Histogram, IntCounter, IntGauge, opts, register_histogram_vec, HistogramVec};

lazy_static! {
    static ref DAG_ADD_NODE_DURATION: Histogram = register_histogram!(
        "dag_add_node_duration_seconds",
        "Time taken to add a node to the RocksDB DAG store",
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    ).unwrap();

    static ref DAG_VERIFY_BRANCH_DURATION: Histogram = register_histogram!(
        "dag_verify_branch_duration_seconds",
        "Time taken to verify a DAG branch in RocksDB"
        // Add buckets appropriate for potentially longer verification times
    ).unwrap(); // Default buckets for now

    static ref DAG_NODE_VERIFICATION_FAILURES: IntCounter = register_int_counter!(
        "dag_node_verification_failures_total",
        "Total number of DAG node verification failures (signature, CID, missing parent)"
    ).unwrap();

    static ref DAG_TIP_COUNT: IntGauge = register_int_gauge!(
        "dag_tip_count",
        "Current number of tips in the RocksDB DAG"
    ).unwrap();

    static ref DAG_NODES_TOTAL: IntGauge = register_int_gauge!(
        "dag_nodes_total",
        "Total number of nodes in the RocksDB DAG"
    ).unwrap();
}
// --- End Prometheus Metrics ---

/// ColumnFamily names for different types of data
const CF_NODES: &str = "nodes";
const CF_TIPS: &str = "tips";
const CF_CHILDREN: &str = "children";
const CF_AUTHORS: &str = "authors";
const CF_PAYLOAD_TYPES: &str = "payload_types";

/// RocksDB-based implementation of the DagStore trait
pub struct RocksDbDagStore {
    db: Arc<DB>,
    // Cache of nodes that have children (not tips)
    non_tips: Arc<RwLock<HashSet<Vec<u8>>>>,
}

impl RocksDbDagStore {
    /// Open a RocksDB database at the specified path, creating it if it doesn't exist
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DagError> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        // Define column families
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_NODES, Options::default()),
            ColumnFamilyDescriptor::new(CF_TIPS, Options::default()),
            ColumnFamilyDescriptor::new(CF_CHILDREN, Options::default()),
            ColumnFamilyDescriptor::new(CF_AUTHORS, Options::default()),
            ColumnFamilyDescriptor::new(CF_PAYLOAD_TYPES, Options::default()),
        ];

        // Open the database
        let db = DB::open_cf_descriptors(&db_opts, path, cf_descriptors)
            .map_err(|e| DagError::StorageError(format!("Failed to open RocksDB: {}", e)))?;

        let store = Self {
            db: Arc::new(db),
            non_tips: Arc::new(RwLock::new(HashSet::new())),
        };

        // Initialize the non_tips cache
        store.initialize_non_tips_cache()?;
        
        // Initialize total nodes gauge (approximation on open)
        store.update_nodes_total_gauge()?; 

        Ok(store)
    }

    /// Initialize the cache of non-tip nodes
    fn initialize_non_tips_cache(&self) -> Result<(), DagError> {
        let cf_children = self
            .db
            .cf_handle(CF_CHILDREN)
            .ok_or_else(|| DagError::StorageError("Children column family not found".to_string()))?;

        let mut cache = self
            .non_tips
            .write()
            .map_err(|e| DagError::StorageError(format!("Failed to acquire write lock: {}", e)))?;

        // Iterate through all entries in the children column family
        let iter = self.db.iterator_cf(cf_children, rocksdb::IteratorMode::Start);
        for result in iter {
            let (key, _) = result
                .map_err(|e| DagError::StorageError(format!("Error iterating database: {}", e)))?;
            cache.insert(key.to_vec());
        }

        Ok(())
    }

    /// Get a column family handle by name
    fn cf_handle(&self, name: &str) -> Result<&ColumnFamily, DagError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| DagError::StorageError(format!("Column family not found: {}", name)))
    }

    /// Serialize a DAG node to bytes using DAG-CBOR
    fn serialize_node(node: &SignedDagNode) -> Result<Vec<u8>, DagError> {
        serde_ipld_dagcbor::to_vec(node)
             .map_err(|e| DagError::SerializationError(format!("DAG-CBOR serialization error (node): {}", e)))
    }

    /// Deserialize a DAG node from DAG-CBOR bytes
    fn deserialize_node(bytes: &[u8]) -> Result<SignedDagNode, DagError> {
        serde_ipld_dagcbor::from_slice(bytes)
            .map_err(|e| DagError::SerializationError(format!("DAG-CBOR deserialization error (node): {}", e)))
    }
    
    /// Serialize a list of CID bytes using DAG-CBOR
    fn serialize_cid_list(list: &Vec<Vec<u8>>) -> Result<Vec<u8>, DagError> {
        serde_ipld_dagcbor::to_vec(list)
            .map_err(|e| DagError::SerializationError(format!("DAG-CBOR serialization error (CID list): {}", e)))
    }
    
    /// Deserialize a list of CID bytes from DAG-CBOR bytes
    fn deserialize_cid_list(bytes: &[u8]) -> Result<Vec<Vec<u8>>, DagError> {
        serde_ipld_dagcbor::from_slice(bytes)
            .map_err(|e| DagError::SerializationError(format!("DAG-CBOR deserialization error (CID list): {}", e)))
    }

    /// Serialize a CID to use as a key
    fn cid_to_key(cid: &Cid) -> Vec<u8> {
        cid.to_bytes()
    }

    /// Update the tips when a new node is added
    fn update_tips(&self, node: &SignedDagNode) -> Result<(), DagError> {
        let cf_tips = self.cf_handle(CF_TIPS)?;
        let node_cid = node.cid.as_ref().unwrap(); // Safe because we ensure CID is computed before adding
        let node_key = Self::cid_to_key(node_cid);

        // Add this node as a tip
        self.db
            .put_cf(cf_tips, &node_key, &[1])
            .map_err(|e| DagError::StorageError(format!("Failed to add tip: {}", e)))?;

        // Remove all parent nodes from tips, as they now have a child
        let mut non_tips = self
            .non_tips
            .write()
            .map_err(|e| DagError::StorageError(format!("Failed to acquire write lock: {}", e)))?;

        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            self.db
                .delete_cf(cf_tips, &parent_key)
                .map_err(|e| DagError::StorageError(format!("Failed to remove tip: {}", e)))?;

            // Add to non-tips cache
            non_tips.insert(parent_key);
        }

        Ok(())
    }

    /// Update the children mapping when a new node is added
    fn update_children(&self, node: &SignedDagNode) -> Result<(), DagError> {
        let cf_children = self.cf_handle(CF_CHILDREN)?;
        let node_cid = node.cid.as_ref().unwrap(); // Safe because we ensure CID is computed before adding
        let node_key = Self::cid_to_key(node_cid);

        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            
            // Get existing children list
            let existing = self.db.get_cf(cf_children, &parent_key)
                .map_err(|e| DagError::StorageError(format!("Failed to get children: {}", e)))?;
            
            let mut children: Vec<Vec<u8>> = match existing {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => Vec::new(),
            };
            
            // Add new child
            children.push(node_key.clone());
            
            // Store updated children list
            let serialized = Self::serialize_cid_list(&children)?;
                
            self.db.put_cf(cf_children, &parent_key, &serialized)
                .map_err(|e| DagError::StorageError(format!("Failed to update children: {}", e)))?;
        }

        Ok(())
    }

    /// Update the author index when a new node is added
    fn update_authors(&self, node: &SignedDagNode) -> Result<(), DagError> {
        let cf_authors = self.cf_handle(CF_AUTHORS)?;
        let author_key = node.node.author.to_string().into_bytes();
        let node_cid = node.cid.as_ref().unwrap(); // Safe because we ensure CID is computed before adding
        let node_key = Self::cid_to_key(node_cid);
        
        // Get existing nodes for this author
        let existing = self.db.get_cf(cf_authors, &author_key)
            .map_err(|e| DagError::StorageError(format!("Failed to get author nodes: {}", e)))?;
        
        let mut author_nodes: Vec<Vec<u8>> = match existing {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => Vec::new(),
        };
        
        // Add new node
        author_nodes.push(node_key);
        
        // Store updated author nodes list
        let serialized = Self::serialize_cid_list(&author_nodes)?;
            
        self.db.put_cf(cf_authors, &author_key, &serialized)
            .map_err(|e| DagError::StorageError(format!("Failed to update author nodes: {}", e)))?;

        Ok(())
    }

    /// Update the payload type index when a new node is added
    fn update_payload_types(&self, node: &SignedDagNode) -> Result<(), DagError> {
        let cf_payload_types = self.cf_handle(CF_PAYLOAD_TYPES)?;
        let payload_type = match &node.node.payload {
            crate::dag::DagPayload::Raw(_) => "raw",
            crate::dag::DagPayload::Json(_) => "json",
            crate::dag::DagPayload::Reference(_) => "reference",
            crate::dag::DagPayload::TrustBundle(_) => "trustbundle",
            crate::dag::DagPayload::ExecutionReceipt(_) => "receipt",
        };
        let payload_key = payload_type.as_bytes();
        let node_cid = node.cid.as_ref().unwrap(); // Safe because we ensure CID is computed before adding
        let node_key = Self::cid_to_key(node_cid);
        
        // Get existing nodes for this payload type
        let existing = self.db.get_cf(cf_payload_types, payload_key)
            .map_err(|e| DagError::StorageError(format!("Failed to get payload type nodes: {}", e)))?;
        
        let mut payload_nodes: Vec<Vec<u8>> = match existing {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => Vec::new(),
        };
        
        // Add new node
        payload_nodes.push(node_key);
        
        // Store updated payload type nodes list
        let serialized = Self::serialize_cid_list(&payload_nodes)?;
            
        self.db.put_cf(cf_payload_types, payload_key, &serialized)
            .map_err(|e| DagError::StorageError(format!("Failed to update payload type nodes: {}", e)))?;

        Ok(())
    }

    /// Helper to update total nodes gauge
    fn update_nodes_total_gauge(&self) -> Result<(), DagError> {
         let cf_nodes = self.cf_handle(CF_NODES)?;
         let mut count = 0;
         let iter = self.db.iterator_cf(cf_nodes, rocksdb::IteratorMode::Start);
         for _ in iter {
            count += 1;
         }
         DAG_NODES_TOTAL.set(count);
         Ok(())
    }
}

// Synchronous implementation
#[cfg(not(feature = "async"))]
impl DagStore for RocksDbDagStore {
    fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        let _timer = DAG_ADD_NODE_DURATION.start_timer(); // Start timing

        let node_cid = node.ensure_cid()?; 
        let node_key = Self::cid_to_key(&node_cid);
        
        // Check if node already exists to avoid re-incrementing counter etc.
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let node_exists = self.db.get_cf(cf_nodes, &node_key)?.is_some();

        let node_bytes = Self::serialize_node(&node)?;

        let mut batch = WriteBatch::default();
        batch.put_cf(cf_nodes, &node_key, &node_bytes);
        
        // 2. Update tips
        let cf_tips = self.cf_handle(CF_TIPS)?;
        // Add this node as a potential tip
        batch.put_cf(cf_tips, &node_key, &[1]);
        // Remove parents from tips
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            batch.delete_cf(cf_tips, &parent_key);
        }

        // 3. Update children index
        let cf_children = self.cf_handle(CF_CHILDREN)?;
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            // Get existing children list (Read operation, outside batch)
            let existing = self.db.get_cf(cf_children, &parent_key)
                .map_err(|e| DagError::StorageError(format!("Failed to get children: {}", e)))?;

            let mut children: Vec<Vec<u8>> = match existing {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => Vec::new(),
            };
            // Add new child
            if !children.contains(&node_key) { // Avoid duplicates if node added multiple times?
                children.push(node_key.clone());
            }
            // Store updated children list in batch (using DAG-CBOR)
            let serialized_children = Self::serialize_cid_list(&children)?;
            batch.put_cf(cf_children, &parent_key, &serialized_children);
        }

        // 4. Update author index
        let cf_authors = self.cf_handle(CF_AUTHORS)?;
        let author_key = node.node.author.to_string().into_bytes();
        // Get existing nodes list (Read operation, outside batch)
        let existing_author_nodes = self.db.get_cf(cf_authors, &author_key)
            .map_err(|e| DagError::StorageError(format!("Failed to get author nodes: {}", e)))?;

        let mut author_nodes: Vec<Vec<u8>> = match existing_author_nodes {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => Vec::new(),
        };
        // Add new node
        if !author_nodes.contains(&node_key) { // Avoid duplicates
            author_nodes.push(node_key.clone());
        }
        // Store updated author nodes list in batch (using DAG-CBOR)
        let serialized_author_nodes = Self::serialize_cid_list(&author_nodes)?;
        batch.put_cf(cf_authors, &author_key, &serialized_author_nodes);


        // 5. Update payload type index
        let cf_payload_types = self.cf_handle(CF_PAYLOAD_TYPES)?;
        let payload_type_str = match &node.node.payload {
            crate::dag::DagPayload::Raw(_) => "raw",
            crate::dag::DagPayload::Json(_) => "json",
            crate::dag::DagPayload::Reference(_) => "reference",
            crate::dag::DagPayload::TrustBundle(_) => "TrustBundle",
            crate::dag::DagPayload::ExecutionReceipt(_) => "ExecutionReceipt",
            // Add other types as needed
        };
        let payload_type_key = payload_type_str.as_bytes().to_vec();
        // Get existing nodes list (Read operation, outside batch)
        let existing_payload_nodes = self.db.get_cf(cf_payload_types, &payload_type_key)
             .map_err(|e| DagError::StorageError(format!("Failed to get payload type nodes: {}", e)))?;

        let mut payload_nodes: Vec<Vec<u8>> = match existing_payload_nodes {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => Vec::new(),
        };
        // Add new node
        if !payload_nodes.contains(&node_key) { // Avoid duplicates
             payload_nodes.push(node_key.clone());
        }
         // Store updated payload nodes list in batch (using DAG-CBOR)
        let serialized_payload_nodes = Self::serialize_cid_list(&payload_nodes)?;
        batch.put_cf(cf_payload_types, &payload_type_key, &serialized_payload_nodes);

        // --- Commit the Atomic Write Batch ---
        self.db.write(batch).map_err(|e| {
            DagError::StorageError(format!("Atomic batch write failed: {}", e))
        })?;

        // --- Update In-Memory Cache (After successful commit) ---
        // Add parents to non-tips cache
        { // Scope the lock guard
            let mut non_tips = self
                .non_tips
                .write()
                .map_err(|e| DagError::StorageError(format!("Failed to acquire write lock for cache: {}", e)))?;
            for parent_cid in &node.node.parents {
                 non_tips.insert(Self::cid_to_key(parent_cid));
            }
        } // Lock guard dropped here

        // Increment node count only if it was a new node
        if !node_exists {
             DAG_NODES_TOTAL.inc();
        }

        // Note: Updating tip count here accurately is complex.
        // It depends on whether parents were already tips.
        // Deferring tip count update to get_tips or a periodic task.

        Ok(node_cid)
    }

    fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let node_key = Self::cid_to_key(cid);
        
        match self.db.get_cf(cf_nodes, &node_key)? {
            Some(data) => Self::deserialize_node(&data),
            None => Err(DagError::NodeNotFound(cid.clone())),
        }
    }

    fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let cf_tips = self.cf_handle(CF_TIPS)?;
        let mut tips = Vec::new();
        
        let iter = self.db.iterator_cf(cf_tips, rocksdb::IteratorMode::Start);
        for result in iter {
            let (key, _) = result
                .map_err(|e| DagError::StorageError(format!("Error iterating tips: {}", e)))?;
            
            let cid = Cid::from_bytes(&key)
                .map_err(|e| DagError::CidError(format!("Invalid CID bytes: {}", e)))?;
                
            tips.push(cid);
        }
        
        // Update the gauge when tips are fetched
        DAG_TIP_COUNT.set(tips.len() as i64);
        Ok(tips)
    }

    fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        unimplemented!("Synchronous get_ordered_nodes not implemented for RocksDB store");
    }

    fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        unimplemented!("Synchronous get_nodes_by_author not implemented for RocksDB store");
    }

    fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        unimplemented!("Synchronous get_nodes_by_payload_type not implemented for RocksDB store");
    }

    fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        unimplemented!("Synchronous find_path not implemented for RocksDB store");
    }

    fn verify_branch(&self, tip: &Cid, resolver: &dyn PublicKeyResolver) -> Result<(), DagError> {
        let _timer = DAG_VERIFY_BRANCH_DURATION.start_timer(); // Start timing

        let tip_key = Self::cid_to_key(tip);
        let cf_nodes = self.cf_handle(CF_NODES)?;
        
        // Check if tip exists
        if self.db.get_cf(cf_nodes, &tip_key)?.is_none() {
            // Don't count "not found" as a verification failure, it's a pre-condition fail
            return Err(DagError::NodeNotFound(tip.clone()));
        }
        
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        queue.push_back(tip_key.clone());
        visited.insert(tip_key);
        
        while let Some(current_key) = queue.pop_front() {
            let node_data = self.db.get_cf(cf_nodes, &current_key)?
                .ok_or_else(|| { 
                    // This case indicates DB inconsistency if visited key is missing
                    DAG_NODE_VERIFICATION_FAILURES.inc(); 
                    DagError::StorageError(format!("Node data missing for visited key {:?}", current_key))
                })?;
            let signed_node = Self::deserialize_node(&node_data)?;
            let node_cid = match signed_node.cid.as_ref() {
                Some(cid) => cid.clone(),
                None => {
                    DAG_NODE_VERIFICATION_FAILURES.inc();
                    return Err(DagError::InvalidNodeData(format!("Missing CID in deserialized node for key {:?}", current_key)));
                }
            };

            // 1. Verify CID calculation matches stored CID
            let calculated_cid = signed_node.calculate_cid()?;
            if calculated_cid != node_cid {
                DAG_NODE_VERIFICATION_FAILURES.inc();
                return Err(DagError::CidMismatch(node_cid));
            }

            // 2. Verify the signature
            let verifying_key = match resolver.resolve(&signed_node.node.author) {
                Ok(key) => key,
                Err(e) => {
                    DAG_NODE_VERIFICATION_FAILURES.inc(); // Count resolution errors as verification failures
                    return Err(e);
                }
            };
            let canonical_bytes = serde_ipld_dagcbor::to_vec(&signed_node.node)
                .map_err(|e| DagError::SerializationError(format!("DAG-CBOR serialization error during verify: {}", e)))?;
            if let Err(_) = verifying_key.verify(&canonical_bytes, &signed_node.signature) {
                DAG_NODE_VERIFICATION_FAILURES.inc();
                return Err(DagError::InvalidSignature(node_cid.clone()));
            }

            // 3. Check parent existence and add to queue
            for parent_cid in &signed_node.node.parents {
                let parent_key = Self::cid_to_key(parent_cid);
                if self.db.get_cf(cf_nodes, &parent_key)?.is_none() {
                    DAG_NODE_VERIFICATION_FAILURES.inc();
                    return Err(DagError::MissingParent(parent_cid.clone()));
                }
                if !visited.contains(&parent_key) {
                    visited.insert(parent_key.clone());
                    queue.push_back(parent_key);
                }
            }
        }
        
        Ok(())
    }
}

// Asynchronous implementation
#[cfg(feature = "async")]
#[async_trait]
impl DagStore for RocksDbDagStore {
    async fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        let _timer = DAG_ADD_NODE_DURATION.start_timer(); // Start timing

        let node_cid = node.ensure_cid()?;
        let node_key = Self::cid_to_key(&node_cid);
        let node_bytes = Self::serialize_node(&node)?;
        
        // Check existence before spawn_blocking to correctly update counter
        let node_exists = {
            let cf_nodes = self.cf_handle(CF_NODES)?;
            self.db.get_cf(cf_nodes, &node_key)?.is_some()
        };

        let db_clone = Arc::clone(&self.db);
        
        let batch_result = tokio::task::spawn_blocking(move || {
            let mut batch = WriteBatch::default();

            // Get handles (assuming cf_handle is cheap or cached internally by rocksdb crate)
            let cf_nodes = db_clone.cf_handle(CF_NODES)
                 .ok_or_else(|| DagError::StorageError("Nodes CF not found".to_string()))?;
            let cf_tips = db_clone.cf_handle(CF_TIPS)
                .ok_or_else(|| DagError::StorageError("Tips CF not found".to_string()))?;
            let cf_children = db_clone.cf_handle(CF_CHILDREN)
                 .ok_or_else(|| DagError::StorageError("Children CF not found".to_string()))?;
            let cf_authors = db_clone.cf_handle(CF_AUTHORS)
                 .ok_or_else(|| DagError::StorageError("Authors CF not found".to_string()))?;
            let cf_payload_types = db_clone.cf_handle(CF_PAYLOAD_TYPES)
                 .ok_or_else(|| DagError::StorageError("PayloadTypes CF not found".to_string()))?;

            // 1. Add node data
            batch.put_cf(cf_nodes, &node_key, &node_bytes);

            // 2. Update tips
            batch.put_cf(cf_tips, &node_key, &[1]);
            let mut parent_keys = Vec::new(); // Collect parent keys for cache update later
            for parent_cid in &node.node.parents {
                let parent_key = Self::cid_to_key(parent_cid);
                batch.delete_cf(cf_tips, &parent_key);
                parent_keys.push(parent_key);
            }

            // 3. Update children index (using DAG-CBOR)
            for parent_cid in &node.node.parents {
                 let parent_key = Self::cid_to_key(parent_cid);
                 let existing = db_clone.get_cf(cf_children, &parent_key)
                     .map_err(|e| DagError::StorageError(format!("Failed to get children: {}", e)))?;
                 let mut children: Vec<Vec<u8>> = match existing {
                    Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                    None => Vec::new(),
                 };
                 if !children.contains(&node_key) {
                    children.push(node_key.clone());
                 }
                 let serialized_children = Self::serialize_cid_list(&children)?;
                 batch.put_cf(cf_children, &parent_key, &serialized_children);
            }

            // 4. Update author index (using DAG-CBOR)
            let author_key = node.node.author.to_string().into_bytes();
            let existing_author_nodes = db_clone.get_cf(cf_authors, &author_key)
                 .map_err(|e| DagError::StorageError(format!("Failed to get author nodes: {}", e)))?;
            let mut author_nodes: Vec<Vec<u8>> = match existing_author_nodes {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => Vec::new(),
            };
            if !author_nodes.contains(&node_key) {
                 author_nodes.push(node_key.clone());
            }
            let serialized_author_nodes = Self::serialize_cid_list(&author_nodes)?;
            batch.put_cf(cf_authors, &author_key, &serialized_author_nodes);

             // 5. Update payload type index (using DAG-CBOR)
            let payload_type_str = match &node.node.payload {
                crate::dag::DagPayload::Raw(_) => "raw",
                crate::dag::DagPayload::Json(_) => "json",
                crate::dag::DagPayload::Reference(_) => "reference",
                crate::dag::DagPayload::TrustBundle(_) => "TrustBundle",
                crate::dag::DagPayload::ExecutionReceipt(_) => "ExecutionReceipt",
            };
            let payload_type_key = payload_type_str.as_bytes().to_vec();
            let existing_payload_nodes = db_clone.get_cf(cf_payload_types, &payload_type_key)
                 .map_err(|e| DagError::StorageError(format!("Failed to get payload type nodes: {}", e)))?;
            let mut payload_nodes: Vec<Vec<u8>> = match existing_payload_nodes {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => Vec::new(),
            };
            if !payload_nodes.contains(&node_key) {
                 payload_nodes.push(node_key.clone());
            }
            let serialized_payload_nodes = Self::serialize_cid_list(&payload_nodes)?;
            batch.put_cf(cf_payload_types, &payload_type_key, &serialized_payload_nodes);

            // --- Commit Batch ---
            db_clone.write(batch).map_err(|e| {
                DagError::StorageError(format!("Atomic batch write failed: {}", e))
            })?;

            // Return parent keys needed for cache update
            Ok::<_, DagError>(parent_keys)

        }).await.map_err(|e| DagError::JoinError(e.to_string()))??; // Handle join error and inner Result

        // --- Update In-Memory Cache (After successful commit) ---
        let parent_keys = batch_result; // Get parent keys from blocking task result
        { // Scope the lock guard
            let mut non_tips = self
                 .non_tips
                 .write() // Consider using blocking_write if this might contend heavily
                 .map_err(|e| DagError::StorageError(format!("Failed to acquire write lock for cache: {}", e)))?;
             for parent_key in parent_keys {
                 non_tips.insert(parent_key);
            }
        } // Lock guard dropped here

        // Increment node count only if it was a new node
        if !node_exists {
             DAG_NODES_TOTAL.inc();
        }
        // Deferring tip count update

        Ok(node_cid)
    }

    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let cid_clone = cid.clone();
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let cf_nodes = db.cf_handle(CF_NODES)
                .ok_or_else(|| DagError::StorageError(format!("Column family not found: {}", CF_NODES)))?;
                
            let node_key = Self::cid_to_key(&cid_clone);
            
            match db.get_cf(cf_nodes, &node_key)? {
                Some(data) => Self::deserialize_node(&data),
                None => Err(DagError::NodeNotFound(cid_clone)),
            }
        }).await.map_err(|e| DagError::JoinError(e.to_string()))?
    }

    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let cf_tips = db.cf_handle(CF_TIPS)
                .ok_or_else(|| DagError::StorageError(format!("Column family not found: {}", CF_TIPS)))?;
                
            let mut tips = Vec::new();
            
            let iter = db.iterator_cf(cf_tips, rocksdb::IteratorMode::Start);
            for result in iter {
                let (key, _) = result
                    .map_err(|e| DagError::StorageError(format!("Error iterating tips: {}", e)))?;
                
                let cid = Cid::from_bytes(&key)
                    .map_err(|e| DagError::CidError(format!("Invalid CID bytes: {}", e)))?;
                    
                tips.push(cid);
            }
            
            Ok(tips)
        }).await.map_err(|e| DagError::JoinError(e.to_string()))?
    }

    async fn verify_branch(&self, tip: &Cid, resolver: &dyn PublicKeyResolver) -> Result<(), DagError> {
        let _timer = DAG_VERIFY_BRANCH_DURATION.start_timer(); // Start timing
        
        let tip_clone = tip.clone();
        let db_clone = self.db.clone();
        // TODO: Properly handle passing resolver

        let verification_result = tokio::task::spawn_blocking(move || {
            let tip_key = Self::cid_to_key(&tip_clone);
            // Use ok_or_else to provide a DagError if cf_handle returns None
            let cf_nodes = db_clone.cf_handle(CF_NODES)
                .ok_or_else(|| DagError::StorageError("Nodes column family not found".to_string()))?;
            
            if db_clone.get_cf(cf_nodes, &tip_key)?.is_none() {
                return Err(DagError::NodeNotFound(tip_clone));
            }
            
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(tip_key.clone());

            while let Some(current_key) = queue.pop_front() {
                let node_data = db_clone.get_cf(cf_nodes, &current_key)?
                     .ok_or_else(|| { 
                        // Don't inc counter here, let the outer handler do it
                        DagError::StorageError(format!("Node data missing for visited key {:?}", current_key))
                     })?;
                let signed_node = Self::deserialize_node(&node_data)?;
                let node_cid = match signed_node.cid.as_ref() {
                     Some(cid) => cid.clone(),
                     None => return Err(DagError::InvalidNodeData(format!("Missing CID in deserialized node for key {:?}", current_key)))
                };

                // 1. Verify CID
                let calculated_cid = signed_node.calculate_cid()?;
                if calculated_cid != node_cid {
                    return Err(DagError::CidMismatch(node_cid));
                }

                // 2. Verify Signature
                let verifying_key = resolver.resolve(&signed_node.node.author)?;
                let canonical_bytes = serde_ipld_dagcbor::to_vec(&signed_node.node)
                     .map_err(|e| DagError::SerializationError(format!("DAG-CBOR serialization error during verify: {}", e)))?;
                verifying_key.verify(&canonical_bytes, &signed_node.signature)
                     .map_err(|_| DagError::InvalidSignature(node_cid.clone()))?;

                // 3. Check Parents
                for parent_cid in &signed_node.node.parents {
                     let parent_key = Self::cid_to_key(parent_cid);
                     if db_clone.get_cf(cf_nodes, &parent_key)?.is_none() {
                         return Err(DagError::MissingParent(parent_cid.clone()));
                     }
                     if !visited.contains(&parent_key) {
                         visited.insert(parent_key.clone());
                         queue.push_back(parent_key);
                     }
                 }
            }
            Ok(())
        }).await??; // Double ?? to handle JoinError then inner Result<(), DagError>
        
        // Handle result and increment counter on specific errors from the blocking task
        match verification_result {
            Ok(Ok(())) => Ok(()), // Inner Ok(()) means success
            Ok(Err(e @ DagError::CidMismatch(_)))
            | Ok(Err(e @ DagError::InvalidSignature(_)))
            | Ok(Err(e @ DagError::MissingParent(_)))
            | Ok(Err(e @ DagError::PublicKeyResolutionError(_, _))) => {
                DAG_NODE_VERIFICATION_FAILURES.inc();
                Err(e)
            },
            Ok(Err(e)) => Err(e), // Other errors (Storage, Serialization, etc.)
            Err(join_err) => Err(join_err), // JoinError
        }
    }
} 