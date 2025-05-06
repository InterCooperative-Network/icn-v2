#![cfg(feature = "persistence")]

use crate::cid::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode};
use crate::identity::Did;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock};

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

    /// Serialize a DAG node to bytes
    fn serialize_node(node: &SignedDagNode) -> Result<Vec<u8>, DagError> {
        serde_json::to_vec(node).map_err(DagError::SerializationError)
    }

    /// Deserialize a DAG node from bytes
    fn deserialize_node(bytes: &[u8]) -> Result<SignedDagNode, DagError> {
        serde_json::from_slice(bytes).map_err(DagError::SerializationError)
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
                Some(bytes) => serde_json::from_slice(&bytes)
                    .map_err(|e| DagError::StorageError(format!("Failed to deserialize children: {}", e)))?,
                None => Vec::new(),
            };
            
            // Add new child
            children.push(node_key.clone());
            
            // Store updated children list
            let serialized = serde_json::to_vec(&children)
                .map_err(|e| DagError::StorageError(format!("Failed to serialize children: {}", e)))?;
                
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
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| DagError::StorageError(format!("Failed to deserialize author nodes: {}", e)))?,
            None => Vec::new(),
        };
        
        // Add new node
        author_nodes.push(node_key);
        
        // Store updated author nodes list
        let serialized = serde_json::to_vec(&author_nodes)
            .map_err(|e| DagError::StorageError(format!("Failed to serialize author nodes: {}", e)))?;
            
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
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| DagError::StorageError(format!("Failed to deserialize payload type nodes: {}", e)))?,
            None => Vec::new(),
        };
        
        // Add new node
        payload_nodes.push(node_key);
        
        // Store updated payload type nodes list
        let serialized = serde_json::to_vec(&payload_nodes)
            .map_err(|e| DagError::StorageError(format!("Failed to serialize payload type nodes: {}", e)))?;
            
        self.db.put_cf(cf_payload_types, payload_key, &serialized)
            .map_err(|e| DagError::StorageError(format!("Failed to update payload type nodes: {}", e)))?;

        Ok(())
    }
}

impl DagStore for RocksDbDagStore {
    fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        // Ensure the node has a CID
        let cid = node.ensure_cid()?;
        let node_key = Self::cid_to_key(&cid);
        
        // Check if the node already exists
        let cf_nodes = self.cf_handle(CF_NODES)?;
        if self.db.get_cf(cf_nodes, &node_key)?.is_some() {
            return Ok(cid); // Node already exists, return the CID
        }
        
