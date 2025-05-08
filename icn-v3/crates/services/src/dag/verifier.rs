use crate::error::ServiceError;
use crate::dag::storage::{DagStorage, DagMetadata};
use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
use icn_common::identity::{ScopedIdentity, Credential};
use icn_common::verification::Verifiable;

use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, trace, warn};

/// Error types for lineage verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineageVerificationError {
    /// Node has invalid signature
    InvalidSignature(String),
    
    /// Node has invalid parent reference
    InvalidParent(String),
    
    /// Node violates scope rules
    ScopeViolation(String),
    
    /// Node has invalid or unauthorized creator
    UnauthorizedCreator(String),
    
    /// Node has missing required fields
    MissingField(String),
    
    /// Node has inconsistent data
    InconsistentData(String),
    
    /// Node has invalid credential
    InvalidCredential(String),
}

/// Result of lineage verification for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageVerificationResult {
    /// Node ID
    pub node_id: DAGNodeID,
    
    /// Whether verification was successful
    pub success: bool,
    
    /// Error if verification failed
    pub error: Option<LineageVerificationError>,
    
    /// Depth in the DAG (0 for roots)
    pub depth: usize,
    
    /// Scope of the node
    pub scope: String,
    
    /// Type of the node
    pub node_type: DAGNodeType,
    
    /// Creator of the node
    pub creator: ScopedIdentity,
    
    /// Timestamp of the node
    pub timestamp: u64,
}

/// Trait for DAG replay verification
#[async_trait]
pub trait DagReplayVerifier: Send + Sync + 'static {
    /// Verify the lineage of a specific node
    async fn verify_node_lineage(&self, node_id: &DAGNodeID) -> Result<LineageVerificationResult, ServiceError>;
    
    /// Verify the entire DAG
    async fn verify_dag(&self) -> Result<Vec<LineageVerificationResult>, ServiceError>;
    
    /// Verify a scope branch of the DAG
    async fn verify_scope(&self, scope: &str) -> Result<Vec<LineageVerificationResult>, ServiceError>;
}

/// Default implementation of DAG replay verifier
pub struct DefaultDagReplayVerifier {
    dag_storage: Arc<DagStorage>,
    /// Registry of known scopes and their authorities
    scope_registry: HashMap<String, HashSet<String>>,
}

impl DefaultDagReplayVerifier {
    /// Create a new DAG replay verifier
    pub fn new(dag_storage: Arc<DagStorage>) -> Self {
        Self {
            dag_storage,
            scope_registry: HashMap::new(),
        }
    }
    
    /// Register a scope with its authorized identities
    pub fn register_scope(&mut self, scope: String, authorized_identities: HashSet<String>) {
        self.scope_registry.insert(scope, authorized_identities);
    }
    
    /// Check if an identity is authorized for a scope
    fn is_authorized_for_scope(&self, identity_id: &str, scope: &str) -> bool {
        // Global identities are authorized for all scopes
        if self.scope_registry.get("global")
            .map(|ids| ids.contains(identity_id))
            .unwrap_or(false) {
            return true;
        }
        
        // Check if the identity is explicitly authorized for this scope
        self.scope_registry.get(scope)
            .map(|ids| ids.contains(identity_id))
            .unwrap_or(false)
    }
    
    /// Validate a node's lineage (basic checks)
    async fn validate_node_basic(&self, node: &DAGNode) -> Result<(), LineageVerificationError> {
        // Verify the node's signature
        if !node.verify().map_err(|e| {
            LineageVerificationError::InvalidSignature(format!("Signature verification failed: {}", e))
        })? {
            return Err(LineageVerificationError::InvalidSignature("Invalid signature".into()));
        }
        
        // Verify that the scope is valid
        if node.header.scope.is_empty() {
            return Err(LineageVerificationError::MissingField("Empty scope".into()));
        }
        
        // Additional basic validations can be added here
        
        Ok(())
    }
    
