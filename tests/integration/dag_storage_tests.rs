#[cfg(test)]
mod tests {
    use icn_types::dag::{DagNode, DagPayload, SignedDagNode};
    use icn_services_storage::{RocksDbDagStore, DagStore, ScopeAuthorization, ConnectionConfig};
    use std::sync::Arc;
    use std::collections::HashSet;
    use tempfile::TempDir;
    use tokio::test;
    use anyhow::Result;
    use icn_identity_core::did::DidKey;

    /// Helper function to create a test DAG store with a temp directory
    async fn create_test_store() -> Result<(TempDir, Arc<RocksDbDagStore>)> {
        let tmp = TempDir::new().expect("Failed to create temp directory");
        let config = ConnectionConfig {
            path: tmp.path().to_path_buf(),
            create_if_missing: true,
            write_buffer_size: Some(64 * 1024 * 1024), // 64MB
            max_open_files: Some(1000),
        };
        
        let store = Arc::new(RocksDbDagStore::new(config));
        store.init().await.expect("Failed to initialize DAG store");
        
        Ok((tmp, store))
    }

    /// Helper function to create a test DAG node
    fn create_test_node(payload: &[u8], parents: Vec<String>, author: &str) -> SignedDagNode {
        let node = DagNode {
            payload: payload.to_vec(),
            parents,
            author: author.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            node_type: "test".to_string(),
            scope: "test:scope".to_string(),
        };
        
        SignedDagNode::new(node, None)
    }

    /// Helper function to create a node scope authorization
    fn create_test_scope(scope_id: &str, authorized_ids: Vec<&str>) -> ScopeAuthorization {
        let mut scope = ScopeAuthorization::new(scope_id.to_string());
        for id in authorized_ids {
            scope.add_identity(id.to_string());
        }
        scope
    }

    #[tokio::test]
    async fn test_duplicate_node_insertion() -> Result<()> {
        // Set up a temporary RocksDB instance
        let (tmp, store) = create_test_store().await?;

        // Create a dummy DAG node
        let node = create_test_node(
            b"duplicate",
            vec![],
            "did:example:alice"
        );
        
        // Define a scope with authorized identity
        let scope = create_test_scope("test:scope", vec!["did:example:alice"]);
        store.register_scope(scope).await?;

        // First insertion
        let node_id1 = store.append_node(node.clone()).await?;

        // Second insertion of the *same* node
        let node_id2 = store.append_node(node).await?;

        // They should be identical node IDs (idempotent)
        assert_eq!(node_id1, node_id2);

        // Retrieval should yield exactly one stored node
        let retrieved = store.get_node(&node_id1).await?.unwrap();
        assert_eq!(retrieved.payload, b"duplicate".to_vec());

        Ok(())
    }

