#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocksdb_dag_store::{RocksDbDagStore, DagStore, NodeScope, ConnectionConfig};
    use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
    use icn_common::identity::{Identity, IdentityType, ScopedIdentity};
    use icn_common::verification::Signature;
    
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use ed25519_dalek::{Keypair, PublicKey, SecretKey};
    use tempfile::tempdir;
    use tokio::test;
    
    // Helper function to create a keypair
    fn create_keypair() -> (SecretKey, PublicKey) {
        let mut rng = rand::thread_rng();
        let keypair = Keypair::generate(&mut rng);
        (keypair.secret, keypair.public)
    }
    
    // Helper function to create an identity
    fn create_identity(name: &str, identity_type: IdentityType) -> (Identity, SecretKey) {
        let (secret_key, public_key) = create_keypair();
        
        let id = {
            let mut hasher = sha2::Sha256::new();
            hasher.update(public_key.as_bytes());
            hasher.update(name.as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };
        
        let identity = Identity {
            id,
            identity_type,
            name: name.to_string(),
            public_key: public_key.to_bytes().to_vec(),
            metadata: None,
        };
        
        (identity, secret_key)
    }
    
    // Helper function to create a scoped identity
    fn create_scoped_identity(
        name: &str, 
        identity_type: IdentityType, 
        scope: &str
    ) -> (ScopedIdentity, SecretKey) {
        let (identity, secret_key) = create_identity(name, identity_type);
        
        let scoped_identity = ScopedIdentity {
            identity,
            scope: scope.to_string(),
            scope_reference: None,
        };
        
        (scoped_identity, secret_key)
    }
    
    // Helper function to create a test DAG node
    fn create_test_node(
        node_type: DAGNodeType,
        parents: HashSet<DAGNodeID>,
        scope: &str,
        creator: &ScopedIdentity,
        private_key: &SecretKey,
        payload: serde_json::Value,
    ) -> DAGNode {
        let header = icn_common::dag::DAGNodeHeader {
            node_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            parents,
            scope: scope.to_string(),
            creator: creator.clone(),
        };
        
        let header_json = serde_json::to_vec(&header).unwrap();
        let payload_json = serde_json::to_vec(&payload).unwrap();
        
        let mut data_to_sign = header_json;
        data_to_sign.extend_from_slice(&payload_json);
        
        let keypair = ed25519_dalek::Keypair {
            secret: *private_key,
            public: PublicKey::from(private_key),
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        DAGNode {
            header,
            payload,
            signature,
        }
    }
    
    // Test initializing the store
    #[test]
    async fn test_init() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let config = ConnectionConfig {
            path: db_path,
            write_buffer_size: Some(8 * 1024 * 1024), // 8MB
            max_open_files: Some(100),
            create_if_missing: true,
        };
        
        let store = RocksDbDagStore::new(config);
        
        // Initialize the store
        assert!(store.init().await.is_ok());
        
        // Get metadata
        let metadata = store.get_metadata().await.unwrap();
        assert_eq!(metadata.node_count, 0);
        assert!(metadata.roots.is_empty());
        assert!(metadata.tips.is_empty());
    }
    
    // Test appending and retrieving nodes
    #[test]
    async fn test_append_and_retrieve_node() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let config = ConnectionConfig {
            path: db_path,
            write_buffer_size: Some(8 * 1024 * 1024), // 8MB
            max_open_files: Some(100),
            create_if_missing: true,
        };
        
        let store = RocksDbDagStore::new(config);
        store.init().await.unwrap();
        
        // Create a test federation scope
        let federation_scope = "federation:test";
        let mut scope = NodeScope::new(federation_scope.to_string());
        
        // Create a federation identity
        let (fed_identity, fed_secret) = create_scoped_identity(
            "Test Federation", 
            IdentityType::Federation, 
            federation_scope
        );
        
        // Add the federation identity to its scope
        scope.add_identity(fed_identity.identity.id.clone());
        store.register_scope(scope).await.unwrap();
        
        // Create a federation creation node
        let node = create_test_node(
            DAGNodeType::FederationCreation,
            HashSet::new(), // No parents (root node)
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({
                "name": "Test Federation",
                "founding_members": ["coop1", "coop2"]
            }),
        );
        
        // Append the node
        let node_id = store.append_node(node.clone()).await.unwrap();
        
        // Retrieve the node
        let retrieved_node = store.get_node(&node_id).await.unwrap().unwrap();
        
        // Verify the retrieved node matches the original
        assert_eq!(
            serde_json::to_string(&retrieved_node).unwrap(),
            serde_json::to_string(&node).unwrap()
        );
        
        // Verify metadata was updated
        let metadata = store.get_metadata().await.unwrap();
        assert_eq!(metadata.node_count, 1);
        assert_eq!(metadata.roots.len(), 1);
        assert!(metadata.roots.contains(&node_id));
        assert_eq!(metadata.tips.len(), 1);
        assert!(metadata.tips.contains(&node_id));
    }
    
    // Test lineage verification
    #[test]
    async fn test_lineage_verification() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let config = ConnectionConfig {
            path: db_path,
            write_buffer_size: Some(8 * 1024 * 1024), // 8MB
            max_open_files: Some(100),
            create_if_missing: true,
        };
        
        let store = RocksDbDagStore::new(config);
        store.init().await.unwrap();
        
        // Create a federation scope
        let federation_scope = "federation:test";
        let mut fed_scope = NodeScope::new(federation_scope.to_string());
        
        // Create a federation identity
        let (fed_identity, fed_secret) = create_scoped_identity(
            "Test Federation", 
            IdentityType::Federation, 
            federation_scope
        );
        
        // Add the federation identity to its scope
        fed_scope.add_identity(fed_identity.identity.id.clone());
        store.register_scope(fed_scope.clone()).await.unwrap();
        
        // Create a cooperative scope
        let coop_scope = "cooperative:test1";
        let mut coop_node_scope = NodeScope::new(coop_scope.to_string());
        
        // Set the federation as the parent scope
        let mut parent_scopes = HashSet::new();
        parent_scopes.insert(federation_scope.to_string());
        coop_node_scope.with_parent_scopes(parent_scopes);
        
        // Create a cooperative identity
        let (coop_identity, coop_secret) = create_scoped_identity(
            "Test Cooperative", 
            IdentityType::Cooperative, 
            coop_scope
        );
        
        // Add the cooperative identity to its scope
        coop_node_scope.add_identity(coop_identity.identity.id.clone());
        store.register_scope(coop_node_scope.clone()).await.unwrap();
        
        // Create a federation creation node
        let fed_node = create_test_node(
            DAGNodeType::FederationCreation,
            HashSet::new(), // No parents (root node)
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({
                "name": "Test Federation",
                "founding_members": ["Test Cooperative"]
            }),
        );
        
        // Append the federation node
        let fed_node_id = store.append_node(fed_node.clone()).await.unwrap();
        
        // Create a cooperative creation node with the federation node as parent
        let mut parents = HashSet::new();
        parents.insert(fed_node_id.clone());
        
        let coop_node = create_test_node(
            DAGNodeType::CooperativeCreation,
            parents, // Federation node as parent
            coop_scope,
            &coop_identity,
            &coop_secret,
            serde_json::json!({
                "name": "Test Cooperative",
                "members": ["member1", "member2"]
            }),
        );
        
        // Append the cooperative node
        let coop_node_id = store.append_node(coop_node.clone()).await.unwrap();
        
        // Verify the lineage of the federation node
        let result = store.verify_lineage(&fed_node_id, &fed_scope).await.unwrap();
        assert!(result, "Federation node lineage verification failed");
        
        // Verify the lineage of the cooperative node
        let result = store.verify_lineage(&coop_node_id, &coop_node_scope).await.unwrap();
        assert!(result, "Cooperative node lineage verification failed");
        
        // Test with a node that crosses scopes
        // The federation identity should still be able to verify the cooperative node
        // because the federation scope is a parent of the cooperative scope
        let result = store.verify_lineage(&coop_node_id, &fed_scope).await.unwrap();
        assert!(result, "Cross-scope lineage verification failed");
        
        // Test with an unauthorized identity
        let mut bad_scope = NodeScope::new("unauthorized:scope".to_string());
        bad_scope.add_identity("unauthorized_id".to_string());
        
        let result = store.verify_lineage(&fed_node_id, &bad_scope).await.unwrap();
        assert!(!result, "Unauthorized verification should fail");
    }
    
    // Test getting nodes by type
    #[test]
    async fn test_get_nodes_by_type() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let config = ConnectionConfig {
            path: db_path,
            write_buffer_size: Some(8 * 1024 * 1024), // 8MB
            max_open_files: Some(100),
            create_if_missing: true,
        };
        
        let store = RocksDbDagStore::new(config);
        store.init().await.unwrap();
        
        // Create a federation scope
        let federation_scope = "federation:test";
        let mut scope = NodeScope::new(federation_scope.to_string());
        
        // Create a federation identity
        let (fed_identity, fed_secret) = create_scoped_identity(
            "Test Federation", 
            IdentityType::Federation, 
            federation_scope
        );
        
        // Add the federation identity to its scope
        scope.add_identity(fed_identity.identity.id.clone());
        store.register_scope(scope).await.unwrap();
        
        // Create and append a federation creation node
        let fed_node = create_test_node(
            DAGNodeType::FederationCreation,
            HashSet::new(),
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({"name": "Test Federation"}),
        );
        
        let fed_node_id = store.append_node(fed_node).await.unwrap();
        
        // Create and append a proposal node
        let proposal_node = create_test_node(
            DAGNodeType::Proposal,
            HashSet::new(),
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({"title": "Test Proposal"}),
        );
        
        let proposal_node_id = store.append_node(proposal_node).await.unwrap();
        
        // Get nodes by type - FederationCreation
        let nodes = store.get_nodes_by_type(DAGNodeType::FederationCreation, None, None).await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].header.node_type, DAGNodeType::FederationCreation);
        
        // Get nodes by type - Proposal
        let nodes = store.get_nodes_by_type(DAGNodeType::Proposal, None, None).await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].header.node_type, DAGNodeType::Proposal);
        
        // Get nodes by type - Vote (should be empty)
        let nodes = store.get_nodes_by_type(DAGNodeType::Vote, None, None).await.unwrap();
        assert_eq!(nodes.len(), 0);
    }
    
    // Test getting children and parents
    #[test]
    async fn test_get_children_and_parents() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let config = ConnectionConfig {
            path: db_path,
            write_buffer_size: Some(8 * 1024 * 1024), // 8MB
            max_open_files: Some(100),
            create_if_missing: true,
        };
        
        let store = RocksDbDagStore::new(config);
        store.init().await.unwrap();
        
        // Create a federation scope
        let federation_scope = "federation:test";
        let mut scope = NodeScope::new(federation_scope.to_string());
        
        // Create a federation identity
        let (fed_identity, fed_secret) = create_scoped_identity(
            "Test Federation", 
            IdentityType::Federation, 
            federation_scope
        );
        
        // Add the federation identity to its scope
        scope.add_identity(fed_identity.identity.id.clone());
        store.register_scope(scope).await.unwrap();
        
        // Create and append a federation creation node
        let fed_node = create_test_node(
            DAGNodeType::FederationCreation,
            HashSet::new(),
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({"name": "Test Federation"}),
        );
        
        let fed_node_id = store.append_node(fed_node).await.unwrap();
        
        // Create and append a proposal node with the federation node as parent
        let mut parents = HashSet::new();
        parents.insert(fed_node_id.clone());
        
        let proposal_node = create_test_node(
            DAGNodeType::Proposal,
            parents,
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({"title": "Test Proposal"}),
        );
        
        let proposal_node_id = store.append_node(proposal_node).await.unwrap();
        
        // Create and append a vote node with the proposal node as parent
        let mut parents = HashSet::new();
        parents.insert(proposal_node_id.clone());
        
        let vote_node = create_test_node(
            DAGNodeType::Vote,
            parents,
            federation_scope,
            &fed_identity,
            &fed_secret,
            serde_json::json!({"vote": "approve"}),
        );
        
        let vote_node_id = store.append_node(vote_node).await.unwrap();
        
        // Get children of federation node
        let children = store.get_children(&fed_node_id).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].header.node_type, DAGNodeType::Proposal);
        
        // Get children of proposal node
        let children = store.get_children(&proposal_node_id).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].header.node_type, DAGNodeType::Vote);
        
        // Get children of vote node (should be empty)
        let children = store.get_children(&vote_node_id).await.unwrap();
        assert_eq!(children.len(), 0);
        
        // Get parents of vote node
        let parents = store.get_parents(&vote_node_id).await.unwrap();
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].header.node_type, DAGNodeType::Proposal);
        
        // Get parents of proposal node
        let parents = store.get_parents(&proposal_node_id).await.unwrap();
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].header.node_type, DAGNodeType::FederationCreation);
        
        // Get parents of federation node (should be empty)
        let parents = store.get_parents(&fed_node_id).await.unwrap();
        assert_eq!(parents.len(), 0);
    }
} 