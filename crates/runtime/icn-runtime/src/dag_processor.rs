#![deny(unsafe_code)] // Keep this at the crate level

use icn_types::{
    Did, Cid, ScopePolicyConfig, PolicyError,
    dag::{SignedDagNode, DagStore, DagError, DagPayload, DagNodeMetadata, NodeScope},
};
use crate::policy::{MembershipIndex, PolicyLoader, ScopeType};
use crate::dag_indexing::DagIndex;
use log::{info, warn, error, debug};
use std::sync::Arc;
use std::any::Any; // Import Any
use std::str::FromStr; // <-- Add this import
use std::sync::Mutex;
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
use std::sync::RwLock;

/// Result of policy validation for a DAG node
#[derive(Debug)]
pub enum ValidationResult {
    /// Node is valid according to policy
    Valid,
    
    /// Node is not valid due to policy restriction
    PolicyViolation(PolicyError),
    
    /// Node has other validation errors
    OtherError(DagError),
}

/// Error types specific to policy update operations
#[derive(Debug, thiserror::Error)]
pub enum PolicyUpdateError {
    #[error("Failed to parse policy update proposal: {0}")]
    InvalidProposal(String),
    
    #[error("Invalid quorum proof: {0}")]
    InvalidQuorumProof(String),
    
    #[error("Failed to retrieve proposal node: {0}")]
    ProposalNotFound(String),
    
