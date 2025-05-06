#![cfg(feature = "persistence")]

use crate::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode, PublicKeyResolver};
use crate::Did;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB, WriteBatch};
use std::collections::{HashSet, HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock};
use async_trait::async_trait;

// --- Prometheus Metrics --- 
use lazy_static::lazy_static;
use prometheus::{register_histogram, register_int_counter, register_int_gauge, Histogram, IntCounter, IntGauge};

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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
        let cf_name_nodes: &'static str = CF_NODES;

        // --- Build graph structure directly ---
        let mut nodes_map = HashMap::<Cid, SignedDagNode>::new();
        let mut adj_list = HashMap::<Cid, Vec<Cid>>::new(); // parent -> children
        let mut in_degree = HashMap::<Cid, usize>::new(); // child -> parent_count
        let mut all_node_cids = HashSet::<Cid>::new(); // Keep track of all nodes encountered

        let cf_handle_nodes = self.cf_handle(cf_name_nodes)?; // Use self.cf_handle

        let iter = self.db.iterator_cf(cf_handle_nodes, rocksdb::IteratorMode::Start);
        for result in iter {
            let (_key, value) = result.map_err(DagError::RocksDbError)?;
            let mut signed_node = Self::deserialize_node(&value)?;
            let node_cid = signed_node.ensure_cid()?; // Ensure CID is present
            
            all_node_cids.insert(node_cid.clone());
            nodes_map.insert(node_cid.clone(), signed_node.clone());

            // Initialize in-degree for this node
            in_degree.entry(node_cid.clone()).or_insert(0);

            // Process parents to build adjacency list and in-degrees
            for parent_cid in &signed_node.node.parents {
                adj_list.entry(parent_cid.clone()).or_default().push(node_cid.clone());
                *in_degree.entry(node_cid.clone()).or_insert(0) += 1;
                in_degree.entry(parent_cid.clone()).or_insert(0);
            }
        }
        
        // Ensure all nodes are in the in_degree map
        for parent_cid in adj_list.keys() {
            in_degree.entry(parent_cid.clone()).or_insert(0);
        }

        // --- Kahn's Algorithm for Topological Sort ---
        let mut sorted_list = Vec::with_capacity(nodes_map.len());
        let mut queue = VecDeque::new();

        // Initialize queue with nodes having in-degree 0
        for (cid, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(cid.clone());
            }
        }
        
        if nodes_map.is_empty() {
            return Ok(sorted_list);
        }
        
        if queue.is_empty() && !nodes_map.is_empty() {
             return Err(DagError::StorageError("Cycle detected in DAG or no root nodes found".to_string()));
        }

        while let Some(cid) = queue.pop_front() {
            if let Some(node) = nodes_map.get(&cid) {
                 sorted_list.push(node.clone());
            } else {
                 return Err(DagError::StorageError(format!("Node {} found in queue but not in map", cid)));
            }

            if let Some(children) = adj_list.get(&cid) {
                for child_cid in children {
                    // Need mutable access to in_degree here
                    if let Some(degree) = in_degree.get_mut(child_cid) { 
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child_cid.clone());
                        }
                    } else {
                         return Err(DagError::StorageError(format!("Child node {} of {} not found in in-degree map", child_cid, cid)));
                    }
                }
            }
        }

        if sorted_list.len() != nodes_map.len() {
            Err(DagError::StorageError(format!(
                "Cycle detected in DAG. Processed {} nodes, expected {}.",
                sorted_list.len(),
                nodes_map.len()
            )))
        } else {
            Ok(sorted_list)
        }
    }

    fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let author_key = author.to_string().into_bytes();
        let cf_authors = self.cf_handle(CF_AUTHORS)?;
        let cf_nodes = self.cf_handle(CF_NODES)?;

        // 1. Get the list of node CID bytes from the author index
        let node_cid_bytes_list: Vec<Vec<u8>> = match self.db.get_cf(cf_authors, &author_key)? {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => return Ok(Vec::new()), // No nodes found for this author
        };

        // 2. Retrieve and deserialize each node from the main nodes CF
        let mut nodes = Vec::with_capacity(node_cid_bytes_list.len());
        for node_cid_bytes in node_cid_bytes_list {
            match self.db.get_cf(cf_nodes, &node_cid_bytes)? {
                Some(node_bytes) => {
                    let node = Self::deserialize_node(&node_bytes)?;
                    nodes.push(node);
                }
                None => {
                    // This indicates an inconsistency between the index and the nodes table
                    let cid_str = Cid::from_bytes(&node_cid_bytes).map(|c| c.to_string()).unwrap_or_else(|_| hex::encode(&node_cid_bytes));
                    return Err(DagError::StorageError(format!(
                        "Author index points to non-existent node CID: {}", cid_str
                    )));
                }
            }
        }

        Ok(nodes)
    }

    fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let payload_key = payload_type.as_bytes().to_vec();
        let cf_payload_types = self.cf_handle(CF_PAYLOAD_TYPES)?;
        let cf_nodes = self.cf_handle(CF_NODES)?;

        // 1. Get the list of node CID bytes from the payload type index
        let node_cid_bytes_list: Vec<Vec<u8>> = match self.db.get_cf(cf_payload_types, &payload_key)? {
            Some(bytes) => Self::deserialize_cid_list(&bytes)?,
            None => return Ok(Vec::new()), // No nodes found for this payload type
        };

        // 2. Retrieve and deserialize each node from the main nodes CF
        let mut nodes = Vec::with_capacity(node_cid_bytes_list.len());
        for node_cid_bytes in node_cid_bytes_list {
            match self.db.get_cf(cf_nodes, &node_cid_bytes)? {
                Some(node_bytes) => {
                    let node = Self::deserialize_node(&node_bytes)?;
                    nodes.push(node);
                }
                None => {
                    // This indicates an inconsistency between the index and the nodes table
                    let cid_str = Cid::from_bytes(&node_cid_bytes).map(|c| c.to_string()).unwrap_or_else(|_| hex::encode(&node_cid_bytes));
                    return Err(DagError::StorageError(format!(
                        "Payload type index points to non-existent node CID: {}", cid_str
                    )));
                }
            }
        }

        Ok(nodes)
    }

    fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let cf_nodes = self.cf_handle(CF_NODES)?;

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut predecessors = HashMap::new(); // child -> parent

        queue.push_back(from.clone());
        visited.insert(from.clone());
        let mut target_found = false;

        while let Some(current_cid) = queue.pop_front() {
            if current_cid == *to {
                target_found = true;
                break;
            }

            let node_bytes = self.db.get_cf(cf_nodes, &Self::cid_to_key(&current_cid))?
                .ok_or_else(|| DagError::NodeNotFound(current_cid.clone()))?;
            let signed_node = Self::deserialize_node(&node_bytes)?;

            for parent_cid in &signed_node.node.parents {
                if visited.insert(parent_cid.clone()) {
                    predecessors.insert(parent_cid.clone(), current_cid.clone());
                    queue.push_back(parent_cid.clone());
                }
            }
        }

        if !target_found {
            return Ok(Vec::new());
        }

        // Reconstruct path
        let mut path_cids = VecDeque::new();
        let mut current = to.clone();
        while current != *from {
            path_cids.push_front(current.clone());
            match predecessors.get(&current) {
                Some(pred) => current = pred.clone(),
                None => return Err(DagError::StorageError("Path reconstruction failed".to_string())),
            }
        }
        path_cids.push_front(from.clone());

        // Retrieve nodes
        let mut path_nodes = Vec::with_capacity(path_cids.len());
        for cid in path_cids {
            let node_bytes = self.db.get_cf(cf_nodes, &Self::cid_to_key(&cid))?
                .ok_or_else(|| DagError::NodeNotFound(cid.clone()))?;
            let signed_node = Self::deserialize_node(&node_bytes)?;
            path_nodes.push(signed_node);
        }

        Ok(path_nodes)
    }

    fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        // TODO: Implement sync verification logic
        Ok(()) // Assume valid for now
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

        }).await.map_err(DagError::from)??; // Handle join error and inner Result

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
        let db_clone = self.db.clone();
        let cf_name: &'static str = CF_NODES;
        let key = Self::cid_to_key(&cid_clone);

        let node_bytes = tokio::task::spawn_blocking(move || {
            let cf_handle = db_clone.cf_handle(cf_name)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name)))?;
            db_clone.get_cf(cf_handle, key)
                .map_err(DagError::RocksDbError)? // Map RocksDB error
                .ok_or_else(|| DagError::NodeNotFound(cid_clone.clone())) // Map Option to NotFound
        }).await.map_err(DagError::JoinError)??; // Map JoinError and inner Result

        Self::deserialize_node(&node_bytes)
    }

    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let db_clone = self.db.clone();
        // Pass CF name as string slice, not &ColumnFamily
        let cf_name: &'static str = CF_TIPS; 
        
        let tip_keys: Vec<Box<[u8]>> = tokio::task::spawn_blocking(move || {
            // Get CF handle inside the closure and map error
            let cf_handle = db_clone.cf_handle(cf_name)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name)))?; 
            let mut keys = Vec::new();
            let iter = db_clone.iterator_cf(cf_handle, rocksdb::IteratorMode::Start);
            for result in iter {
                let (key, _) = result.map_err(DagError::RocksDbError)?;
                keys.push(key);
            }
            // Return Ok<_, DagError>
            return Ok::<_, DagError>(keys); 
        }).await.map_err(DagError::JoinError)??;

        let mut tips = Vec::new();
        for key in tip_keys {
            // Use Cid::from_bytes which expects &[u8] and returns Result<Cid, CidError>
            let cid = Cid::from_bytes(key.as_ref())
                .map_err(|e| DagError::CidError(format!("Invalid CID bytes in tips CF: {}", e)))?;
            tips.push(cid);
        }
        
        DAG_TIP_COUNT.set(tips.len() as i64);
        Ok(tips)
    }

    async fn verify_branch(&self, tip: &Cid, _resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        let _timer = DAG_VERIFY_BRANCH_DURATION.start_timer(); // Start timing
        
        let _tip_clone = tip.clone();
        let _db_clone = self.db.clone();
        // Pass resolver appropriately, assuming it's Send + Sync
        // If resolver itself is not Send/Sync, need to handle differently (e.g., Arc<Mutex<...>>)
        // Commenting out spawn_blocking call as resolver handling is unclear
        /*
        let verification_result = tokio::task::spawn_blocking(move || {
            // ... blocking verification logic using resolver ...
        }).await;
        */
        // Using placeholder result until spawn_blocking is fixed
        let verification_result: Result<Result<(), DagError>, tokio::task::JoinError> = Ok(Ok(()));

        // Match on the outer JoinError first, then the inner verification Result
        match verification_result {
             Ok(Ok(())) => Ok(()), // Inner Ok(()) means success
             Ok(Err(e @ DagError::CidMismatch(_)))
             | Ok(Err(e @ DagError::InvalidSignature(_)))
             | Ok(Err(e @ DagError::MissingParent(_)))
             | Ok(Err(e @ DagError::PublicKeyResolutionError(_, _))) => {
                 DAG_NODE_VERIFICATION_FAILURES.inc();
                 Err(e)
             }
             Ok(Err(e)) => Err(e), // Other DagErrors (Storage, Serialization)
             Err(join_err) => Err(DagError::JoinError(join_err)), // JoinError
         }
    }

    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        let db_clone = self.db.clone();
        let cf_name_nodes: &'static str = CF_NODES;
        // let cf_name_children: &'static str = CF_CHILDREN; // Alternative: Could potentially use CF_CHILDREN

        // Spawn blocking task to read all nodes and build graph structure
        let (nodes_map, adj_list, mut in_degree) = tokio::task::spawn_blocking(move || {
            let mut nodes = HashMap::<Cid, SignedDagNode>::new();
            let mut adj = HashMap::<Cid, Vec<Cid>>::new(); // parent -> children
            let mut in_deg = HashMap::<Cid, usize>::new(); // child -> parent_count
            let mut all_node_cids = HashSet::<Cid>::new(); // Keep track of all nodes encountered

            let cf_handle_nodes = db_clone.cf_handle(cf_name_nodes)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_nodes)))?; 

            let iter = db_clone.iterator_cf(cf_handle_nodes, rocksdb::IteratorMode::Start);
            for result in iter {
                let (_key, value) = result.map_err(DagError::RocksDbError)?;
                let mut signed_node = Self::deserialize_node(&value)?;
                let node_cid = signed_node.ensure_cid()?; // Ensure CID is present
                
                all_node_cids.insert(node_cid.clone());
                nodes.insert(node_cid.clone(), signed_node.clone());

                // Initialize in-degree for this node (might be overwritten later if it's a child)
                in_deg.entry(node_cid.clone()).or_insert(0);

                // Process parents to build adjacency list and in-degrees
                for parent_cid in &signed_node.node.parents {
                    // Add edge from parent to current node (child)
                    adj.entry(parent_cid.clone()).or_default().push(node_cid.clone());
                    // Increment in-degree of the current node (child)
                    *in_deg.entry(node_cid.clone()).or_insert(0) += 1;
                    // Ensure parent is also in the in-degree map (even if it has 0 in-degree itself initially)
                    in_deg.entry(parent_cid.clone()).or_insert(0);
                }
            }
            
            // Ensure all nodes are in the in_degree map, even roots discovered only as parents
            for parent_cid in adj.keys() {
                 in_deg.entry(parent_cid.clone()).or_insert(0);
            }

            Ok::<_, DagError>((nodes, adj, in_deg))
        }).await.map_err(DagError::from)??; // Map JoinError and inner Result

        // --- Kahn's Algorithm for Topological Sort ---
        let mut sorted_list = Vec::with_capacity(nodes_map.len());
        let mut queue = VecDeque::new();

        // Initialize queue with nodes having in-degree 0
        for (cid, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(cid.clone());
            }
        }
        
        // If the DAG is empty
        if nodes_map.is_empty() {
            return Ok(sorted_list); // Return empty list
        }
        
        // If no nodes have in-degree 0 in a non-empty DAG, it implies a cycle or isolated nodes
        // However, our iteration logic should cover all nodes. If queue is empty here and nodes_map isn't, 
        // something is wrong, likely a cycle involving all nodes.
        if queue.is_empty() && !nodes_map.is_empty() {
             return Err(DagError::StorageError("Cycle detected in DAG or no root nodes found".to_string()));
        }

        while let Some(cid) = queue.pop_front() {
            if let Some(node) = nodes_map.get(&cid) {
                 sorted_list.push(node.clone());
            } else {
                 // Should not happen if maps were built correctly
                 return Err(DagError::StorageError(format!("Node {} found in queue but not in map", cid)));
            }

            // For each neighbor (child) of the current node
            if let Some(children) = adj_list.get(&cid) {
                for child_cid in children {
                    if let Some(degree) = in_degree.get_mut(child_cid) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child_cid.clone());
                        }
                    } else {
                         // Should not happen if maps were built correctly
                         return Err(DagError::StorageError(format!("Child node {} of {} not found in in-degree map", child_cid, cid)));
                    }
                }
            }
        }

        // Check for cycles: if sorted list size is less than total nodes, a cycle exists
        if sorted_list.len() != nodes_map.len() {
            Err(DagError::StorageError(format!(
                "Cycle detected in DAG. Processed {} nodes, expected {}.",
                sorted_list.len(),
                nodes_map.len()
            )))
        } else {
            Ok(sorted_list)
        }
    }

    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let db_clone = self.db.clone();
        let author_key = author.to_string().into_bytes();
        let cf_name_authors: &'static str = CF_AUTHORS;
        let cf_name_nodes: &'static str = CF_NODES;
        
        tokio::task::spawn_blocking(move || {
            let cf_authors = db_clone.cf_handle(cf_name_authors)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_authors)))?;
            let cf_nodes = db_clone.cf_handle(cf_name_nodes)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_nodes)))?;

            // 1. Get the list of node CID bytes from the author index
            let node_cid_bytes_list: Vec<Vec<u8>> = match db_clone.get_cf(cf_authors, &author_key)? {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => return Ok(Vec::new()), // No nodes found for this author
            };

            // 2. Retrieve and deserialize each node from the main nodes CF
            let mut nodes = Vec::with_capacity(node_cid_bytes_list.len());
            for node_cid_bytes in node_cid_bytes_list {
                match db_clone.get_cf(cf_nodes, &node_cid_bytes)? {
                    Some(node_bytes) => {
                        let node = Self::deserialize_node(&node_bytes)?;
                        nodes.push(node);
                    }
                    None => {
                        // This indicates an inconsistency between the index and the nodes table
                        let cid_str = Cid::from_bytes(&node_cid_bytes).map(|c| c.to_string()).unwrap_or_else(|_| hex::encode(&node_cid_bytes));
                        return Err(DagError::StorageError(format!(
                            "Author index points to non-existent node CID: {}", cid_str
                        )));
                    }
                }
            }

            Ok(nodes)
        }).await.map_err(DagError::from)? // Propagate JoinError
    }

    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let db_clone = self.db.clone();
        let payload_key = payload_type.as_bytes().to_vec();
        let cf_name_payload_types: &'static str = CF_PAYLOAD_TYPES;
        let cf_name_nodes: &'static str = CF_NODES;
        
        tokio::task::spawn_blocking(move || {
            let cf_payload_types = db_clone.cf_handle(cf_name_payload_types)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_payload_types)))?;
            let cf_nodes = db_clone.cf_handle(cf_name_nodes)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_nodes)))?;

            // 1. Get the list of node CID bytes from the payload type index
            let node_cid_bytes_list: Vec<Vec<u8>> = match db_clone.get_cf(cf_payload_types, &payload_key)? {
                Some(bytes) => Self::deserialize_cid_list(&bytes)?,
                None => return Ok(Vec::new()), // No nodes found for this payload type
            };

            // 2. Retrieve and deserialize each node from the main nodes CF
            let mut nodes = Vec::with_capacity(node_cid_bytes_list.len());
            for node_cid_bytes in node_cid_bytes_list {
                match db_clone.get_cf(cf_nodes, &node_cid_bytes)? {
                    Some(node_bytes) => {
                        let node = Self::deserialize_node(&node_bytes)?;
                        nodes.push(node);
                    }
                    None => {
                        // This indicates an inconsistency between the index and the nodes table
                        let cid_str = Cid::from_bytes(&node_cid_bytes).map(|c| c.to_string()).unwrap_or_else(|_| hex::encode(&node_cid_bytes));
                        return Err(DagError::StorageError(format!(
                            "Payload type index points to non-existent node CID: {}", cid_str
                        )));
                    }
                }
            }

            Ok(nodes)
        }).await.map_err(DagError::from)? // Propagate JoinError
    }

    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let db_clone = self.db.clone();
        let from_cid = from.clone();
        let to_cid = to.clone();
        let cf_name_nodes: &'static str = CF_NODES;

        tokio::task::spawn_blocking(move || {
            let cf_nodes = db_clone.cf_handle(cf_name_nodes)
                .ok_or_else(|| DagError::StorageError(format!("CF not found: {}", cf_name_nodes)))?;

            let mut queue = VecDeque::new();
            let mut visited = HashSet::new();
            // Stores child -> immediate parent relationship discovered during BFS from 'from'
            let mut predecessors = HashMap::new(); 

            queue.push_back(from_cid.clone());
            visited.insert(from_cid.clone());
            let mut target_found = false;

            while let Some(current_cid) = queue.pop_front() {
                if current_cid == to_cid {
                    target_found = true;
                    break; // Found the target node
                }

                // Get the current node's data to find its parents
                let node_bytes = db_clone.get_cf(cf_nodes, &Self::cid_to_key(&current_cid))?
                    .ok_or_else(|| DagError::NodeNotFound(current_cid.clone()))?; 
                let signed_node = Self::deserialize_node(&node_bytes)?;

                for parent_cid in &signed_node.node.parents {
                    if visited.insert(parent_cid.clone()) { // Returns true if value was not present
                        predecessors.insert(parent_cid.clone(), current_cid.clone());
                        queue.push_back(parent_cid.clone());
                    }
                }
            }

            if !target_found {
                return Ok(Vec::new()); // No path found
            }

            // --- Reconstruct path from 'to' back to 'from' using predecessors --- 
            let mut path_cids = VecDeque::new();
            let mut current = to_cid.clone();
            while current != from_cid {
                path_cids.push_front(current.clone());
                match predecessors.get(&current) {
                    Some(pred) => current = pred.clone(),
                    None => {
                        // Should not happen if target_found is true and graph is consistent
                        return Err(DagError::StorageError("Path reconstruction failed: predecessor not found".to_string()));
                    }
                }
            }
            path_cids.push_front(from_cid.clone()); // Add the starting node

            // --- Retrieve actual nodes for the path --- 
            let mut path_nodes = Vec::with_capacity(path_cids.len());
            for cid in path_cids {
                 let node_bytes = db_clone.get_cf(cf_nodes, &Self::cid_to_key(&cid))?
                    .ok_or_else(|| DagError::NodeNotFound(cid.clone()))?; // Path points to missing node
                 let signed_node = Self::deserialize_node(&node_bytes)?;
                 path_nodes.push(signed_node);
            }
            
            Ok(path_nodes)

        }).await.map_err(DagError::from)? // Propagate JoinError
    }
} 