    /// Validate a node's scope authority
    async fn validate_node_scope_authority(&self, node: &DAGNode) -> Result<(), LineageVerificationError> {
        let creator_id = node.header.creator.id();
        let scope = &node.header.scope;
        
        // Check if this is a scope creation event, which has special rules
        if matches!(node.header.node_type, 
            DAGNodeType::CooperativeCreation | 
            DAGNodeType::FederationCreation
        ) {
            // For now, allow scope creation without pre-existing authorization
            // In a real implementation, we would check federation join requests
            return Ok(());
        }
        
        // For all other operations, verify the creator is authorized for this scope
        if !self.is_authorized_for_scope(creator_id, scope) {
            return Err(LineageVerificationError::UnauthorizedCreator(
                format!("Identity {} not authorized for scope {}", creator_id, scope)
            ));
        }
        
        Ok(())
    }
    
    /// Validate a node's payload is consistent with its type
    async fn validate_node_payload(
        &self, 
        node: &DAGNode,
        parent_nodes: &[DAGNode],
    ) -> Result<(), LineageVerificationError> {
        match node.header.node_type {
            DAGNodeType::Identity => {
                // Identity nodes should contain identity data
                if node.payload.get("identity").is_none() {
                    return Err(LineageVerificationError::MissingField(
                        "Identity node missing 'identity' field".into()
                    ));
                }
            }
            
            DAGNodeType::CooperativeCreation => {
                // Cooperative creation should have members
                if node.payload.get("members").is_none() {
                    return Err(LineageVerificationError::MissingField(
                        "Cooperative creation node missing 'members' field".into()
                    ));
                }
            }
            
            DAGNodeType::FederationCreation => {
                // Federation creation should have founding cooperatives
                if node.payload.get("cooperatives").is_none() {
                    return Err(LineageVerificationError::MissingField(
                        "Federation creation node missing 'cooperatives' field".into()
                    ));
                }
            }
            
            DAGNodeType::CredentialIssuance => {
                // Credential issuance should have a credential
                if node.payload.get("credential").is_none() {
                    return Err(LineageVerificationError::MissingField(
                        "Credential issuance node missing 'credential' field".into()
                    ));
                }
                
                // Verify the credential if possible
                if let Some(credential_value) = node.payload.get("credential") {
                    let credential: Result<Credential, _> = serde_json::from_value(credential_value.clone());
                    
                    if let Ok(credential) = credential {
                        if !credential.verify().map_err(|e| {
                            LineageVerificationError::InvalidCredential(format!("Credential verification failed: {}", e))
                        })? {
                            return Err(LineageVerificationError::InvalidCredential("Invalid credential signature".into()));
                        }
                    }
                }
            }
            
            // Add validation for other node types as needed
            _ => {
                // Default validation for other types - no specific requirements yet
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl DagReplayVerifier for DefaultDagReplayVerifier {
    async fn verify_node_lineage(&self, node_id: &DAGNodeID) -> Result<LineageVerificationResult, ServiceError> {
        let node = self.dag_storage.get_node(node_id).await?;
        let parent_nodes = self.dag_storage.get_parents(node_id).await?;
        
        // First, perform basic validation
        let basic_validation = self.validate_node_basic(&node).await;
        if let Err(e) = basic_validation {
            return Ok(LineageVerificationResult {
                node_id: node_id.clone(),
                success: false,
                error: Some(e),
                depth: 0, // Will be calculated later if needed
                scope: node.header.scope.clone(),
                node_type: node.header.node_type.clone(),
                creator: node.header.creator.clone(),
                timestamp: node.header.timestamp,
            });
        }
        
        // Then, validate scope authority
        let scope_validation = self.validate_node_scope_authority(&node).await;
        if let Err(e) = scope_validation {
            return Ok(LineageVerificationResult {
                node_id: node_id.clone(),
                success: false,
                error: Some(e),
                depth: 0,
                scope: node.header.scope.clone(),
                node_type: node.header.node_type.clone(),
                creator: node.header.creator.clone(),
                timestamp: node.header.timestamp,
            });
        }
        
        // Finally, validate payload consistency
        let payload_validation = self.validate_node_payload(&node, &parent_nodes).await;
        if let Err(e) = payload_validation {
            return Ok(LineageVerificationResult {
                node_id: node_id.clone(),
                success: false,
                error: Some(e),
                depth: 0,
                scope: node.header.scope.clone(),
                node_type: node.header.node_type.clone(),
                creator: node.header.creator.clone(),
                timestamp: node.header.timestamp,
            });
        }
        
        // All validations passed
        Ok(LineageVerificationResult {
            node_id: node_id.clone(),
            success: true,
            error: None,
            depth: 0, // Depth is calculated during DAG verification
            scope: node.header.scope.clone(),
            node_type: node.header.node_type.clone(),
            creator: node.header.creator.clone(),
            timestamp: node.header.timestamp,
        })
    }
    
    async fn verify_dag(&self) -> Result<Vec<LineageVerificationResult>, ServiceError> {
        let roots = self.dag_storage.get_roots().await?;
        let mut results = Vec::new();
        
        // Use breadth-first search to verify all nodes in order
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut depth_map = HashMap::new();
        
        // Add all roots to the queue with depth 0
        for root in roots {
            let root_id = root.id()?;
            queue.push_back((root, 0));
            visited.insert(root_id.clone());
            depth_map.insert(root_id, 0);
        }
        
        while let Some((node, depth)) = queue.pop_front() {
            let node_id = node.id()?;
            
            // Verify this node's lineage
            let mut verification_result = self.verify_node_lineage(&node_id).await?;
            verification_result.depth = depth;
            results.push(verification_result);
            
            // Add children to the queue if verification was successful
            if verification_result.success {
                let children = self.dag_storage.get_children(&node_id).await?;
                
                for child in children {
                    let child_id = child.id()?;
                    
                    if !visited.contains(&child_id) {
                        queue.push_back((child, depth + 1));
                        visited.insert(child_id.clone());
                        depth_map.insert(child_id, depth + 1);
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    async fn verify_scope(&self, scope: &str) -> Result<Vec<LineageVerificationResult>, ServiceError> {
        let scope_nodes = self.dag_storage.get_nodes_by_scope(scope, None, None).await?;
        let mut results = Vec::new();
        
        // First, find all the roots within this scope
        let mut scope_roots = Vec::new();
        let mut all_node_ids = HashSet::new();
        let mut node_map = HashMap::new();
        
        for node in scope_nodes {
            let node_id = node.id()?;
            all_node_ids.insert(node_id.clone());
            node_map.insert(node_id.clone(), node.clone());
            
            // A node is a root if all its parents are outside this scope
            let mut is_root = true;
            for parent_id in &node.header.parents {
                if all_node_ids.contains(parent_id) {
                    is_root = false;
                    break;
                }
            }
            
            if is_root {
                scope_roots.push(node);
            }
        }
        
        // Use breadth-first search to verify all nodes in this scope
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut depth_map = HashMap::new();
        
        // Add all scope roots to the queue with depth 0
        for root in scope_roots {
            let root_id = root.id()?;
            queue.push_back((root, 0));
            visited.insert(root_id.clone());
            depth_map.insert(root_id, 0);
        }
        
        while let Some((node, depth)) = queue.pop_front() {
            let node_id = node.id()?;
            
            // Verify this node's lineage
            let mut verification_result = self.verify_node_lineage(&node_id).await?;
            verification_result.depth = depth;
            results.push(verification_result);
            
            // Add children to the queue if they're in this scope and verification was successful
            if verification_result.success {
                let children = self.dag_storage.get_children(&node_id).await?;
                
                for child in children {
                    if child.header.scope == scope {
                        let child_id = child.id()?;
                        
                        if !visited.contains(&child_id) {
                            queue.push_back((child, depth + 1));
                            visited.insert(child_id.clone());
                            depth_map.insert(child_id, depth + 1);
                        }
                    }
                }
            }
        }
        
        Ok(results)
    }
} 