use icn_types::{
    Did, Cid, ScopePolicyConfig, PolicyError,
    dag::{SignedDagNode, DagNode, DagStore, DagError, DagPayload},
};
use icn_types::dag::payload::ActionType;
use icn_types::receipts::QuorumProof;
use crate::policy::{MembershipIndex, PolicyLoader};
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
    membership_index: Arc<dyn MembershipIndex + Send + Sync>,
    
    /// Policy loader to retrieve policies for different scopes
    policy_loader: Arc<dyn PolicyLoader + Send + Sync>,
}

impl DagProcessor {
    /// Create a new DAG processor with policy enforcement
    pub fn new(
        membership_index: Arc<dyn MembershipIndex + Send + Sync>,
        policy_loader: Arc<dyn PolicyLoader + Send + Sync>,
    ) -> Self {
        Self {
            membership_index,
            policy_loader,
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
            let did = Did::try_from(node.node.issuer.clone())
                .map_err(|e| {
                    error!("Invalid DID format in node: {}", e);
                    ValidationResult::OtherError(DagError::InvalidData(format!("Invalid DID: {}", e)))
                })?;
            
            // Apply the authorization check
            match self.check_authorization(&node.node.scope_id, &action, &did).await {
                Ok(_) => ValidationResult::Valid,
                Err(err) => {
                    warn!("Policy validation failed for {} performing '{}' in scope {}: {}", 
                         did, action, node.node.scope_id, err);
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
        if node.node.scope_id.is_empty() || node.node.scope_id == "system" {
            return true;
        }
        
        // Exemption logic based on payload type could go here
        // ...
        
        false
    }
    
    /// Extract the action type from a node's payload for policy checking
    fn get_action_type(&self, node: &SignedDagNode) -> Option<String> {
        // Parse the payload to determine what action is being performed
        // This will depend on how your payloads are structured
        match &node.node.payload {
            DagPayload::Event(event) => event.action_type(),
            DagPayload::Raw(raw_json) => {
                // Attempt to extract action_type from raw JSON if available
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_json) {
                    value.get("action_type")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            }
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
                    self.policy_loader.set_policy(proposed_policy);
                    
                    info!("Policy update successfully applied!");
                    return Ok(());
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
        node: SignedDagNode, 
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
                // Node passes policy validation, add it to the DAG
                dag_store.add_node(node).await
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
        node: SignedDagNode, 
        dag_store: &mut S
    ) -> Result<Cid, DagError> {
        // Validate node against policy
        match self.validate_node(&node) {
            ValidationResult::Valid => {
                // Node passes policy validation, add it to the DAG
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
                Err(DagError::AuthorizationError(format!("Policy violation: {}", err)))
            },
            ValidationResult::OtherError(err) => Err(err),
        }
    }
} 