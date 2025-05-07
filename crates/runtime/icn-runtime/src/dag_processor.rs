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

/// DAG processor with policy enforcement
pub struct DagProcessor {
    /// Membership index to check federation/cooperative/community memberships
    membership_index: Arc<MembershipIndex>,
    
    /// Policy loader to retrieve policies for different scopes
    policy_loader: Arc<PolicyLoader>,
}

impl DagProcessor {
    /// Create a new DAG processor with policy enforcement
    pub fn new(membership_index: Arc<MembershipIndex>, policy_loader: Arc<PolicyLoader>) -> Self {
        Self {
            membership_index,
            policy_loader,
        }
    }
    
    /// Validate a node against policy requirements before adding it to the DAG
    pub fn validate_node(&self, node: &SignedDagNode) -> ValidationResult {
        // Check if this node requires policy enforcement
        let action_type = match node.node.payload.action_type() {
            Some(action) => action,
            None => return ValidationResult::Valid, // No policy enforcement needed
        };
        
        // Get scope information from the node
        let scope_type = match &node.node.metadata.scope {
            icn_types::dag::NodeScope::Federation => "Federation",
            icn_types::dag::NodeScope::Cooperative => "Cooperative",
            icn_types::dag::NodeScope::Community => "Community",
        };
        
        let scope_id = match &node.node.metadata.scope_id {
            Some(id) => id.clone(),
            None => {
                if scope_type != "Federation" {
                    return ValidationResult::OtherError(DagError::InvalidNodeData(
                        format!("Scope ID is required for {} scope", scope_type)
                    ));
                }
                // For federation scope, use federation_id as scope_id
                node.node.metadata.federation_id.clone()
            }
        };
        
        // Load policy for this scope
        let policy = match self.policy_loader.load_for_scope(scope_type, &scope_id) {
            Ok(policy) => policy,
            Err(PolicyError::PolicyNotFound) => {
                // No policy defined for this scope, allow the operation
                debug!("No policy found for scope {}/{}, allowing operation", scope_type, scope_id);
                return ValidationResult::Valid;
            }
            Err(err) => return ValidationResult::PolicyViolation(err),
        };
        
        // Evaluate policy
        match crate::policy::evaluate_policy(
            &policy, 
            &action_type, 
            &node.node.author, 
            &self.membership_index
        ) {
            Ok(()) => ValidationResult::Valid,
            Err(err) => ValidationResult::PolicyViolation(err),
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
                    
                    // Verify quorum proof (simplified here, would be more complex in real implementation)
                    let quorum_proof = payload.get("quorum_proof")
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
        match self.validate_node(&node) {
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