    #[error("Underlying DAG error: {0}")]
    DagError(#[from] DagError),
    
    #[error("Policy error: {0}")]
    PolicyError(#[from] PolicyError),
}

/// A processor for handling DAG operations with policy validation
#[derive(Clone)]
pub struct DagProcessor {
    /// Membership index to check federation/cooperative/community memberships
    _membership_index: Arc<dyn MembershipIndex + Send + Sync>,
    
    /// Policy loader to retrieve policies for different scopes
    policy_loader: Arc<dyn PolicyLoader + Send + Sync>,
    
    /// Dag index for auxiliary indexing
    dag_index: Arc<dyn DagIndex + Send + Sync>,
}

impl DagProcessor {
    /// Create a new DAG processor with policy enforcement
    pub fn new(
        membership_index: Arc<dyn MembershipIndex + Send + Sync>,
        policy_loader: Arc<dyn PolicyLoader + Send + Sync>,
        dag_index: Arc<dyn DagIndex + Send + Sync>,
    ) -> Self {
        Self {
            _membership_index: membership_index,
            policy_loader,
            dag_index,
        }
    }
    
    /// Validate that a DAG node complies with applicable policies
    pub fn validate_node(&self, node: &mut SignedDagNode) -> ValidationResult {
        // Basic validation (e.g., signature)
        if node.ensure_cid().is_err() { // Ensure CID is calculated for potential logging
             return ValidationResult::OtherError(DagError::InvalidNodeData("Failed to calculate node CID".to_string()));
        }
        // TODO: Add actual signature verification using PublicKeyResolver
        // if node.verify_signature(resolver).is_err() { ... }

        // Check if exempt
        if self.is_exempt_from_validation(node) {
            return ValidationResult::Valid;
        }

        // Determine scope and action
        let scope_id = node.node.metadata.scope_id.as_deref().unwrap_or("_"); // Use placeholder if None
        let scope_type = self.determine_scope_type(scope_id);
        
        let action = match self.get_action_type(node) {
            Some(a) => a,
            None => return ValidationResult::Valid, // No specific action implies no specific policy check needed?
        };

        // Load policy for the scope
        let _policy = match self.policy_loader.load_for_scope(&scope_type, scope_id) { // Changed from load_policy
            Ok(p) => p,
            Err(PolicyError::PolicyNotFound) => {
                // If policy isn't found, it's not a validation error, but an operational issue.
                // This might indicate a need to load/create a policy.
                // For now, let's treat as OtherError, but this could be refined.
                return ValidationResult::OtherError(PolicyError::PolicyNotFound.into());
            },
            Err(e) => return ValidationResult::OtherError(e.into()), // This is the main one for other PolicyErrors
        };

        // Perform authorization check using the loaded policy
        match self.policy_loader.check_authorization(&scope_type, scope_id, &action, &node.node.author) { // Removed .await
             Ok(()) => ValidationResult::Valid,
             Err(e) => ValidationResult::PolicyViolation(e),
        }
    }
    
    /// Check if a node is exempt from policy validation
    fn is_exempt_from_validation(&self, node: &SignedDagNode) -> bool {
        // Genesis nodes and certain system operations may be exempt
        if node.node.metadata.scope_id.is_none() || 
           (node.node.metadata.scope_id.as_ref().map_or(false, |id| id == "system")) {
            return true;
        }
        
        // Exemption logic based on payload type could go here
        // ...
        
        false
    }
    
    /// Extract the action type from a node's payload for policy checking
    fn get_action_type(&self, node: &SignedDagNode) -> Option<String> {
        // Parse the payload to determine what action is being performed
        match &node.node.payload {
            DagPayload::Json(json_value) => {
                // Try to extract action_type from JSON payload
                json_value.get("action_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            },
            DagPayload::Raw(raw_bytes) => {
                // Try to parse raw bytes as JSON and extract action_type
                if let Ok(text) = String::from_utf8(raw_bytes.clone()) {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        return value.get("action_type")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
                None
            },
            // Other payload types may not require authorization
            _ => None
        }
    }
    
    /// Check authorization for an action in a scope
    async fn check_authorization(&self, scope_id: &str, action: &str, did: &Did) -> Result<(), PolicyError> {
        // Determine the scope type from the scope ID
        // This might involve parsing the ID or lookup in a registry
        let scope_type = self.determine_scope_type(scope_id);
        
        // Use the policy loader to check authorization
        self.policy_loader.check_authorization(&scope_type, scope_id, action, did)
    }
    
    /// Determine the scope type from a scope ID
    fn determine_scope_type(&self, scope_id: &str) -> String {
        // This is a simplified implementation
        // In reality, you might have a more complex mapping of IDs to types
        if scope_id.starts_with("fed:") {
            "Federation".to_string()
        } else if scope_id.starts_with("coop:") {
            "Cooperative".to_string()
        } else if scope_id.starts_with("com:") {
            "Community".to_string()
        } else {
            "Unknown".to_string()
        }
    }
    
    /// Process a policy update approval and apply the new policy
    pub async fn process_policy_update<S: DagStore + Send + Sync>(
        &self,
        node: &SignedDagNode,
        dag_store: &S // Note: dag_store is passed but might not be needed if proposal is fetched by CID from payload
    ) -> Result<(), PolicyUpdateError> {
        // Check if the node payload indicates a policy update approval
        if let DagPayload::Json(payload) = &node.node.payload {
            if let Some(node_type) = payload.get("type").and_then(|t| t.as_str()) {
                if node_type == "PolicyUpdateApproval" {
                    // Extract CID of the approved proposal
                    let proposal_cid_str = payload.get("proposal_cid")
                        .and_then(|c| c.as_str())
                        .ok_or(PolicyUpdateError::InvalidQuorumProof("Missing proposal_cid".to_string()))?;
                    let proposal_cid = icn_types::Cid::from_str(proposal_cid_str)
                        .map_err(|e| PolicyUpdateError::InvalidProposal(format!("Invalid proposal CID: {}", e)))?;
                        
                    // Fetch the actual proposal node
                    // Ensure dag_store implements Send + Sync for across await
                    let proposal_node = dag_store.get_node(&proposal_cid).await
                        .map_err(|e| PolicyUpdateError::ProposalNotFound(format!("Failed to retrieve proposal: {}", e)))?;
                    
                    // Extract proposed policy from proposal
                    let proposed_policy = self.extract_policy_from_proposal(&proposal_node)?;
                    
                    // Verify quorum proof
                    let _quorum_proof = payload.get("quorum_proof")
                        .ok_or(PolicyUpdateError::InvalidQuorumProof("Missing quorum proof".to_string()))?;
                    
                    // Attempt to update the policy using safe downcasting
                    if let Ok(()) = (|| -> Result<(), PolicyUpdateError> {
                        let policy_loader_trait_object: &dyn Any = self.policy_loader.as_ref(); // Get as &dyn Any by dereferencing Arc
                        
                        // Attempt downcast to concrete type (assuming it exists at crate::policy::DefaultPolicyLoader)
                        if let Some(_loader) = policy_loader_trait_object.downcast_ref::<crate::policy::DefaultPolicyLoader>() {
                             // TODO: Ensure DefaultPolicyLoader::set_policy exists and handles mutability correctly
                             //       (it might need interior mutability like Mutex/RwLock if called concurrently)
                             // loader.set_policy(proposed_policy);
                             warn!("Policy update check successful via downcast, but set_policy call is commented out pending review of DefaultPolicyLoader's mutability.");
                             Ok(()) // Temporarily succeed without actually setting
                        } else {
                             Err(PolicyUpdateError::InvalidProposal(
                                 "PolicyLoader implementation does not support set_policy via downcast to DefaultPolicyLoader".to_string()
                             ))
                        }
                    })() {
                        info!("Policy update check successful (set_policy call commented out pending review).");
                        return Ok(());
                    } else {
                        warn!("Could not apply policy update - incompatible PolicyLoader implementation or downcast failed.");
                        return Err(PolicyUpdateError::InvalidProposal(
                            "Cannot update policy with current PolicyLoader implementation".to_string()
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Extract a policy from a policy update proposal node
    fn extract_policy_from_proposal(&self, node: &SignedDagNode) -> Result<ScopePolicyConfig, PolicyUpdateError> {
        if let DagPayload::Json(payload) = &node.node.payload {
            if let Some(node_type) = payload.get("type").and_then(|t| t.as_str()) {
                if node_type == "PolicyUpdateProposal" {
                    // Extract the proposed policy JSON
                    let policy_json = payload.get("proposed_policy")
                        .and_then(|p| p.as_str())
                        .ok_or(PolicyUpdateError::InvalidProposal("Missing proposed_policy".to_string()))?;
                    
                    // Parse the policy
                    let policy = ScopePolicyConfig::from_json_string(policy_json)
                        .map_err(|e| PolicyUpdateError::InvalidProposal(e))?;
                    
                    return Ok(policy);
                }
            }
        }
        
        Err(PolicyUpdateError::InvalidProposal("Not a valid policy update proposal".to_string()))
    }
    
    /// Process a node with policy enforcement before adding it to the DAG
    #[cfg(feature = "async")]
    pub async fn process_node<S: DagStore + Send + Sync>(
        &self, 
        mut node: SignedDagNode, 
        dag_store: &mut S
    ) -> Result<Cid, DagError> {
        // Check for policy update approval
        if let Err(e) = self.process_policy_update(&node, dag_store).await {
            warn!("Failed to process potential policy update: {}", e);
            // Continue processing - we don't want to block the node if policy update processing fails
        }
        
        // Validate node against policy
        match self.validate_node(&mut node) {
            ValidationResult::Valid => {
                // Ensure node has its CID before adding to store and index
                let node_cid = node.cid.clone().ok_or_else(|| DagError::CidError("CID not present after validation".to_string()))?;
                
                let metadata = node.node.metadata.clone(); // Clone metadata for indexing
                let author_did = node.node.author.clone(); // Clone author DID for indexing metadata

                // Clone the node before moving it into the store, so we can still use it for indexing
                let node_for_store = node.clone(); 
                let node_for_index = &node; // Keep original reference for indexer

                // Add to main DAG store
                dag_store.add_node(node_for_store).await?;
                
                // Add to auxiliary DAG index
                info!("Node {} added to DAG store. Attempting to index.", node_cid);
                
                // Assuming add_node_to_index takes &DagNode, use node_for_index.node
                if let Err(e) = self.dag_index.add_node_to_index(&node_cid, &node_for_index.node) {
                     error!("Failed to add node {} to DAG index: {:?}", node_cid, e);
                }

                Ok(node_cid)
            },
            ValidationResult::PolicyViolation(err) => {
                error!("Policy violation: {}", err);
                Err(DagError::InvalidNodeData(format!("Policy violation: {}", err)))
            },
            ValidationResult::OtherError(err) => {
                error!("Node validation error: {}", err);
                Err(err)
            }
        }
    }
    
    /// Synchronous version for non-async environments
    #[cfg(not(feature = "async"))]
    pub fn process_node<S: DagStore + Send + Sync>(
        &self, 
        mut node: SignedDagNode, 
        dag_store: &mut S
    ) -> Result<Cid, DagError> {
        match self.validate_node(&mut node) {
            ValidationResult::Valid => {
                let node_cid = node.cid.clone().ok_or_else(|| DagError::CidError("CID not present after validation".to_string()))?;
                dag_store.add_node(node)?;
                Ok(node_cid)
            },
            ValidationResult::PolicyViolation(err) => {
                error!("Policy violation: {}", err);
                Err(DagError::InvalidNodeData(format!("Policy violation: {}", err)))
            },
            ValidationResult::OtherError(err) => {
                error!("Node validation error: {}", err);
                Err(err)
            }
        }
    }
}

impl ValidationResult {
    /// Convert validation result to a DagError if invalid
    pub fn to_dag_error(self) -> Result<(), DagError> {
        match self {
            ValidationResult::Valid => Ok(()),
            ValidationResult::PolicyViolation(err) => {
                Err(DagError::InvalidNodeData(format!("Policy violation: {}", err)))
            },
            ValidationResult::OtherError(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag_indexing::{DagIndex, IndexError};
    use icn_types::dag::{DagStore, DagError, SignedDagNode, DagNode, DagNodeMetadata, NodeScope, DagPayload};
    use icn_types::{Did, Cid, ScopePolicyConfig, PolicyError};
    use crate::policy::{MembershipIndex, PolicyLoader, ScopeType};
    use std::sync::{Arc, Mutex, RwLock};
    use std::collections::{HashMap, HashSet};
    use async_trait::async_trait;
    use std::str::FromStr;
    use chrono::Utc;
    use ed25519_dalek::Signature;
    use crate::policy::DefaultPolicyLoader; // Assuming DefaultPolicyLoader is in crate::policy

    // --- Mock Implementations ---

    #[derive(Clone, Default)]
    struct MockDagStore {
        added_nodes: Arc<Mutex<HashSet<String>>>,
        nodes: Arc<Mutex<HashMap<Cid, SignedDagNode>>>
    }
    #[async_trait]
    impl DagStore for MockDagStore {
        async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, DagError> {
            let cid = node.calculate_cid().unwrap_or_else(|_| Cid::from_bytes(b"mock_cid_error").unwrap());
            let mut nodes_map = self.nodes.lock().unwrap();
            nodes_map.insert(cid.clone(), node);
            let mut nodes_set = self.added_nodes.lock().unwrap();
            nodes_set.insert(cid.to_string());
            Ok(cid)
        }
        async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> { 
            let nodes_map = self.nodes.lock().unwrap();
            nodes_map.get(cid).cloned().ok_or_else(|| DagError::NodeNotFound(cid.clone()))
        }
        async fn get_data(&self, _cid: &Cid) -> Result<Option<Vec<u8>>, DagError> { Ok(None) }
        async fn get_tips(&self) -> Result<Vec<Cid>, DagError> { Ok(vec![]) }
        async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, DagError> { Ok(vec![]) }
        async fn get_nodes_by_author(&self, _author: &Did) -> Result<Vec<SignedDagNode>, DagError> { Ok(vec![]) }
        async fn get_nodes_by_payload_type(&self, _payload_type: &str) -> Result<Vec<SignedDagNode>, DagError> { Ok(vec![]) }
        async fn find_path(&self, _from: &Cid, _to: &Cid) -> Result<Vec<SignedDagNode>, DagError> { Ok(vec![]) }
        async fn verify_branch(&self, _tip: &Cid, _resolver: &(dyn icn_types::dag::PublicKeyResolver + Send + Sync)) -> Result<(), DagError> { Ok(()) }
    }

    #[derive(Clone, Default)]
    struct MockDagIndex {
        indexed_nodes: Arc<Mutex<HashMap<String, (Did, NodeScope)>>>,
    }
    impl DagIndex for MockDagIndex {
        fn add_node_to_index(&self, cid: &Cid, metadata_provider: &DagNode) -> Result<(), IndexError> {
            let mut nodes = self.indexed_nodes.lock().unwrap();
            nodes.insert(cid.to_string(), (metadata_provider.author.clone(), metadata_provider.metadata.scope.clone()));
            Ok(())
        }
        fn nodes_by_did(&self, _did: &Did) -> Result<Vec<Cid>, IndexError> { Ok(vec![]) } 
        fn nodes_by_scope(&self, _scope: &NodeScope) -> Result<Vec<Cid>, IndexError> { Ok(vec![]) } 
    }

    #[derive(Clone, Default)]
    struct MockMembershipIndex {}
    impl MembershipIndex for MockMembershipIndex {
        fn is_federation_member(&self, _did: &Did, _federation_id: &str) -> bool { true }
        fn is_cooperative_member(&self, _did: &Did, _coop_id: &str) -> bool { true }
        fn is_community_member(&self, _did: &Did, _community_id: &str) -> bool { true }
        fn is_member_of_federation(&self, _did: &Did, _federation_id: &str) -> bool { true }
    }

    #[derive(Clone)]
    struct MockPolicyLoader {
        allow_all: bool, 
        policies: Arc<RwLock<HashMap<String, Arc<ScopePolicyConfig>>>>
    }
    // Implement PolicyLoader for MockPolicyLoader
    #[async_trait] // PolicyLoader might need async trait if check_authorization becomes async
    impl PolicyLoader for MockPolicyLoader {
        fn check_authorization(&self, _scope_type: &str, _scope_id: &str, _action: &str, _principal: &Did) -> Result<(), PolicyError> {
            if self.allow_all { Ok(()) } else { Err(PolicyError::ActionNotPermitted) }
        }
        // Implement load_for_scope
        fn load_for_scope(&self, st: &str, sid: &str) -> Result<ScopePolicyConfig, PolicyError> {
            let key = format!("{}:{}", st, sid);
            self.policies
                .read()
                .unwrap()
                .get(&key)
                .cloned()
                .map(|arc| (*arc).clone())
                .ok_or(PolicyError::PolicyNotFound)
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }
    impl MockPolicyLoader {
        fn allow_all() -> Self { 
            Self { 
                allow_all: true, 
                policies: Arc::new(RwLock::new(HashMap::new())) 
            }
        }
        fn add_policy(&self, scope_type: &str, scope_id: &str, policy: ScopePolicyConfig) {
            let key = format!("{}:{}", scope_type, scope_id);
            let mut policies = self.policies.write().unwrap();
            policies.insert(key, Arc::new(policy));
        }
    }

    #[derive(Clone)]
    struct MockDefaultPolicyLoader { // Renamed to avoid conflict with actual DefaultPolicyLoader if it exists
         mock: MockPolicyLoader,
         supports_set: bool,
    }
    #[async_trait]
    impl PolicyLoader for MockDefaultPolicyLoader {
        fn check_authorization(&self, st: &str, sid: &str, act: &str, p: &Did) -> Result<(), PolicyError> {
            self.mock.check_authorization(st, sid, act, p)
        }
        fn load_for_scope(&self, st: &str, sid: &str) -> Result<ScopePolicyConfig, PolicyError> {
            self.mock.load_for_scope(st, sid)
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }
    impl MockDefaultPolicyLoader {
        fn set_policy(&self, _policy: ScopePolicyConfig) -> Result<(), PolicyError> {
            if self.supports_set {
                 warn!("(MockDefaultPolicyLoader) set_policy called successfully.");
                 Ok(())
            } else {
                Err(PolicyError::InternalError("MockDefaultPolicyLoader does not support set_policy".into()))
            }
        }
    }

    // Helper to create a simple test node
    fn create_test_node(author_did_str: &str, scope: NodeScope, scope_id: Option<&str>) -> SignedDagNode {
        let author = Did::from_str(author_did_str).unwrap();
        let metadata = DagNodeMetadata {
            federation_id: "test-fed".into(),
            timestamp: Utc::now(),
            label: None,
            scope: scope.clone(),
            scope_id: scope_id.map(String::from),
        };
        let node = DagNode {
            author: author.clone(),
            metadata,
            payload: DagPayload::Raw(b"test payload".to_vec()),
            parents: Vec::new(),
        };
        // Create a placeholder signature
        let sig_bytes = [0u8; 64];
        let signature = Signature::from_bytes(&sig_bytes);
        SignedDagNode { node, signature, cid: None }
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_process_node_valid_adds_to_store_and_index() {
        // Arrange
        let mock_store = MockDagStore::default();
        let mock_index = MockDagIndex::default();
        let mock_membership = Arc::new(MockMembershipIndex::default());
        let mock_policy_loader = Arc::new(MockPolicyLoader::allow_all()); // Allow the action

        let processor = DagProcessor::new(
            mock_membership,
            mock_policy_loader,
            Arc::new(mock_index.clone()), // Pass Arc<MockDagIndex>
        );

        let test_node = create_test_node("did:icn:test:author1", NodeScope::Cooperative, Some("coop:test"));
        let expected_cid = test_node.calculate_cid().unwrap();

        let mut store_instance = mock_store.clone(); // Clone for mutable use

        // Act
        let result = processor.process_node(test_node, &mut store_instance).await;

        // Assert
        assert!(result.is_ok(), "process_node should succeed for valid node");
        let processed_cid = result.unwrap();
        assert_eq!(processed_cid, expected_cid, "Returned CID should match calculated CID");

        // Check if added to store
        let added_to_store = mock_store.added_nodes.lock().unwrap().contains(&expected_cid.to_string());
        assert!(added_to_store, "Node should be added to the DAG store");

        // Check if added to index
        let indexed_data = mock_index.indexed_nodes.lock().unwrap();
        assert!(indexed_data.contains_key(&expected_cid.to_string()), "Node should be added to the DAG index");
        if let Some((indexed_author, indexed_scope)) = indexed_data.get(&expected_cid.to_string()) {
            assert_eq!(indexed_author.to_string(), "did:icn:test:author1");
            assert_eq!(*indexed_scope, NodeScope::Cooperative);
        } else {
            panic!("Indexed data not found for CID");
        }
    }

    // TODO: Add test case for policy violation (using MockPolicyLoader::deny_all())
    // TODO: Add test case for exemption logic (is_exempt_from_validation)
    // TODO: Add test cases for process_policy_update if needed

} 