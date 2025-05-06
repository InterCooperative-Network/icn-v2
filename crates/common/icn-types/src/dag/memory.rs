use crate::Cid;
use crate::dag::{DagError, DagStore, SignedDagNode, PublicKeyResolver};
use crate::Did;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

/// An in-memory implementation of the DagStore trait for testing
#[derive(Debug, Clone)]
pub struct MemoryDagStore {
    /// Map of CID -> SignedDagNode
    nodes: Arc<RwLock<HashMap<String, SignedDagNode>>>,
    /// Set of tip nodes (nodes with no children)
    tips: Arc<RwLock<HashSet<String>>>,
    /// Map of parent CID -> Set of child CIDs
    children: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Map of author DID -> Set of node CIDs
    author_nodes: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Map of payload type -> Set of node CIDs
    payload_types: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl MemoryDagStore {
    /// Create a new in-memory DAG store
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            tips: Arc::new(RwLock::new(HashSet::new())),
            children: Arc::new(RwLock::new(HashMap::new())),
            author_nodes: Arc::new(RwLock::new(HashMap::new())),
            payload_types: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Convert a CID to a string key
    fn cid_to_key(cid: &Cid) -> String {
        cid.to_string()
    }
    
    /// Get the payload type as a string
    fn get_payload_type(node: &SignedDagNode) -> String {
        match &node.node.payload {
            crate::dag::DagPayload::Raw(_) => "raw".to_string(),
            crate::dag::DagPayload::Json(_) => "json".to_string(),
            crate::dag::DagPayload::Reference(_) => "reference".to_string(),
            crate::dag::DagPayload::TrustBundle(_) => "trustbundle".to_string(),
            crate::dag::DagPayload::ExecutionReceipt(_) => "receipt".to_string(),
        }
    }
}

// Asynchronous implementation ONLY
#[cfg(feature = "async")]
#[async_trait]
impl DagStore for MemoryDagStore {
    async fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        // Ensure the node has a CID
        let cid = node.ensure_cid()?;
        let cid_key = Self::cid_to_key(&cid);
        
        // Acquire write locks asynchronously
        let mut nodes = self.nodes.write().await;
        let mut tips = self.tips.write().await;
        let mut children = self.children.write().await;
        let mut author_nodes = self.author_nodes.write().await;
        let mut payload_types = self.payload_types.write().await;
        
        // Check if the node already exists
        if nodes.contains_key(&cid_key) {
            return Ok(cid.clone());
        }
        
        // Validate parent references (must hold nodes lock)
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            if !nodes.contains_key(&parent_key) {
                // Drop locks before returning error to avoid deadlock potential if caller retries
                drop(nodes);
                drop(tips);
                drop(children);
                drop(author_nodes);
                drop(payload_types);
                return Err(DagError::ParentNotFound { child: cid.clone(), parent: parent_cid.clone() });
            }
        }
        
        // Store the node
        nodes.insert(cid_key.clone(), node.clone());
        
        // Update tips
        tips.insert(cid_key.clone());
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            tips.remove(&parent_key);
            
