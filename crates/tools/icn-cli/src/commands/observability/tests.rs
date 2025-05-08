#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use crate::error::CliResult;
    use icn_types::dag::{DagNode, DagNodeBuilder, DagNodeMetadata, DagPayload, NodeScope, SignedDagNode};
    use icn_types::dag::memory::MemoryDagStore;
    use icn_types::{Cid, Did};
    use icn_identity_core::did::DidKey;
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use chrono::{DateTime, Utc};
    use std::sync::Arc;
    use serde_json::json;
    
    // Helper function to create a test DAG store with sample data
    async fn setup_test_dag_store() -> Arc<dyn icn_types::dag::DagStore + Send + Sync> {
        // Create a memory store
        let store = Box::new(MemoryDagStore::new());
        let store_arc = Arc::new(store);
        let mut store = icn_types::dag::SharedDagStore::from_arc(store_arc.clone());
        
        // Generate a test key
        let secret_bytes = [1u8; 32]; // Test seed
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let did_key = DidKey::from_signing_key(signing_key);
        let did = did_key.did().clone();
        
        // Create a federation
        let federation_id = "fed-test";
        let federation_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Federation)
            .with_author(did.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "FederationCreate",
                "name": "Test Federation",
                "description": "A federation for testing"
            })))
            .build()
            .unwrap();
        
        let federation_signed = SignedDagNode {
            node: federation_node,
            signature: did_key.sign(b"federation").unwrap(),
            cid: None,
        };
        
        let federation_cid = store.add_node(federation_signed).await.unwrap();
        
        // Create a cooperative
        let coop_id = "coop-test";
        let coop_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_author(did.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "CooperativeCreate",
                "name": "Test Cooperative",
                "description": "A cooperative for testing"
            })))
            .build()
            .unwrap();
        
        let coop_signed = SignedDagNode {
            node: coop_node,
            signature: did_key.sign(b"cooperative").unwrap(),
            cid: None,
        };
        
        let coop_cid = store.add_node(coop_signed).await.unwrap();
        
        // Add a policy to the cooperative
        let policy_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_author(did.clone())
            .with_parent(coop_cid.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "Policy",
                "policy_type": "ScopePolicy",
                "rules": {
                    "mint_token": {
                        "quorum": 2,
                        "roles": ["admin"]
                    }
                }
            })))
            .build()
            .unwrap();
        
        let policy_signed = SignedDagNode {
            node: policy_node,
            signature: did_key.sign(b"policy").unwrap(),
            cid: None,
        };
        
        let policy_cid = store.add_node(policy_signed).await.unwrap();
        
        // Add a proposal to the cooperative
        let proposal_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_author(did.clone())
            .with_parent(policy_cid.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "Proposal",
                "title": "Test Proposal",
                "content": "This is a test proposal",
                "created_at": Utc::now()
            })))
            .build()
            .unwrap();
        
        let proposal_signed = SignedDagNode {
            node: proposal_node,
            signature: did_key.sign(b"proposal").unwrap(),
            cid: None,
        };
        
        let proposal_cid = store.add_node(proposal_signed).await.unwrap();
        
        // Add a vote on the proposal
        let vote_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_author(did.clone())
            .with_parent(proposal_cid.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "Vote",
                "vote": "approve",
                "target_cid": proposal_cid.to_string(),
                "reason": "Looks good to me"
            })))
            .build()
            .unwrap();
        
        let vote_signed = SignedDagNode {
            node: vote_node,
            signature: did_key.sign(b"vote").unwrap(),
            cid: None,
        };
        
        let vote_cid = store.add_node(vote_signed).await.unwrap();
        
        // Add a policy update proposal with quorum proof
        let policy_update_node = DagNodeBuilder::new()
            .with_federation_id(federation_id.to_string())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(coop_id.to_string())
            .with_author(did.clone())
            .with_parent(vote_cid.clone())
            .with_payload(DagPayload::Json(json!({
                "type": "PolicyUpdate",
                "policy_type": "ScopePolicy",
                "rules": {
                    "mint_token": {
                        "quorum": 3,
                        "roles": ["admin", "member"]
                    }
                },
                "quorum_proof": {
                    "required_signers": [did.to_string()],
                    "signers": [
                        {
                            "did": did.to_string(),
                            "role": "admin",
                            "scope": coop_id
                        }
                    ],
                    "policy_version": 1
                }
            })))
            .build()
            .unwrap();
        
        let policy_update_signed = SignedDagNode {
            node: policy_update_node,
            signature: did_key.sign(b"policy_update").unwrap(),
            cid: None,
        };
        
        let _ = store.add_node(policy_update_signed).await.unwrap();
        
        store_arc
    }
    
    #[tokio::test]
    async fn test_dag_view() {
        let store = setup_test_dag_store().await;
        let dag_inspector = super::DAGInspector::new(store);
        
        // Test cooperative scope
        let nodes = dag_inspector.get_scope_nodes(
            NodeScope::Cooperative, 
            Some("coop-test")
        ).await.unwrap();
        
        assert!(!nodes.is_empty(), "Should find cooperative nodes");
        
        // Test rendering
        let text = dag_inspector.render_text(&nodes, 10);
        assert!(text.contains("Test Proposal"), "Should include proposal title");
        assert!(text.contains("Vote: approve"), "Should include vote information");
        
        let json = dag_inspector.render_json(&nodes, 10);
        assert!(json.contains("Test Proposal"), "JSON should include proposal title");
    }
    
    #[tokio::test]
    async fn test_policy_inspector() {
        use super::policy_inspector::PolicyInspector;
        
        let store = setup_test_dag_store().await;
        let policy_inspector = PolicyInspector::new(store);
        
        // Test cooperative policy
        let policy_info = policy_inspector.get_active_policy(
            NodeScope::Cooperative, 
            Some("coop-test")
        ).await.unwrap();
        
        assert!(policy_info.is_some(), "Should find a policy");
        
        if let Some(policy) = policy_info {
            // Test policy content
            let content_str = serde_json::to_string(&policy.content).unwrap();
            assert!(content_str.contains("mint_token"), "Policy should contain mint_token rule");
            
            // Test rendering
            let text = policy_inspector.render_text(&policy);
            assert!(text.contains("ACTIVE POLICY"), "Should include policy header");
            
            let json = policy_inspector.render_json(&policy);
            assert!(json.contains("policy"), "JSON should include policy section");
        }
    }
    
    #[tokio::test]
    async fn test_quorum_validator() {
        use super::quorum_validator::QuorumValidator;
        
        let store = setup_test_dag_store().await;
        
        // Find the node with quorum proof
        let all_nodes = store.get_ordered_nodes().await.unwrap();
        let quorum_node = all_nodes.iter().find(|node| {
            if let DagPayload::Json(json) = &node.node.payload {
                json.get("quorum_proof").is_some()
            } else {
                false
            }
        }).unwrap();
        
        let quorum_cid = quorum_node.cid.as_ref().unwrap();
        
        let quorum_validator = QuorumValidator::new(store);
        let quorum_info = quorum_validator.validate_quorum(quorum_cid).await.unwrap();
        
        // Test quorum validation
        assert!(quorum_info.is_valid, "Quorum should be valid");
        assert!(!quorum_info.required_signers.is_empty(), "Should have required signers");
        assert!(!quorum_info.actual_signers.is_empty(), "Should have actual signers");
        
        // Test rendering
        let text = quorum_validator.render_text(&quorum_info, true);
        assert!(text.contains("VALID"), "Should indicate valid quorum");
        
        let json = quorum_validator.render_json(&quorum_info, true);
        assert!(json.contains("\"is_valid\": true"), "JSON should indicate valid quorum");
    }
    
    #[tokio::test]
    async fn test_activity_log() {
        use super::activity_log::{ActivityLog, ActivityType};
        
        let store = setup_test_dag_store().await;
        let activity_log = ActivityLog::new(store);
        
        // Test cooperative activities
        let activities = activity_log.get_scope_activities(
            NodeScope::Cooperative, 
            Some("coop-test"),
            10
        ).await.unwrap();
        
        assert!(!activities.is_empty(), "Should find activities");
        
        // Check for expected activity types
        let activity_types: Vec<_> = activities.iter()
            .map(|a| &a.activity_type)
            .collect();
            
        assert!(activity_types.iter().any(|t| matches!(t, ActivityType::ProposalSubmitted)), 
                "Should include proposal activity");
        assert!(activity_types.iter().any(|t| matches!(t, ActivityType::VoteCast)), 
                "Should include vote activity");
        assert!(activity_types.iter().any(|t| matches!(t, ActivityType::PolicyChanged)), 
                "Should include policy activity");
        
        // Test rendering
        let text = activity_log.render_text(&activities);
        assert!(text.contains("GOVERNANCE ACTIVITY LOG"), "Should include log header");
        
        let json = activity_log.render_json(&activities);
        assert!(json.contains("activities"), "JSON should include activities array");
    }
    
    #[tokio::test]
    async fn test_federation_overview() {
        use super::federation_overview::FederationInspector;
        
        let store = setup_test_dag_store().await;
        let federation_inspector = FederationInspector::new(store);
        
        // Test federation overview
        let overview = federation_inspector.get_federation_overview("fed-test").await.unwrap();
        
        assert_eq!(overview.federation_id, "fed-test", "Should match federation ID");
        assert!(overview.description.is_some(), "Should have description");
        assert!(!overview.cooperatives.is_empty(), "Should have cooperative members");
        
        // Test rendering
        let text = federation_inspector.render_text(&overview);
        assert!(text.contains("FEDERATION OVERVIEW"), "Should include overview header");
        assert!(text.contains("Test Cooperative"), "Should include cooperative name");
        
        let json = federation_inspector.render_json(&overview);
        assert!(json.contains("federation"), "JSON should include federation section");
        assert!(json.contains("members"), "JSON should include members section");
    }
} 