        // Validate parent references
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            if self.db.get_cf(cf_nodes, &parent_key)?.is_none() {
                return Err(DagError::InvalidParentRefs);
            }
        }

        // Serialize and store the node
        let node_data = Self::serialize_node(&node)?;
        self.db
            .put_cf(cf_nodes, &node_key, &node_data)
            .map_err(|e| DagError::StorageError(format!("Failed to store node: {}", e)))?;
        
        // Update indexes
        self.update_tips(&node)?;
        self.update_children(&node)?;
        self.update_authors(&node)?;
        self.update_payload_types(&node)?;
        
        Ok(cid)
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
        
        Ok(tips)
    }

    fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        // Implement topological sort
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let cf_children = self.cf_handle(CF_CHILDREN)?;
        
        // Step 1: Build a graph representation and count incoming edges
        let mut nodes: HashMap<Vec<u8>, SignedDagNode> = HashMap::new();
        let mut incoming_count: HashMap<Vec<u8>, usize> = HashMap::new();
        
        // Load all nodes and count incoming edges
        let iter = self.db.iterator_cf(cf_nodes, rocksdb::IteratorMode::Start);
        for result in iter {
            let (key, value) = result
                .map_err(|e| DagError::StorageError(format!("Error iterating nodes: {}", e)))?;
                
            let node = Self::deserialize_node(&value)?;
            nodes.insert(key.to_vec(), node.clone());
            
            // Initialize incoming count for this node
            incoming_count.entry(key.to_vec()).or_insert(0);
            
            // Count incoming edges for each child
            for parent_cid in &node.node.parents {
                let parent_key = Self::cid_to_key(parent_cid);
                *incoming_count.entry(parent_key).or_insert(0) += 1;
            }
        }
        
        // Step 2: Find all nodes with no incoming edges (sources)
        let mut queue: VecDeque<Vec<u8>> = VecDeque::new();
        for (key, count) in &incoming_count {
            if *count == 0 {
                queue.push_back(key.clone());
            }
        }
        
        // Step 3: Perform topological sort
        let mut sorted_nodes = Vec::new();
        
        while let Some(current) = queue.pop_front() {
            // Add the current node to the sorted list
            if let Some(node) = nodes.get(&current) {
                sorted_nodes.push(node.clone());
            }
            
            // Get children of the current node
            if let Ok(Some(children_data)) = self.db.get_cf(cf_children, &current) {
                let children: Vec<Vec<u8>> = serde_json::from_slice(&children_data)
                    .map_err(|e| DagError::StorageError(format!("Failed to deserialize children: {}", e)))?;
                
                // Decrease incoming count for each child and add to queue if no more incoming edges
                for child in children {
                    if let Some(count) = incoming_count.get_mut(&child) {
                        *count -= 1;
                        if *count == 0 {
                            queue.push_back(child);
                        }
                    }
                }
            }
        }
        
        // Check if we have a cycle (not all nodes were visited)
        if sorted_nodes.len() != nodes.len() {
            return Err(DagError::InvalidNodeData("DAG contains cycles".to_string()));
        }
        
        Ok(sorted_nodes)
    }

    fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let cf_authors = self.cf_handle(CF_AUTHORS)?;
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let author_key = author.to_string().into_bytes();
        
        match self.db.get_cf(cf_authors, &author_key)? {
            Some(data) => {
                let node_keys: Vec<Vec<u8>> = serde_json::from_slice(&data)
                    .map_err(|e| DagError::StorageError(format!("Failed to deserialize author nodes: {}", e)))?;
                
                let mut nodes = Vec::new();
                for key in node_keys {
                    if let Some(node_data) = self.db.get_cf(cf_nodes, &key)? {
                        let node = Self::deserialize_node(&node_data)?;
                        nodes.push(node);
                    }
                }
                
                Ok(nodes)
            },
            None => Ok(Vec::new()),
        }
    }

    fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let cf_payload_types = self.cf_handle(CF_PAYLOAD_TYPES)?;
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let payload_key = payload_type.as_bytes();
        
        match self.db.get_cf(cf_payload_types, payload_key)? {
            Some(data) => {
                let node_keys: Vec<Vec<u8>> = serde_json::from_slice(&data)
                    .map_err(|e| DagError::StorageError(format!("Failed to deserialize payload type nodes: {}", e)))?;
                
                let mut nodes = Vec::new();
                for key in node_keys {
                    if let Some(node_data) = self.db.get_cf(cf_nodes, &key)? {
                        let node = Self::deserialize_node(&node_data)?;
                        nodes.push(node);
                    }
                }
                
                Ok(nodes)
            },
            None => Ok(Vec::new()),
        }
    }

    fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        // Perform a BFS search to find a path between two nodes
        let cf_nodes = self.cf_handle(CF_NODES)?;
        let cf_children = self.cf_handle(CF_CHILDREN)?;
        
        // Check if the nodes exist
        let from_key = Self::cid_to_key(from);
        let to_key = Self::cid_to_key(to);
        
        if self.db.get_cf(cf_nodes, &from_key)?.is_none() {
            return Err(DagError::NodeNotFound(from.clone()));
        }
        
        if self.db.get_cf(cf_nodes, &to_key)?.is_none() {
            return Err(DagError::NodeNotFound(to.clone()));
        }
        
        // Special case: from and to are the same
        if from == to {
            let node_data = self.db.get_cf(cf_nodes, &from_key)?.unwrap();
            let node = Self::deserialize_node(&node_data)?;
            return Ok(vec![node]);
        }
        
        // BFS search
        let mut queue = VecDeque::new();
        queue.push_back(from_key.clone());
        
        // Keep track of visited nodes and their predecessors
        let mut visited = HashSet::new();
        let mut predecessors: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        
        visited.insert(from_key.clone());
        
        while let Some(current_key) = queue.pop_front() {
            // Check if we've reached the target
            if current_key == to_key {
                break;
            }
            
            // Get children of the current node
            if let Ok(Some(children_data)) = self.db.get_cf(cf_children, &current_key) {
                let children: Vec<Vec<u8>> = serde_json::from_slice(&children_data)
                    .map_err(|e| DagError::StorageError(format!("Failed to deserialize children: {}", e)))?;
                
                for child in children {
                    if !visited.contains(&child) {
                        visited.insert(child.clone());
                        predecessors.insert(child.clone(), current_key.clone());
                        queue.push_back(child);
                    }
                }
            }
        }
        
        // Reconstruct the path if found
        if !predecessors.contains_key(&to_key) && from_key != to_key {
            return Ok(Vec::new()); // No path found
        }
        
        // Reconstruct the path
        let mut path = Vec::new();
        let mut current = to_key.clone();
        
        while current != from_key {
            let node_data = self.db.get_cf(cf_nodes, &current)?.unwrap();
            let node = Self::deserialize_node(&node_data)?;
            path.push(node);
            current = predecessors.get(&current).unwrap().clone();
        }
        
        // Add the starting node
        let node_data = self.db.get_cf(cf_nodes, &from_key)?.unwrap();
        let node = Self::deserialize_node(&node_data)?;
        path.push(node);
        
        // Reverse to get the path from start to end
        path.reverse();
        
        Ok(path)
    }

    fn verify_branch(&self, tip: &Cid) -> Result<bool, DagError> {
        // Verify a branch of the DAG, starting from the given tip
        // This ensures that all signatures are valid and parent references are correct
        
        let tip_key = Self::cid_to_key(tip);
        let cf_nodes = self.cf_handle(CF_NODES)?;
        
        if self.db.get_cf(cf_nodes, &tip_key)?.is_none() {
            return Err(DagError::NodeNotFound(tip.clone()));
        }
        
        // Perform a topological traversal starting from the tip
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        
        queue.push_back(tip_key.clone());
        visited.insert(tip_key);
        
        while let Some(current_key) = queue.pop_front() {
            // Get the current node
            let node_data = self.db.get_cf(cf_nodes, &current_key)?.unwrap();
            let node = Self::deserialize_node(&node_data)?;
            
            // TODO: Verify the signature of the node
            // This would require accessing the author's public key
            // For now, we just check that the CID is correctly calculated
            let calculated_cid = node.calculate_cid()?;
            if node.cid.as_ref().unwrap() != &calculated_cid {
                return Ok(false);
            }
            
            // Add parent nodes to the queue
            for parent_cid in &node.node.parents {
                let parent_key = Self::cid_to_key(parent_cid);
                
                // Check if the parent exists
                if self.db.get_cf(cf_nodes, &parent_key)?.is_none() {
                    return Ok(false); // Missing parent, branch is invalid
                }
                
                if !visited.contains(&parent_key) {
                    visited.insert(parent_key.clone());
                    queue.push_back(parent_key);
                }
            }
        }
        
        // All nodes in the branch are valid
        Ok(true)
    }
} 