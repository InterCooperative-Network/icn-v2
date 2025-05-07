#[cfg(test)]
mod policy_update_tests {
    use crate::policy::{MembershipIndex, PolicyLoader, ScopeType};
    use icn_types::{Did, ScopePolicyConfig, PolicyRule, PolicyError};
    use icn_types::dag::{NodeScope, DagNodeBuilder, DagPayload, SignedDagNode, MemoryDagStore, DagStore};
    use icn_types::receipts::QuorumProof;
    use crate::dag_processor::{DagProcessor, ValidationResult};
    use std::sync::Arc;
    use serde_json::json;
    use tokio::sync::Mutex;

    // Test helper function to create a test DID
    fn create_test_did(id: &str) -> Did {
        Did::new_unchecked(&format!("did:icn:test:{}", id))
    }
    
    // Setup test federation with members
    async fn setup_test_federation() -> (String, MembershipIndex, Arc<Mutex<MemoryDagStore>>, Vec<Did>) {
        let federation_id = "fed-main".to_string();
        let membership_index = MembershipIndex::new();
        
        // Create multiple members
        let members = vec![
            create_test_did("member1"),
            create_test_did("member2"),
            create_test_did("member3"),
            create_test_did("member4"),
            create_test_did("member5"),
        ];
        
        // Add all members to the federation
        for member in &members {
            membership_index.add_federation_member(member.clone(), federation_id.clone());
        }
        
        // Create a DAG store
        let dag_store = Arc::new(Mutex::new(MemoryDagStore::new()));
        
        // Create a federation genesis node
        let federation_genesis = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(json!({
                "type": "FederationGenesis",
                "federationId": federation_id,
                "name": "Test Federation",
                "description": "A test federation for policy updates",
                "createdAt": chrono::Utc::now().to_rfc3339(),
            })))
            .with_author(members[0].clone())
            .with_federation_id(federation_id.clone())
            .with_scope(NodeScope::Federation)
            .with_label("FederationGenesis".to_string())
            .build()
            .unwrap();
            
        // Sign and add the genesis node
        let signed_genesis = SignedDagNode {
            node: federation_genesis,
            signature: Vec::new(), // No real signature needed for test
            cid: None,
        };
        
        // Add to the DAG store
        {
            let mut store = dag_store.lock().await;
            store.add_node(signed_genesis).await.unwrap();
        }
        
