#[cfg(test)]
mod tests {
    use super::storage::{DagStorage, create_temp_db};
    use super::verifier::{DefaultDagReplayVerifier, LineageVerificationError};
    use icn_common::dag::{DAGNode, DAGNodeType};
    use icn_common::identity::{Identity, IdentityType, ScopedIdentity};
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;
    use tokio_test::block_on;
    
    /// Generate a sample identity for testing
    fn create_test_identity(name: &str, identity_type: IdentityType) -> (Identity, ed25519_dalek::SecretKey) {
        Identity::new(
            identity_type,
            name.to_string(),
            Some(serde_json::json!({
                "description": format!("Test identity for {}", name)
            }))
        )
    }
    
    /// Create a test DAG with a simple lineage
    async fn create_test_dag() -> (Arc<DagStorage>, Vec<icn_common::dag::DAGNodeID>) {
        // Create a temporary database
        let (temp_dir, rocks_db) = create_temp_db();
        let storage = Arc::new(DagStorage::new(rocks_db));
        storage.init().await.unwrap();
        
        // Create test identities
        let (federation_id, federation_key) = create_test_identity("Test Federation", IdentityType::Federation);
        let (coop1_id, coop1_key) = create_test_identity("Test Cooperative 1", IdentityType::Cooperative);
        let (coop2_id, coop2_key) = create_test_identity("Test Cooperative 2", IdentityType::Cooperative);
        let (individual_id, individual_key) = create_test_identity("Test Individual", IdentityType::Individual);
        
        // Create scoped identities
        let federation_scope = "federation:test".to_string();
        let federation_scoped = ScopedIdentity::new(
            federation_id.clone(),
            federation_scope.clone(),
            None
        );
        
        let coop1_scope = "cooperative:test1".to_string();
        let coop1_scoped = ScopedIdentity::new(
            coop1_id.clone(),
            coop1_scope.clone(),
            None
        );
        
        let coop2_scope = "cooperative:test2".to_string();
        let coop2_scoped = ScopedIdentity::new(
            coop2_id.clone(),
            coop2_scope.clone(),
            None
        );
        
        let individual_scoped = ScopedIdentity::new(
            individual_id.clone(),
            coop1_scope.clone(),
            None
        );
        
        // Create a federation node (root)
        let federation_node = DAGNode::new(
            DAGNodeType::FederationCreation,
            HashSet::new(), // No parents
            federation_scope.clone(),
            federation_scoped.clone(),
            serde_json::json!({
                "name": "Test Federation",
                "cooperatives": [
                    {"id": coop1_id.id.clone(), "name": coop1_id.name.clone()},
                    {"id": coop2_id.id.clone(), "name": coop2_id.name.clone()},
                ]
            }),
            &federation_key
        ).unwrap();
        
        // Add the federation node to the DAG
        let federation_node_id = storage.add_node(&federation_node).await.unwrap();
        
        // Create cooperative 1 node (references federation)
        let mut parent_set = HashSet::new();
        parent_set.insert(federation_node_id.clone());
        
        let coop1_node = DAGNode::new(
            DAGNodeType::CooperativeCreation,
            parent_set.clone(),
            coop1_scope.clone(),
            coop1_scoped.clone(),
            serde_json::json!({
                "name": "Test Cooperative 1",
                "description": "A test cooperative",
                "members": [
                    {"id": individual_id.id.clone(), "name": individual_id.name.clone()}
                ]
            }),
            &coop1_key
        ).unwrap();
        
        // Add the cooperative 1 node to the DAG
        let coop1_node_id = storage.add_node(&coop1_node).await.unwrap();
        
        // Create cooperative 2 node (references federation)
        let coop2_node = DAGNode::new(
            DAGNodeType::CooperativeCreation,
            parent_set.clone(),
            coop2_scope.clone(),
            coop2_scoped.clone(),
            serde_json::json!({
                "name": "Test Cooperative 2",
                "description": "Another test cooperative",
                "members": []
            }),
            &coop2_key
        ).unwrap();
        
        // Add the cooperative 2 node to the DAG
        let coop2_node_id = storage.add_node(&coop2_node).await.unwrap();
        
        // Create a credential issuance node (references cooperative 1)
        let mut coop1_parent_set = HashSet::new();
        coop1_parent_set.insert(coop1_node_id.clone());
        
        let credential_node = DAGNode::new(
            DAGNodeType::CredentialIssuance,
            coop1_parent_set,
            coop1_scope.clone(),
            coop1_scoped.clone(),
            serde_json::json!({
                "credential": {
                    "id": "credential:test",
                    "type": "MembershipCredential",
                    "issuer": coop1_id.id.clone(),
                    "subject": individual_id.id.clone(),
                    "claims": {
                        "role": "member",
                        "joinDate": "2023-08-15"
                    }
                }
            }),
            &coop1_key
        ).unwrap();
        
        // Add the credential node to the DAG
        let credential_node_id = storage.add_node(&credential_node).await.unwrap();
        
        // Return the storage and node IDs
        (
            storage,
            vec![
                federation_node_id,
                coop1_node_id,
                coop2_node_id,
                credential_node_id
            ]
        )
    }
    