    #[tokio::test]
    async fn test_lineage_verification_failure() -> Result<()> {
        // Set up a temporary RocksDB instance
        let (tmp, store) = create_test_store().await?;
        
        // Create authorized scope and unauthorized scope
        let auth_scope = create_test_scope("authorized:scope", vec!["did:example:alice"]);
        let unauth_scope = create_test_scope("unauthorized:scope", vec!["did:example:bob"]);
        
        // Register both scopes
        store.register_scope(auth_scope.clone()).await?;
        store.register_scope(unauth_scope.clone()).await?;
        
        // Create root node by Alice (authorized in auth_scope)
        let root_node = create_test_node(
            b"root-node",
            vec![],
            "did:example:alice"
        );
        
        // Add root node
        let root_id = store.append_node(root_node).await?;
        
        // Create child node by Bob (unauthorized in auth_scope)
        let child_node = create_test_node(
            b"child-node",
            vec![root_id.as_str().to_string()],
            "did:example:bob"
        );
        
        // Add child node
        let child_id = store.append_node(child_node).await?;
        
        // Verify lineage with authorized scope - should fail
        let auth_result = store.verify_lineage(&child_id, &auth_scope).await?;
        assert!(!auth_result, "Lineage verification should fail with unauthorized author");
        
        // Verify lineage with unauthorized scope - should pass (since bob is authorized in this scope)
        let unauth_result = store.verify_lineage(&child_id, &unauth_scope).await?;
        assert!(unauth_result, "Lineage verification should pass with authorized author");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_error_propagation_and_recovery() -> Result<()> {
        // Set up a temporary RocksDB instance
        let (tmp, store) = create_test_store().await?;
        
        // Create and register a scope
        let scope = create_test_scope("test:scope", vec!["did:example:alice"]);
        store.register_scope(scope.clone()).await?;
        
        // Create and add a node
        let node = create_test_node(
            b"test-node",
            vec![],
            "did:example:alice"
        );
        let node_id = store.append_node(node).await?;
        
        // Test retrieval of a non-existent node - should return None
        let non_existent_id = "non-existent-id";
        let result = store.get_node(&non_existent_id.to_string()).await?;
        assert!(result.is_none(), "Non-existent node should return None");
        
        // Test node existence check
        let exists = store.node_exists(&node_id).await?;
        assert!(exists, "Node should exist");
        
        let non_exists = store.node_exists(&non_existent_id.to_string()).await?;
        assert!(!non_exists, "Non-existent node should not exist");
        
        // Try to get children of a non-existent node - should return empty vector
        let children = store.get_children(&non_existent_id.to_string()).await?;
        assert!(children.is_empty(), "Non-existent node should have no children");
        
        // Close and reopen the store to test recovery
        store.close().await?;
        
        // Create a new store instance pointing to the same directory
        let config = ConnectionConfig {
            path: tmp.path().to_path_buf(),
            create_if_missing: false, // It should exist
            write_buffer_size: Some(64 * 1024 * 1024),
            max_open_files: Some(1000),
        };
        
        let reopened_store = Arc::new(RocksDbDagStore::new(config));
        reopened_store.init().await?;
        
        // Verify the node is still there
        let recovered_node = reopened_store.get_node(&node_id).await?;
        assert!(recovered_node.is_some(), "Node should be recovered after reopening");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_complex_lineage_verification() -> Result<()> {
        // Set up a temporary RocksDB instance
        let (tmp, store) = create_test_store().await?;
        
        // Create multiple scopes with different authorization rules
        let federation_scope = create_test_scope("federation:test", vec!["did:example:federation"]);
        let coop1_scope = create_test_scope("coop:test1", vec!["did:example:coop1", "did:example:member1"]);
        let coop2_scope = create_test_scope("coop:test2", vec!["did:example:coop2", "did:example:member2"]);
        
        // Register all scopes
        store.register_scope(federation_scope.clone()).await?;
        store.register_scope(coop1_scope.clone()).await?;
        store.register_scope(coop2_scope.clone()).await?;
        
        // Create a complex lineage:
        // Federation (root) -> Coop1 -> Member1 -> Member1Data
        //                   \-> Coop2 -> Member2 -> Member2Data
        
        // Federation root node
        let federation_node = create_test_node(
            b"federation-creation",
            vec![],
            "did:example:federation"
        );
        let federation_id = store.append_node(federation_node).await?;
        
        // Cooperative 1 node (child of federation)
        let coop1_node = create_test_node(
            b"coop1-creation",
            vec![federation_id.as_str().to_string()],
            "did:example:coop1"
        );
        let coop1_id = store.append_node(coop1_node).await?;
        
        // Cooperative 2 node (child of federation)
        let coop2_node = create_test_node(
            b"coop2-creation",
            vec![federation_id.as_str().to_string()],
            "did:example:coop2"
        );
        let coop2_id = store.append_node(coop2_node).await?;
        
        // Member 1 node (child of coop1)
        let member1_node = create_test_node(
            b"member1-creation",
            vec![coop1_id.as_str().to_string()],
            "did:example:member1"
        );
        let member1_id = store.append_node(member1_node).await?;
        
        // Member 2 node (child of coop2)
        let member2_node = create_test_node(
            b"member2-creation",
            vec![coop2_id.as_str().to_string()],
            "did:example:member2"
        );
        let member2_id = store.append_node(member2_node).await?;
        
        // Member 1 data node
        let member1_data_node = create_test_node(
            b"member1-data",
            vec![member1_id.as_str().to_string()],
            "did:example:member1"
        );
        let member1_data_id = store.append_node(member1_data_node).await?;
        
        // Member 2 data node
        let member2_data_node = create_test_node(
            b"member2-data",
            vec![member2_id.as_str().to_string()],
            "did:example:member2"
        );
        let member2_data_id = store.append_node(member2_data_node).await?;
        
        // Verify lineage with federation scope - should pass for all nodes
        assert!(store.verify_lineage(&federation_id, &federation_scope).await?, "Federation node should pass federation scope");
        assert!(store.verify_lineage(&coop1_id, &federation_scope).await?, "Coop1 node should pass federation scope");
        assert!(store.verify_lineage(&coop2_id, &federation_scope).await?, "Coop2 node should pass federation scope");
        assert!(store.verify_lineage(&member1_id, &federation_scope).await?, "Member1 node should pass federation scope");
        assert!(store.verify_lineage(&member2_id, &federation_scope).await?, "Member2 node should pass federation scope");
        assert!(store.verify_lineage(&member1_data_id, &federation_scope).await?, "Member1 data should pass federation scope");
        assert!(store.verify_lineage(&member2_data_id, &federation_scope).await?, "Member2 data should pass federation scope");
        
        // Verify lineage with coop1 scope - should pass only for coop1 and member1
        assert!(!store.verify_lineage(&federation_id, &coop1_scope).await?, "Federation node should not pass coop1 scope");
        assert!(store.verify_lineage(&coop1_id, &coop1_scope).await?, "Coop1 node should pass coop1 scope");
        assert!(!store.verify_lineage(&coop2_id, &coop1_scope).await?, "Coop2 node should not pass coop1 scope");
        assert!(store.verify_lineage(&member1_id, &coop1_scope).await?, "Member1 node should pass coop1 scope");
        assert!(!store.verify_lineage(&member2_id, &coop1_scope).await?, "Member2 node should not pass coop1 scope");
        assert!(store.verify_lineage(&member1_data_id, &coop1_scope).await?, "Member1 data should pass coop1 scope");
        assert!(!store.verify_lineage(&member2_data_id, &coop1_scope).await?, "Member2 data should not pass coop1 scope");
        
        // Verify lineage with coop2 scope - should pass only for coop2 and member2
        assert!(!store.verify_lineage(&federation_id, &coop2_scope).await?, "Federation node should not pass coop2 scope");
        assert!(!store.verify_lineage(&coop1_id, &coop2_scope).await?, "Coop1 node should not pass coop2 scope");
        assert!(store.verify_lineage(&coop2_id, &coop2_scope).await?, "Coop2 node should pass coop2 scope");
        assert!(!store.verify_lineage(&member1_id, &coop2_scope).await?, "Member1 node should not pass coop2 scope");
        assert!(store.verify_lineage(&member2_id, &coop2_scope).await?, "Member2 node should pass coop2 scope");
        assert!(!store.verify_lineage(&member1_data_id, &coop2_scope).await?, "Member1 data should not pass coop2 scope");
        assert!(store.verify_lineage(&member2_data_id, &coop2_scope).await?, "Member2 data should pass coop2 scope");
        
        Ok(())
    }
} 