        (federation_id, membership_index, dag_store, members)
    }
    
    // Setup a test cooperative with initial policy
    async fn setup_test_cooperative(
        federation_id: &str, 
        membership_index: &MembershipIndex,
        policy_loader: &PolicyLoader,
        dag_store: &Arc<Mutex<MemoryDagStore>>, 
        founder: &Did
    ) -> String {
        let coop_id = "coop-test".to_string();
        
        // Create initial policy
        let initial_policy = ScopePolicyConfig {
            scope_type: NodeScope::Cooperative,
            scope_id: coop_id.clone(),
            allowed_actions: vec![
                PolicyRule {
                    action_type: "submit_proposal".to_string(),
                    required_membership: Some(federation_id.to_string()),
                    allowed_dids: None,
                },
            ],
        };
        
        // Set the initial policy
        policy_loader.set_policy(initial_policy);
        
        // Create cooperative genesis node
        let coop_genesis = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(json!({
                "type": "CooperativeGenesis",
                "name": coop_id,
                "federationId": federation_id,
                "description": "A test cooperative for policy updates",
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "founder": founder.to_string(),
            })))
            .with_author(founder.clone())
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.clone())
            .with_label("CooperativeGenesis".to_string())
            .build()
            .unwrap();
            
        // Sign and add the genesis node
        let signed_genesis = SignedDagNode {
            node: coop_genesis,
            signature: Vec::new(), // No real signature needed for test
            cid: None,
        };
        
        // Add to the DAG store
        {
            let mut store = dag_store.lock().await;
            store.add_node(signed_genesis).await.unwrap();
        }
        
        coop_id
    }
    
    // Create a policy update proposal
    async fn create_policy_update_proposal(
        federation_id: &str,
        coop_id: &str,
        dag_store: &Arc<Mutex<MemoryDagStore>>,
        proposer: &Did,
        new_policy: &ScopePolicyConfig
    ) -> String {
        // Convert policy to JSON
        let policy_json = serde_json::to_string(&new_policy).unwrap();
        
        // Create proposal node
        let proposal_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(json!({
                "type": "PolicyUpdateProposal",
                "scope_type": "Cooperative",
                "scope_id": coop_id,
                "proposed_policy": policy_json,
                "proposer_did": proposer.to_string(),
                "description": "Adding mint_token action to policy",
                "proposed_at": chrono::Utc::now().to_rfc3339(),
            })))
            .with_author(proposer.clone())
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_label("PolicyUpdateProposal".to_string())
            .build()
            .unwrap();
            
        // Sign and add the proposal node
        let signed_proposal = SignedDagNode {
            node: proposal_node,
            signature: Vec::new(),
            cid: None,
        };
        
        // Add to the DAG store
        let mut store = dag_store.lock().await;
        let cid = store.add_node(signed_proposal).await.unwrap();
        cid.to_string()
    }
    
    // Record a vote on a policy update proposal
    async fn vote_on_policy_update(
        federation_id: &str,
        coop_id: &str,
        proposal_cid: &str,
        dag_store: &Arc<Mutex<MemoryDagStore>>,
        voter: &Did,
        choice: &str
    ) -> String {
        // Create vote node
        let vote_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(json!({
                "type": "PolicyUpdateVote",
                "proposal_cid": proposal_cid,
                "choice": choice,
                "reason": "Test vote",
                "voter_did": voter.to_string(),
                "voted_at": chrono::Utc::now().to_rfc3339(),
            })))
            .with_author(voter.clone())
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_label("PolicyUpdateVote".to_string())
            .build()
            .unwrap();
            
        // Sign and add the vote node
        let signed_vote = SignedDagNode {
            node: vote_node,
            signature: Vec::new(),
            cid: None,
        };
        
        // Add to the DAG store
        let mut store = dag_store.lock().await;
        let cid = store.add_node(signed_vote).await.unwrap();
        cid.to_string()
    }
    
    // Create an approval for a policy update
    async fn approve_policy_update(
        federation_id: &str,
        coop_id: &str,
        proposal_cid: &str,
        vote_cids: Vec<String>,
        dag_store: &Arc<Mutex<MemoryDagStore>>,
        approver: &Did
    ) -> String {
        // Create a simplified quorum proof
        let quorum_proof = QuorumProof {
            id: uuid::Uuid::new_v4().to_string(),
            proposal_id: proposal_cid.to_string(),
            votes: vote_cids,
            threshold: 3, // Simple majority for test
            approved: true,
            timestamp: chrono::Utc::now().to_rfc3339(),
            federation_id: federation_id.to_string(),
            issuer: approver.to_string(),
            signature: Vec::new(),
            signers: Vec::new(),
        };
        
        // Create approval node
        let approval_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(json!({
                "type": "PolicyUpdateApproval",
                "proposal_cid": proposal_cid,
                "quorum_proof": quorum_proof,
                "approver_did": approver.to_string(),
                "approved_at": chrono::Utc::now().to_rfc3339(),
            })))
            .with_author(approver.clone())
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_label("PolicyUpdateApproval".to_string())
            .build()
            .unwrap();
            
        // Sign and add the approval node
        let signed_approval = SignedDagNode {
            node: approval_node,
            signature: Vec::new(),
            cid: None,
        };
        
        // Add to the DAG store
        let mut store = dag_store.lock().await;
        let cid = store.add_node(signed_approval).await.unwrap();
        cid.to_string()
    }
    
    #[tokio::test]
    async fn test_policy_update_flow() {
        // Set up test environment
        let (federation_id, membership_index, dag_store, members) = setup_test_federation().await;
        let policy_loader = PolicyLoader::new();
        
        // Set up a test cooperative with initial policy
        let coop_id = setup_test_cooperative(
            &federation_id, 
            &membership_index, 
            &policy_loader, 
            &dag_store, 
            &members[0]
        ).await;
        
        // Verify initial policy
        let initial_policy = policy_loader.load_for_scope("Cooperative", &coop_id).unwrap();
        assert_eq!(initial_policy.allowed_actions.len(), 1);
        assert_eq!(initial_policy.allowed_actions[0].action_type, "submit_proposal");
        
        // Create a new policy with additional rule
        let new_policy = ScopePolicyConfig {
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
                    allowed_dids: Some(vec![members[0].clone()]),
                },
            ],
        };
        
        // Create policy update proposal
        let proposal_cid = create_policy_update_proposal(
            &federation_id,
            &coop_id,
            &dag_store,
            &members[0],
            &new_policy
        ).await;
        
        // Create votes (3 approve, 1 reject)
        let mut vote_cids = Vec::new();
        for i in 0..4 {
            let choice = if i < 3 { "approve" } else { "reject" };
            let vote_cid = vote_on_policy_update(
                &federation_id,
                &coop_id,
                &proposal_cid,
                &dag_store,
                &members[i],
                choice
            ).await;
            vote_cids.push(vote_cid);
        }
        
        // Create policy update approval
        let approval_cid = approve_policy_update(
            &federation_id,
            &coop_id,
            &proposal_cid,
            vote_cids,
            &dag_store,
            &members[0]
        ).await;
        
        // Create DAG processor
        let dag_processor = DagProcessor::new(
            Arc::new(membership_index),
            Arc::new(policy_loader.clone())
        );
        
        // Process approval node
        let store = dag_store.lock().await;
        let approval_node = store.get_node_by_cid_string(&approval_cid).await.unwrap();
        dag_processor.process_policy_update(&approval_node, &*store).await.unwrap();
        
        // Verify policy was updated
        let updated_policy = policy_loader.load_for_scope("Cooperative", &coop_id).unwrap();
        assert_eq!(updated_policy.allowed_actions.len(), 2);
        
        // Verify new action is present
        let mint_token_rule = updated_policy.allowed_actions.iter()
            .find(|rule| rule.action_type == "mint_token");
        assert!(mint_token_rule.is_some());
        
        // Verify authorized member can mint tokens
        let authorized_member = &members[0];
        let test_node = create_test_node(
            authorized_member.clone(),
            &federation_id,
            &coop_id,
            "MintToken"
        );
        
        match dag_processor.validate_node(&test_node) {
            ValidationResult::Valid => {
                // Success - authorized member can mint tokens
            },
            other => {
                panic!("Expected ValidationResult::Valid, got {:?}", other);
            }
        }
        
        // Verify unauthorized member cannot mint tokens
        let unauthorized_member = &members[1];
        let test_node = create_test_node(
            unauthorized_member.clone(),
            &federation_id,
            &coop_id,
            "MintToken"
        );
        
        match dag_processor.validate_node(&test_node) {
            ValidationResult::PolicyViolation(PolicyError::DidNotInAllowlist) => {
                // Success - unauthorized member cannot mint tokens
            },
            other => {
                panic!("Expected PolicyViolation(DidNotInAllowlist), got {:?}", other);
            }
        }
    }
    
    // Helper function to create a test DAG node
    fn create_test_node(author: Did, federation_id: &str, coop_id: &str, node_type: &str) -> SignedDagNode {
        let payload = DagPayload::Json(json!({
            "type": node_type,
            "action_type": node_type.to_lowercase(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
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
} 