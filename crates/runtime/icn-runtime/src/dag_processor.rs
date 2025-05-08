use icn_types::{
    Did, Cid, ScopePolicyConfig, PolicyError,
    dag::{SignedDagNode, DagStore, DagError, DagPayload, DagNodeMetadata, NodeScope},
};
use crate::policy::{MembershipIndex, PolicyLoader};
use crate::dag_indexing::DagIndex;
use log::{info, warn, error, debug};
use std::sync::Arc;

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
    pub async fn validate_node(&self, node: &SignedDagNode) -> ValidationResult {
        // Skip validation for special node types that don't require it
        if self.is_exempt_from_validation(node) {
            return ValidationResult::Valid;
        }
        
        // For nodes that require validation, check if authorization is needed based on payload
        if let Some(action) = self.get_action_type(node) {
            debug!("Validating node with action: {}", action);
            
            // Get the author DID
            let did = node.node.author.clone();
            
            // Get scope ID from metadata
            let scope_id = match &node.node.metadata.scope_id {
                Some(id) => id.clone(),
                None => {
                    // If scope_id is not set, use federation_id as the scope for federation-level operations
                    if let NodeScope::Federation = node.node.metadata.scope {
                        node.node.metadata.federation_id.clone()
                    } else {
                        // For non-federation scopes, scope_id is required
                        return ValidationResult::OtherError(DagError::InvalidNodeData(
                            format!("Missing scope_id for non-federation scope")
                        ));
                    }
                }
            };
            
            // Apply the authorization check
            match self.check_authorization(&scope_id, &action, &did).await {
                Ok(_) => ValidationResult::Valid,
                Err(err) => {
                    warn!("Policy validation failed for {} performing '{}' in scope {}: {}", 
                         did, action, scope_id, err);
                    ValidationResult::PolicyViolation(err)
                }
            }
        } else {
            // If no action type is defined, the node doesn't require authorization
            ValidationResult::Valid
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
        dag_store: &S
    ) -> Result<(), PolicyUpdateError> {
        // Check if this is a policy update approval
        if let DagPayload::Json(payload) = &node.node.payload {
            if let Some(node_type) = payload.get("type").and_then(|t| t.as_str()) {
                if node_type == "PolicyUpdateApproval" {
                    info!("Processing policy update approval");
                    
                    // Extract proposal CID and quorum proof
                    let proposal_cid_str = payload.get("proposal_cid")
                        .and_then(|c| c.as_str())
                        .ok_or(PolicyUpdateError::InvalidProposal("Missing proposal_cid".to_string()))?;
                    
                    let proposal_cid = Cid::from_bytes(proposal_cid_str.as_bytes())
                        .map_err(|e| PolicyUpdateError::InvalidProposal(format!("Invalid proposal CID: {}", e)))?;
                    
                    // Retrieve the proposal node
                    let proposal_node = dag_store.get_node(&proposal_cid).await
                        .map_err(|e| PolicyUpdateError::ProposalNotFound(format!("Failed to retrieve proposal: {}", e)))?;
                    
                    // Extract proposed policy from proposal
                    let proposed_policy = self.extract_policy_from_proposal(&proposal_node)?;
                    
                    // Verify quorum proof
                    let _quorum_proof = payload.get("quorum_proof")
                        .ok_or(PolicyUpdateError::InvalidQuorumProof("Missing quorum proof".to_string()))?;
                    
                    // TODO: Validate the quorum proof properly
                    // This would typically involve verifying signatures, checking vote thresholds, etc.
                    
                    // Update the policy in the policy loader
                    // For now, we'll log a warning that this operation is not fully supported
                    // A better approach would be to extend the PolicyLoader trait with set_policy
                    warn!("Policy updates are only partially supported - consider extending the PolicyLoader trait with set_policy");
                    
                    // Try to use DefaultPolicyLoader's set_policy through a closure
                    if let Ok(()) = (|| -> Result<(), PolicyUpdateError> {
                        // Try to get the policy loader as the concrete DefaultPolicyLoader type
                        let default_loader = self.policy_loader.clone();
                        let any_ptr = Arc::as_ptr(&default_loader) as *const ();
                        let loader_ptr = any_ptr as *const crate::policy::DefaultPolicyLoader;
                        
                        // Safety: this is unsafe and will only work if the actual type
                        // behind the trait object is DefaultPolicyLoader
                        if let Some(loader) = unsafe { loader_ptr.as_ref() } {
                            loader.set_policy(proposed_policy);
                            Ok(())
                        } else {
                            Err(PolicyUpdateError::InvalidProposal(
                                "PolicyLoader implementation does not support set_policy".to_string()
                            ))
                        }
                    })() {
                        info!("Policy update successfully applied!");
                        return Ok(());
                    } else {
                        warn!("Could not apply policy update - incompatible PolicyLoader implementation");
                        return Err(PolicyUpdateError::InvalidProposal(
                            "Cannot update policy with current PolicyLoader implementation".to_string()
                        ));
                    }
                }
            }
        }
        
        // Not a policy update approval, nothing to do
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
        match self.validate_node(&node).await {
            ValidationResult::Valid => {
                // Ensure node has its CID before adding to store and index
                let node_cid = node.ensure_cid()?;
                let metadata = node.node.metadata.clone(); // Clone metadata for indexing
                let author_did = node.node.author.clone(); // Clone author DID for indexing metadata

                // Add to main DAG store
                dag_store.add_node(node).await?;
                
                // Add to auxiliary DAG index
                info!("Node {} added to DAG store. Attempting to index.", node_cid);
                // TEMPORARY: Create a temporary struct that matches what SledDagIndex expects if NodeMetadata is a distinct type
                struct IndexerMetadata<'a> {
                    author: &'a Did,
                    scope: &'a NodeScope,
                }
                let indexer_meta = IndexerMetadata { author: &author_did, scope: &metadata.scope };
                
                if let Err(e) = self.dag_index.add_node_to_index(&node_cid, &node.node) {
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
        match self.validate_node(&node) {
            ValidationResult::Valid => {
                let node_cid = node.ensure_cid()?;
                dag_store.add_node(node)
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