            // Update children map
            children
                .entry(parent_key)
                .or_insert_with(HashSet::new)
                .insert(cid_key.clone());
        }
        
        // Update author_nodes
        let author_key = node.node.author.to_string();
        author_nodes
            .entry(author_key)
            .or_insert_with(HashSet::new)
            .insert(cid_key.clone());
        
        // Update payload_types
        let payload_type = Self::get_payload_type(&node);
        payload_types
            .entry(payload_type)
            .or_insert_with(HashSet::new)
            .insert(cid_key);
            
        // Locks are dropped automatically when guards go out of scope
        Ok(cid)
    }

    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let cid_key = Self::cid_to_key(cid);
        // Acquire read lock asynchronously
        let nodes = self.nodes.read().await;
        
        nodes.get(&cid_key)
            .cloned()
            .ok_or_else(|| DagError::NodeNotFound(cid.clone()))
    }

    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        // Acquire read lock asynchronously
        let tips = self.tips.read().await;
        
        // Attempt to convert keys back to Cids
        let result: Result<Vec<Cid>, _> = tips.iter()
            .map(|key| Cid::from_bytes(key.as_bytes())) // Use from_bytes
            .collect();
            
        result.map_err(|e| DagError::CidError(format!("Failed to parse CID from key: {}", e)))
    }

    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        // 1. Clone data under lock
        let (nodes_clone, children_clone) = {
             let nodes_guard = self.nodes.read().await;
             let children_guard = self.children.read().await;
            (nodes_guard.clone(), children_guard.clone())
        }; // Locks are dropped here

        // 2. Build graph structure from cloned data (no locks needed)
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node_key in nodes_clone.keys() {
            in_degree.insert(node_key.clone(), 0);
        }

        // Use the cloned children map to build adj_list and update in_degree
        let mut adj_list: HashMap<String, Vec<String>> = HashMap::new(); 
        for (parent_key, child_set) in &children_clone {
             let children_vec: Vec<String> = child_set.iter().cloned().collect();
             adj_list.insert(parent_key.clone(), children_vec.clone());
             for child_key in child_set {
                 if let Some(count) = in_degree.get_mut(child_key) {
                     *count += 1;
                 } else {
                     let cid_from_key = Cid::from_bytes(child_key.as_bytes()).map(|c| c.to_string()).unwrap_or_else(|_| child_key.clone());
                     return Err(DagError::StorageError(format!(
                        "Inconsistent state: Child {} found but not present in nodes map.", cid_from_key
                    )));
                 }
             }
        }

        // 3. Perform Kahn's Algorithm (no locks needed)
        let mut sorted_list = Vec::with_capacity(nodes_clone.len());
        let mut queue: VecDeque<String> = in_degree.iter()
            .filter_map(|(key, count)| if *count == 0 { Some(key.clone()) } else { None })
            .collect();

        if nodes_clone.is_empty() {
            return Ok(sorted_list);
        }

        if queue.is_empty() && !nodes_clone.is_empty() {
            return Err(DagError::StorageError("Cycle detected in DAG or no root nodes found".to_string()));
        }

        while let Some(cid_key) = queue.pop_front() {
            if let Some(node) = nodes_clone.get(&cid_key) {
                sorted_list.push(node.clone());
            } else {
                 return Err(DagError::StorageError(format!("Node key {} found in queue but not in cloned map", cid_key)));
            }

            if let Some(children_keys) = adj_list.get(&cid_key) {
                for child_key in children_keys {
                    if let Some(degree) = in_degree.get_mut(child_key) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child_key.clone());
                        }
                    } else {
                         return Err(DagError::StorageError(format!("Child key {} of {} not found in in-degree map", child_key, cid_key)));
                    }
                }
            }
        }

        // 4. Check for cycles and return result
        if sorted_list.len() != nodes_clone.len() {
            Err(DagError::StorageError(format!(
                "Cycle detected in DAG. Processed {} nodes, expected {}.",
                sorted_list.len(),
                nodes_clone.len()
            )))
        } else {
            Ok(sorted_list)
        }
    }

    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let author_key = author.to_string();
        
        let cid_keys_to_fetch: Option<HashSet<String>> = {
            let author_nodes_guard = self.author_nodes.read().await;
            author_nodes_guard.get(&author_key).cloned()
        };

        match cid_keys_to_fetch {
            Some(cid_keys) => {
                let nodes_guard = self.nodes.read().await;
                let result = cid_keys.iter()
                    .filter_map(|key| nodes_guard.get(key).cloned())
                    .collect();
                Ok(result)
            }
            None => Ok(Vec::new()),
        }
    }

    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let cid_keys_to_fetch: Option<HashSet<String>> = {
            let payload_types_guard = self.payload_types.read().await;
            payload_types_guard.get(payload_type).cloned()
        };

        match cid_keys_to_fetch {
            Some(cid_keys) => {
                let nodes_guard = self.nodes.read().await;
                let result = cid_keys.iter()
                    .filter_map(|key| nodes_guard.get(key).cloned())
                    .collect();
                Ok(result)
            }
            None => Ok(Vec::new()),
        }
    }

    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let from_key = Self::cid_to_key(from);
        let to_key = Self::cid_to_key(to);
        
        let nodes_clone = {
            let nodes_guard = self.nodes.read().await;
            if !nodes_guard.contains_key(&from_key) {
                return Err(DagError::NodeNotFound(from.clone()));
            }
            if !nodes_guard.contains_key(&to_key) {
                 return Err(DagError::NodeNotFound(to.clone()));
            }
            if from_key == to_key {
                return Ok(vec![nodes_guard.get(&from_key).unwrap().clone()]); 
            }
            nodes_guard.clone()
        };

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut predecessors = HashMap::new();

        queue.push_back(from_key.clone());
        visited.insert(from_key.clone());
        let mut target_found = false;

        while let Some(current_key) = queue.pop_front() {
            if current_key == to_key {
                target_found = true;
                break;
            }

            if let Some(signed_node) = nodes_clone.get(&current_key) {
                for parent_cid in &signed_node.node.parents {
                    let parent_key = Self::cid_to_key(parent_cid);
                    if nodes_clone.contains_key(&parent_key) && visited.insert(parent_key.clone()) {
                        predecessors.insert(parent_key.clone(), current_key.clone());
                        queue.push_back(parent_key.clone());
                    }
                }
            }
        }

        if !target_found {
            return Ok(Vec::new());
        }

        let mut path_nodes = VecDeque::new();
        let mut current_key = to_key.clone();
        while current_key != from_key {
             if let Some(node) = nodes_clone.get(&current_key) {
                 path_nodes.push_front(node.clone());
             } else {
                 return Err(DagError::StorageError(format!("Path reconstruction failed: node {} not found in cloned map", current_key)));
             }
            
            match predecessors.get(&current_key) {
                Some(pred_key) => current_key = pred_key.clone(),
                None => {
                    return Err(DagError::StorageError("Path reconstruction failed: predecessor not found".to_string()));
                }
            }
        }
        if let Some(start_node) = nodes_clone.get(&from_key) {
             path_nodes.push_front(start_node.clone());
        } else {
             return Err(DagError::StorageError(format!("Path reconstruction failed: start node {} not found", from_key)));
        }

        Ok(path_nodes.into())
    }

    async fn verify_branch(&self, tip: &Cid, resolver: &(dyn PublicKeyResolver + Send + Sync)) -> Result<(), DagError> {
        // TODO: Implement async verification logic. This will likely involve cloning node data under lock,
        // then performing the traversal and signature checks outside the lock.
        // Need to be careful about how the resolver is used (can it be called concurrently?).
        let _tip = tip; // Mark as used
        let _resolver = resolver; // Mark as used
        println!("Warning: MemoryDagStore::verify_branch is not fully implemented.");
        Ok(()) // Assume valid for now
    }

    #[cfg(feature = "async")]
    async fn get_data(&self, _cid: &Cid) -> Result<Option<Vec<u8>>, DagError> {
        // Placeholder implementation
        // A real implementation would look up raw block bytes if stored separately,
        // or get the node and serialize its payload if that's how data is stored.
        unimplemented!("get_data not yet implemented for MemoryDagStore")
    }

    #[cfg(not(feature = "async"))]
    fn get_data(&self, _cid: &Cid) -> Result<Option<Vec<u8>>, DagError> {
        unimplemented!("get_data not yet implemented for MemoryDagStore")
    }
} 