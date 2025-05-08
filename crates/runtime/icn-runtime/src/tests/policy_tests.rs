#[cfg(test)]
mod policy_tests {
    use crate::policy::{MembershipIndex, PolicyLoader, ScopeType};
    use icn_types::{Did, ScopePolicyConfig, PolicyRule, PolicyError};
    use icn_types::dag::{NodeScope, DagNodeBuilder, DagPayload, SignedDagNode};
    use crate::dag_processor::{DagProcessor, ValidationResult};
    use std::sync::Arc;
    use serde_json::json;

    // Test helper function to create a test DID
    fn create_test_did(id: &str) -> Did {
        Did::new_unchecked(&format!("did:icn:test:{}", id))
    }
    
    // Setup test federation with a member
    fn setup_test_federation() -> (String, MembershipIndex, Did) {
        let federation_id = "fed-main".to_string();
        let membership_index = MembershipIndex::new();
        let member_did = create_test_did("member1");
        
        membership_index.add_federation_member(member_did.clone(), federation_id.clone());
        
        (federation_id, membership_index, member_did)
    }
    
    // Setup test cooperative with a policy
    fn setup_test_cooperative(federation_id: &str, membership_index: &MembershipIndex, policy_loader: &PolicyLoader) -> String {
        let coop_id = "coop-test".to_string();
        
        // Create a policy for the cooperative
        let policy = ScopePolicyConfig {
            scope_type: NodeScope::Cooperative,
            scope_id: coop_id.clone(),
            allowed_actions: vec![
                PolicyRule {
                    action_type: "submit_proposal".to_string(),
                    required_membership: Some(federation_id.to_string()),
                    allowed_dids: None,
                },
                PolicyRule {
                    action_type: "mint_token".to_string(),
                    required_membership: None, 
                    allowed_dids: Some(vec![create_test_did("admin")]),
                },
            ],
        };
        
        policy_loader.set_policy(policy);
        
        coop_id
    }
    
    // Create a test DAG node
    fn create_test_node(author: Did, federation_id: &str, coop_id: &str, node_type: &str) -> SignedDagNode {
        let payload = DagPayload::Json(json!({
            "type": node_type,
            "title": "Test Proposal", 
            "content": "This is a test proposal",
            "status": "Open",
        }));
        
        let node = DagNodeBuilder::new()
            .with_payload(payload)
            .with_author(author)
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_label(node_type.to_string())
            .build()
            .unwrap();
            
        // Create a simple signed node (no actual signature)
        SignedDagNode {
            node,
            signature: Vec::new(), // No signature validation needed for this test
            cid: None,
        }
    }
    
    #[test]
    fn test_federation_member_can_submit_proposal() {
        // Setup test federation and membership
        let (federation_id, membership_index, member_did) = setup_test_federation();
        let policy_loader = PolicyLoader::new();
        
        // Setup cooperative with policy
        let coop_id = setup_test_cooperative(&federation_id, &membership_index, &policy_loader);
        
        // Create test proposal node from federation member
        let node = create_test_node(
            member_did.clone(), 
            &federation_id, 
            &coop_id, 
            "CooperativeProposal"
        );
        
        // Create processor and validate
        let processor = DagProcessor::new(
            Arc::new(membership_index),
            Arc::new(policy_loader)
        );
        
        // Node should be valid
        match processor.validate_node(&node) {
            ValidationResult::Valid => {
                // Expected - test passes
            },
            other => {
                panic!("Expected ValidatioResult::Valid, got {:?}", other);
            }
        }
    }
    
    #[test]
    fn test_non_member_cannot_submit_proposal() {
        // Setup test federation and membership
        let (federation_id, membership_index, _) = setup_test_federation();
        let policy_loader = PolicyLoader::new();
        
        // Setup cooperative with policy
        let coop_id = setup_test_cooperative(&federation_id, &membership_index, &policy_loader);
        
        // Create test proposal node from a non-member
        let non_member_did = create_test_did("nonmember");
        let node = create_test_node(
            non_member_did.clone(), 
            &federation_id, 
            &coop_id, 
            "CooperativeProposal"
        );
        
        // Create processor and validate
        let processor = DagProcessor::new(
            Arc::new(membership_index),
            Arc::new(policy_loader)
        );
        
        // Node should be rejected
        match processor.validate_node(&node) {
            ValidationResult::PolicyViolation(PolicyError::UnauthorizedScopeAccess) => {
                // Expected - test passes
            },
            other => {
                panic!("Expected PolicyViolation(UnauthorizedScopeAccess), got {:?}", other);
            }
        }
    }
    
    #[test]
    fn test_allowlist_authorization() {
        // Setup test federation and membership
        let (federation_id, membership_index, _) = setup_test_federation();
        let policy_loader = PolicyLoader::new();
        
        // Setup cooperative with policy
        let coop_id = setup_test_cooperative(&federation_id, &membership_index, &policy_loader);
        
        // Create test token minting node from admin
        let admin_did = create_test_did("admin");
        let node = create_test_node(
            admin_did.clone(), 
            &federation_id, 
            &coop_id, 
            "MintToken"
        );
        
        // Create processor and validate
        let processor = DagProcessor::new(
            Arc::new(membership_index),
            Arc::new(policy_loader)
        );
        
        // Admin should be allowed to mint tokens
        match processor.validate_node(&node) {
            ValidationResult::Valid => {
                // Expected - test passes
            },
            other => {
                panic!("Expected ValidationResult::Valid, got {:?}", other);
            }
        }
        
        // Non-admin should not be allowed to mint tokens
        let non_admin_did = create_test_did("non_admin");
        let node = create_test_node(
            non_admin_did, 
            &federation_id, 
            &coop_id, 
            "MintToken"
        );
        
        match processor.validate_node(&node) {
            ValidationResult::PolicyViolation(PolicyError::DidNotInAllowlist) => {
                // Expected - test passes
            },
            other => {
                panic!("Expected PolicyViolation(DidNotInAllowlist), got {:?}", other);
            }
        }
    }
} 