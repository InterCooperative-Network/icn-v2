use icn_types::{
    Did, Cid, ScopePolicyConfig, PolicyError,
    dag::{SignedDagNode, DagNode, DagStore, DagError},
};
use icn_types::dag::payload::ActionType;
use crate::policy::{MembershipIndex, PolicyLoader};
use log::{info, warn, error, debug};
use std::sync::Arc;

/// Result of policy validation for a DAG node
pub enum ValidationResult {
    /// Node is valid according to policy
    Valid,
    
    /// Node is not valid due to policy restriction
    PolicyViolation(PolicyError),
    
    /// Node has other validation errors
    OtherError(DagError),
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
    
    /// Process a node with policy enforcement before adding it to the DAG
    #[cfg(feature = "async")]
    pub async fn process_node<S: DagStore + Send + Sync>(
        &self, 
        node: SignedDagNode, 
        dag_store: &mut S
    ) -> Result<Cid, DagError> {
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