    #[test]
    fn test_dag_storage_and_retrieval() {
        block_on(async {
            let (storage, node_ids) = create_test_dag().await;
            
            // Verify we can retrieve all nodes
            for node_id in &node_ids {
                let node = storage.get_node(node_id).await.unwrap();
                let retrieved_id = node.id().unwrap();
                assert_eq!(retrieved_id.as_str(), node_id.as_str());
            }
            
            // Check we can get nodes by type
            let federation_nodes = storage.get_nodes_by_type(
                DAGNodeType::FederationCreation,
                None,
                None
            ).await.unwrap();
            
            assert_eq!(federation_nodes.len(), 1);
            
            let coop_nodes = storage.get_nodes_by_type(
                DAGNodeType::CooperativeCreation,
                None,
                None
            ).await.unwrap();
            
            assert_eq!(coop_nodes.len(), 2);
            
            // Check we can get nodes by scope
            let federation_scope_nodes = storage.get_nodes_by_scope(
                "federation:test",
                None,
                None
            ).await.unwrap();
            
            assert_eq!(federation_scope_nodes.len(), 1);
            
            let coop1_scope_nodes = storage.get_nodes_by_scope(
                "cooperative:test1",
                None,
                None
            ).await.unwrap();
            
            assert_eq!(coop1_scope_nodes.len(), 2); // Coop1 node and credential node
        });
    }
    
    #[test]
    fn test_dag_lineage_verification() {
        block_on(async {
            let (storage, node_ids) = create_test_dag().await;
            
            // Create a verifier
            let mut verifier = DefaultDagReplayVerifier::new(storage.clone());
            
            // Register authorization (in real usage, these would come from membership attestations)
            let mut federation_auth = HashSet::new();
            federation_auth.insert("Test Federation".to_string()); // Simplified ID format for test
            verifier.register_scope("federation:test".to_string(), federation_auth);
            
            let mut coop1_auth = HashSet::new();
            coop1_auth.insert("Test Cooperative 1".to_string());
            verifier.register_scope("cooperative:test1".to_string(), coop1_auth);
            
            let mut coop2_auth = HashSet::new();
            coop2_auth.insert("Test Cooperative 2".to_string());
            verifier.register_scope("cooperative:test2".to_string(), coop2_auth);
            
            // Verify the federation node
            let federation_result = verifier.verify_node_lineage(&node_ids[0]).await.unwrap();
            assert!(federation_result.success, "Federation node verification failed");
            
            // Verify the cooperative nodes
            let coop1_result = verifier.verify_node_lineage(&node_ids[1]).await.unwrap();
            assert!(coop1_result.success, "Cooperative 1 node verification failed");
            
            let coop2_result = verifier.verify_node_lineage(&node_ids[2]).await.unwrap();
            assert!(coop2_result.success, "Cooperative 2 node verification failed");
            
            // Verify the credential node
            let credential_result = verifier.verify_node_lineage(&node_ids[3]).await.unwrap();
            assert!(credential_result.success, "Credential node verification failed");
            
            // Verify the entire DAG
            let all_results = verifier.verify_dag().await.unwrap();
            assert_eq!(all_results.len(), 4);
            assert!(all_results.iter().all(|r| r.success), "Full DAG verification failed");
            
            // Verify by scope
            let federation_scope_results = verifier.verify_scope("federation:test").await.unwrap();
            assert_eq!(federation_scope_results.len(), 1);
            
            let coop1_scope_results = verifier.verify_scope("cooperative:test1").await.unwrap();
            assert_eq!(coop1_scope_results.len(), 2);
        });
    }
} 