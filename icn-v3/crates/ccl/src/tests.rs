#[cfg(test)]
mod tests {
    use super::federation::{Federation, FederationManager, DefaultFederationManager};
    use super::quorum::{
        QuorumPolicy, QuorumProof, QuorumType, 
        MembershipJoinRequest, MembershipVote, MembershipAcceptance
    };
    use super::error::CclError;
    
    use icn_common::identity::{Identity, IdentityType, ScopedIdentity};
    use icn_services::dag::{DagStorage, RocksDbDagStorage, DagReplayVerifier, DefaultDagReplayVerifier};
    
    use std::sync::Arc;
    use tokio_test::block_on;
    
    /// Create test identities and a federation
    fn create_test_federation() -> (
        Arc<DagStorage>,
        Federation,
        Vec<(Identity, ed25519_dalek::SecretKey)>,
        (Identity, ed25519_dalek::SecretKey),
    ) {
        block_on(async {
            // Create temporary database
            let temp_dir = tempfile::tempdir().unwrap();
            let storage = Arc::new(RocksDbDagStorage::new(temp_dir.path()));
            let dag_storage = Arc::new(DagStorage::new(storage));
            dag_storage.init().await.unwrap();
            
            // Create federation identity
            let (federation_identity, federation_key) = Identity::new(
                IdentityType::Federation,
                "Test Federation".to_string(),
                Some(serde_json::json!({
                    "description": "A test federation for cooperative collaboration"
                }))
            );
            
            // Create cooperative identities
            let mut cooperatives = Vec::new();
            
            for i in 1..=3 {
                let (coop_id, coop_key) = Identity::new(
                    IdentityType::Cooperative,
                    format!("Test Cooperative {}", i),
                    Some(serde_json::json!({
                        "description": format!("Test cooperative {}", i)
                    }))
                );
                
                cooperatives.push((coop_id, coop_key));
            }
            
            // Create federation manager
            let fed_manager = DefaultFederationManager::new(dag_storage.clone());
            
            // Create the federation
            let federation_scope = "federation:test".to_string();
            let coop_identities: Vec<Identity> = cooperatives.iter().map(|(id, _)| id.clone()).collect();
            
            let federation = fed_manager.create_federation(
                "Test Federation".to_string(),
                federation_scope.clone(),
                Some("A test federation".to_string()),
                coop_identities,
                federation_identity.clone(),
                &federation_key,
                None,
            ).await.unwrap();
            
            (dag_storage, federation, cooperatives, (federation_identity, federation_key))
        })
    }
    
    #[test]
    fn test_federation_creation() {
        let (_, federation, cooperatives, _) = create_test_federation();
        
        // Verify federation properties
        assert_eq!(federation.name, "Test Federation");
        assert_eq!(federation.scope, "federation:test");
        assert!(federation.description.is_some());
        
        // Verify federation members
        assert_eq!(federation.active_member_count(), 3);
        
        // Verify each cooperative is a member
        for (coop_id, _) in &cooperatives {
            assert!(federation.is_member(&coop_id.id));
            assert!(federation.has_role(&coop_id.id, "founder"));
        }
        
        // Verify federation policies
        assert_eq!(federation.policies.len(), 2);
        assert!(federation.get_policy("Default").is_some());
        assert!(federation.get_policy("Important").is_some());
    }
    
    #[test]
    fn test_federation_membership_flow() {
        block_on(async {
            let (dag_storage, federation, cooperatives, (federation_id, federation_key)) = create_test_federation();
            
            // Create a new cooperative that wants to join
            let (new_coop_id, new_coop_key) = Identity::new(
                IdentityType::Cooperative,
                "New Cooperative".to_string(),
                Some(serde_json::json!({
                    "description": "A new cooperative wanting to join the federation"
                }))
            );
            
            // Create scoped identity for the new cooperative
            let new_coop_scope = "cooperative:new".to_string();
            let new_coop_scoped = ScopedIdentity::new(
                new_coop_id.clone(),
                new_coop_scope.clone(),
                None
            );
            
            // Create federation manager
            let fed_manager = DefaultFederationManager::new(dag_storage.clone());
            
            // Create a membership join request
            let join_request = MembershipJoinRequest::new(
                new_coop_scoped.clone(),
                federation.scope.clone(),
                Vec::new(), // No credentials yet
                Some(serde_json::json!({
                    "reason": "Want to collaborate with the federation"
                })),
                &new_coop_key,
            ).unwrap();
            
            // Submit the join request
            let request_node_id = fed_manager.submit_join_request(join_request.clone()).await.unwrap();
            
            // Create votes from existing members
            let mut vote_node_ids = Vec::new();
            
            for (i, (coop_id, coop_key)) in cooperatives.iter().enumerate() {
                let coop_scoped = ScopedIdentity::new(
                    coop_id.clone(),
                    federation.scope.clone(),
                    None
                );
                
                let vote = MembershipVote::new(
                    join_request.id.clone(),
                    coop_scoped,
                    true, // Approve
                    Some(format!("Approve from coop {}", i+1)),
                    coop_key,
                ).unwrap();
                
                let vote_node_id = fed_manager.vote_on_join_request(vote).await.unwrap();
                vote_node_ids.push(vote_node_id);
            }
            
            // Get the votes
            let votes = fed_manager.get_request_votes(&join_request.id).await.unwrap();
            assert_eq!(votes.len(), 3);
            
            // Check all votes are approvals
            for vote in &votes {
                assert!(vote.approve);
            }
            
            // Create federation scoped identity
            let federation_scoped = ScopedIdentity::new(
                federation_id.clone(),
                federation.scope.clone(),
                federation.dag_reference.clone(),
            );
            
            // Create a quorum policy
            let policy = QuorumPolicy::new(
                "Membership".to_string(),
                federation.scope.clone(),
                QuorumType::SimpleMajority,
                None,
                None,
            );
            
            // Create signed data
            let signed_data = serde_json::to_vec(&join_request).unwrap();
            
            // Create quorum proof
            let mut quorum_proof = QuorumProof::new(
                policy,
                signed_data,
                None,
            );
            
            // Add signatures to the proof
            for vote in &votes {
                quorum_proof.add_signature(vote.voter.id().to_string(), vote.signature.clone());
            }
            
            // Create a membership credential
            let credential = icn_common::identity::Credential::new(
                new_coop_id.id.clone(),
                federation_scoped.clone(),
                federation.scope.clone(),
                std::collections::HashMap::new(), // Empty claims
                None,
                None, // No expiry
                &federation_key,
            ).unwrap();
            
            // Create a membership acceptance
            let acceptance = MembershipAcceptance::new(
                join_request.id.clone(),
                federation_scoped.clone(),
                vec![credential],
                quorum_proof,
                &federation_key,
            ).unwrap();
            
            // Accept the join request
            let acceptance_node_id = fed_manager.accept_join_request(acceptance).await.unwrap();
            
            // Verify the new cooperative is now a member
            let updated_federation = fed_manager.get_federation(&federation.scope).await.unwrap();
            assert!(updated_federation.is_member(&new_coop_id.id));
            
            // Verify the membership count increased
            assert_eq!(updated_federation.active_member_count(), 4);
        });
    }
} 