use crate::cid::Cid;
use crate::dag::{DagError, DagNode, DagStore, SignedDagNode, PublicKeyResolver};
use crate::identity::Did;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use ed25519_dalek::VerifyingKey;

/// An in-memory implementation of the DagStore trait for testing
#[derive(Debug, Clone, Default)]
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

impl Default for MemoryDagStore {
    fn default() -> Self {
        Self::new()
    }
}

// Synchronous implementation
#[cfg(not(feature = "async"))]
impl DagStore for MemoryDagStore {
    fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        // Ensure the node has a CID
        let cid = node.ensure_cid()?;
        let cid_key = Self::cid_to_key(&cid);
        
        // Acquire write locks
        let mut nodes = self.nodes.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let mut tips = self.tips.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire tips lock: {}", e)))?;
        let mut children = self.children.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        let mut author_nodes = self.author_nodes.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire author_nodes lock: {}", e)))?;
        let mut payload_types = self.payload_types.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire payload_types lock: {}", e)))?;
        
        // Check if the node already exists
        if nodes.contains_key(&cid_key) {
            return Ok(cid);
        }
        
        // Validate parent references
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            if !nodes.contains_key(&parent_key) {
                return Err(DagError::InvalidParentRefs);
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
        
        Ok(cid)
    }

    fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let cid_key = Self::cid_to_key(cid);
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        nodes.get(&cid_key)
            .cloned()
            .ok_or_else(|| DagError::NodeNotFound(cid.clone()))
    }

    fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let tips = self.tips.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire tips lock: {}", e)))?;
        
        Ok(tips.iter()
            .filter_map(|key| Cid::from_bytes(key.as_bytes()).ok())
            .collect())
    }

    fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let children = self.children.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        
        // Count incoming edges for each node
        let mut incoming_count: HashMap<String, usize> = HashMap::new();
        for (node_key, _) in nodes.iter() {
            incoming_count.insert(node_key.clone(), 0);
        }
        
        for (_, child_set) in children.iter() {
            for child_key in child_set {
                if let Some(count) = incoming_count.get_mut(child_key) {
                    *count += 1;
                }
            }
        }
        
        // Find nodes with no incoming edges (sources)
        let mut queue: VecDeque<String> = incoming_count.iter()
            .filter_map(|(key, count)| if *count == 0 { Some(key.clone()) } else { None })
            .collect();
        
        // Perform topological sort
        let mut sorted_nodes = Vec::new();
        
        while let Some(current) = queue.pop_front() {
            // Add the current node to the sorted list
            if let Some(node) = nodes.get(&current) {
                sorted_nodes.push(node.clone());
            }
            
            // Process children of the current node
            if let Some(child_set) = children.get(&current) {
                for child_key in child_set {
                    if let Some(count) = incoming_count.get_mut(child_key) {
                        *count -= 1;
                        if *count == 0 {
                            queue.push_back(child_key.clone());
                        }
                    }
                }
            }
        }
        
        // Check for cycles
        if sorted_nodes.len() != nodes.len() {
            return Err(DagError::InvalidNodeData("DAG contains cycles".to_string()));
        }
        
        Ok(sorted_nodes)
    }

    fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let author_key = author.to_string();
        let author_nodes = self.author_nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire author_nodes lock: {}", e)))?;
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        let result = match author_nodes.get(&author_key) {
            Some(cids) => {
                cids.iter()
                    .filter_map(|cid_key| nodes.get(cid_key).cloned())
                    .collect()
            }
            None => Vec::new(),
        };
        
        Ok(result)
    }

    fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let payload_types = self.payload_types.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire payload_types lock: {}", e)))?;
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        let result = match payload_types.get(payload_type) {
            Some(cids) => {
                cids.iter()
                    .filter_map(|cid_key| nodes.get(cid_key).cloned())
                    .collect()
            }
            None => Vec::new(),
        };
        
        Ok(result)
    }

    fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let from_key = Self::cid_to_key(from);
        let to_key = Self::cid_to_key(to);
        
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let children = self.children.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        
        // Check if nodes exist
        if !nodes.contains_key(&from_key) {
            return Err(DagError::NodeNotFound(from.clone()));
        }
        if !nodes.contains_key(&to_key) {
            return Err(DagError::NodeNotFound(to.clone()));
        }
        
        // Special case: from and to are the same
        if from_key == to_key {
            return Ok(vec![nodes.get(&from_key).unwrap().clone()]);
        }
        
        // BFS search
        let mut queue = VecDeque::new();
        queue.push_back(from_key.clone());
        
        let mut visited = HashSet::new();
        let mut predecessors: HashMap<String, String> = HashMap::new();
        
        visited.insert(from_key.clone());
        
        while let Some(current_key) = queue.pop_front() {
            // Check if we've reached the target
            if current_key == to_key {
                break;
            }
            
            // Process children
            if let Some(child_set) = children.get(&current_key) {
                for child_key in child_set {
                    if !visited.contains(child_key) {
                        visited.insert(child_key.clone());
                        predecessors.insert(child_key.clone(), current_key.clone());
                        queue.push_back(child_key.clone());
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
            path.push(nodes.get(&current).unwrap().clone());
            current = predecessors.get(&current).unwrap().clone();
        }
        
        // Add the starting node
        path.push(nodes.get(&from_key).unwrap().clone());
        
        // Reverse to get the path from start to end
        path.reverse();
        
        Ok(path)
    }

    fn verify_branch(&self, tip: &Cid) -> Result<bool, DagError> {
        let tip_key = Self::cid_to_key(tip);
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        if !nodes.contains_key(&tip_key) {
            return Err(DagError::NodeNotFound(tip.clone()));
        }
        
        // Perform a topological traversal starting from the tip
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        
        queue.push_back(tip_key.clone());
        visited.insert(tip_key);
        
        while let Some(current_key) = queue.pop_front() {
            // Get the current node
            let node = nodes.get(&current_key).unwrap();
            
            // TODO: Verify signature of the node
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
                if !nodes.contains_key(&parent_key) {
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

// Asynchronous implementation
#[cfg(feature = "async")]
#[async_trait::async_trait]
impl DagStore for MemoryDagStore {
    async fn add_node(&mut self, mut node: SignedDagNode) -> Result<Cid, DagError> {
        // Ensure the node has a CID
        let cid = node.ensure_cid()?;
        let cid_key = Self::cid_to_key(&cid);
        
        // Acquire write locks
        let mut nodes = self.nodes.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let mut tips = self.tips.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire tips lock: {}", e)))?;
        let mut children = self.children.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        let mut author_nodes = self.author_nodes.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire author_nodes lock: {}", e)))?;
        let mut payload_types = self.payload_types.write().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire payload_types lock: {}", e)))?;
        
        // Check if the node already exists
        if nodes.contains_key(&cid_key) {
            return Ok(cid);
        }
        
        // Validate parent references
        for parent_cid in &node.node.parents {
            let parent_key = Self::cid_to_key(parent_cid);
            if !nodes.contains_key(&parent_key) {
                return Err(DagError::InvalidParentRefs);
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
        
        Ok(cid)
    }

    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        let cid_key = Self::cid_to_key(cid);
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        nodes.get(&cid_key)
            .cloned()
            .ok_or_else(|| DagError::NodeNotFound(cid.clone()))
    }

    async fn get_tips(&self) -> Result<Vec<Cid>, DagError> {
        let tips = self.tips.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire tips lock: {}", e)))?;
        
        Ok(tips.iter()
            .filter_map(|key| Cid::from_bytes(key.as_bytes()).ok())
            .collect())
    }

    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> {
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let children = self.children.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        
        // Count incoming edges for each node
        let mut incoming_count: HashMap<String, usize> = HashMap::new();
        for (node_key, _) in nodes.iter() {
            incoming_count.insert(node_key.clone(), 0);
        }
        
        for (_, child_set) in children.iter() {
            for child_key in child_set {
                if let Some(count) = incoming_count.get_mut(child_key) {
                    *count += 1;
                }
            }
        }
        
        // Find nodes with no incoming edges (sources)
        let mut queue: VecDeque<String> = incoming_count.iter()
            .filter_map(|(key, count)| if *count == 0 { Some(key.clone()) } else { None })
            .collect();
        
        // Perform topological sort
        let mut sorted_nodes = Vec::new();
        
        while let Some(current) = queue.pop_front() {
            // Add the current node to the sorted list
            if let Some(node) = nodes.get(&current) {
                sorted_nodes.push(node.clone());
            }
            
            // Process children of the current node
            if let Some(child_set) = children.get(&current) {
                for child_key in child_set {
                    if let Some(count) = incoming_count.get_mut(child_key) {
                        *count -= 1;
                        if *count == 0 {
                            queue.push_back(child_key.clone());
                        }
                    }
                }
            }
        }
        
        // Check for cycles
        if sorted_nodes.len() != nodes.len() {
            return Err(DagError::InvalidNodeData("DAG contains cycles".to_string()));
        }
        
        Ok(sorted_nodes)
    }

    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, DagError> {
        let author_key = author.to_string();
        let author_nodes = self.author_nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire author_nodes lock: {}", e)))?;
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        let result = match author_nodes.get(&author_key) {
            Some(cids) => {
                cids.iter()
                    .filter_map(|cid_key| nodes.get(cid_key).cloned())
                    .collect()
            }
            None => Vec::new(),
        };
        
        Ok(result)
    }

    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> {
        let payload_types = self.payload_types.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire payload_types lock: {}", e)))?;
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        
        let result = match payload_types.get(payload_type) {
            Some(cids) => {
                cids.iter()
                    .filter_map(|cid_key| nodes.get(cid_key).cloned())
                    .collect()
            }
            None => Vec::new(),
        };
        
        Ok(result)
    }

    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, DagError> {
        let from_key = Self::cid_to_key(from);
        let to_key = Self::cid_to_key(to);
        
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire nodes lock: {}", e)))?;
        let children = self.children.read().map_err(|e| 
            DagError::StorageError(format!("Failed to acquire children lock: {}", e)))?;
        
        // Check if nodes exist
        if !nodes.contains_key(&from_key) {
            return Err(DagError::NodeNotFound(from.clone()));
        }
        if !nodes.contains_key(&to_key) {
            return Err(DagError::NodeNotFound(to.clone()));
        }
        
        // Special case: from and to are the same
        if from_key == to_key {
            return Ok(vec![nodes.get(&from_key).unwrap().clone()]);
        }
        
        // BFS search
        let mut queue = VecDeque::new();
        queue.push_back(from_key.clone());
        
        let mut visited = HashSet::new();
        let mut predecessors: HashMap<String, String> = HashMap::new();
        
        visited.insert(from_key.clone());
        
        while let Some(current_key) = queue.pop_front() {
            // Check if we've reached the target
            if current_key == to_key {
                break;
            }
            
            // Process children
            if let Some(child_set) = children.get(&current_key) {
                for child_key in child_set {
                    if !visited.contains(child_key) {
                        visited.insert(child_key.clone());
                        predecessors.insert(child_key.clone(), current_key.clone());
                        queue.push_back(child_key.clone());
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
            path.push(nodes.get(&current).unwrap().clone());
            current = predecessors.get(&current).unwrap().clone();
        }
        
        // Add the starting node
        path.push(nodes.get(&from_key).unwrap().clone());
        
        // Reverse to get the path from start to end
        path.reverse();
        
        Ok(path)
    }

    #[tracing::instrument(skip(self, resolver))]
    async fn verify_branch(&self, tip: &Cid, resolver: &dyn PublicKeyResolver) -> Result<(), DagError> {
        let tip_key = Self::cid_to_key(tip);
        let nodes = self.nodes.read().map_err(|e| 
            DagError::StorageError(format!("Failed to lock nodes for read: {}", e))
        )?;

        if !nodes.contains_key(&tip_key) {
            return Err(DagError::NodeNotFound(tip.clone()));
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(tip_key.clone());

        while let Some(current_key) = queue.pop_front() {
            if !visited.insert(current_key.clone()) {
                continue; // Already visited
            }

            let node = nodes.get(&current_key)
                .ok_or_else(|| DagError::NodeNotFound(Self::key_to_cid(&current_key).unwrap()))?;
            
            // *** Verify signature ***
            let author_did = &node.node.author;
            let verifying_key = resolver.resolve_public_key(author_did).await?;
            node.verify_signature(&verifying_key)?;

            for parent_cid in &node.node.parents {
                let parent_key = Self::cid_to_key(parent_cid);
                if !nodes.contains_key(&parent_key) {
                    return Err(DagError::ParentNotFound { 
                        child: Self::key_to_cid(&current_key).unwrap(), 
                        parent: parent_cid.clone() 
                    });
                }
                if !visited.contains(&parent_key) {
                    queue.push_back(parent_key);
                }
            }
        }

        Ok(()) // Return Ok(()) instead of Ok(true)
    }
} 