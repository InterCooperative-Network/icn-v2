use super::*;
use crate::dag::memory::MemoryDagStore;
use crate::dag::sync::memory::MemoryDAGSyncService;
use crate::dag::{DagNodeBuilder, DagPayload, SignedDagNode};
use crate::identity::Did;
use ed25519_dalek::{SigningKey, Signature};
use rand::rngs::OsRng;

// Helper function to create a signed node
fn create_signed_node(
    parents: Vec<crate::cid::Cid>,
    author: Did,
    payload: DagPayload,
    federation_id: Option<String>,
    signing_key: &SigningKey,
) -> SignedDagNode {
    // Create a node with a builder
    let mut builder = DagNodeBuilder::new()
        .with_payload(payload)
        .with_parents(parents)
        .with_author(author)
        .with_sequence(1);
        
    if let Some(fed_id) = federation_id {
        builder = builder.with_federation_id(fed_id);
    }
    
    let node = builder.build().unwrap();

    // Serialize for signing
    let node_bytes = serde_json::to_vec(&node).unwrap();

    // Sign the node
    let signature = signing_key.sign(&node_bytes);

    // Create a signed node
    SignedDagNode {
        node,
        signature,
        cid: None, // Will be computed when added to the DAG
    }
}

#[test]
fn test_memory_dag_sync_service_basics() {
    // Create DAG stores for two peers
    let store1 = MemoryDagStore::new();
    let store2 = MemoryDagStore::new();
    
    // Create sync services for two peers
    let mut service1 = MemoryDAGSyncService::new(
        store1, 
        "test-federation".to_string(), 
        "peer1".to_string()
    );
    
    let mut service2 = MemoryDAGSyncService::new(
        store2, 
        "test-federation".to_string(), 
        "peer2".to_string()
    );
    
    // Add each peer to the other's known peers
    let peer1 = FederationPeer {
        id: "peer1".to_string(),
        endpoint: "http://peer1.test".to_string(),
        federation_id: "test-federation".to_string(),
        metadata: None,
    };
    
    let peer2 = FederationPeer {
        id: "peer2".to_string(),
        endpoint: "http://peer2.test".to_string(),
        federation_id: "test-federation".to_string(),
        metadata: None,
    };
    
    service1.add_peer(peer2.clone(), 100);
    service2.add_peer(peer1.clone(), 100);
    
    // Verify the peers were added
    assert_eq!(service1.get_peer("peer2").unwrap().endpoint, "http://peer2.test");
    assert_eq!(service2.get_peer("peer1").unwrap().endpoint, "http://peer1.test");
    
    // Test trust levels
    assert_eq!(service1.get_peer_trust("peer2").unwrap(), 100);
    assert_eq!(service2.get_peer_trust("peer1").unwrap(), 100);
}

#[test]
fn test_dag_sync_bundle_verification() {
    // Create a DAG store
    let store = MemoryDagStore::new();
    
    // Create a sync service
    let service = MemoryDAGSyncService::new(
        store, 
        "test-federation".to_string(), 
        "peer1".to_string()
    );
    
    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    
    // Create a DID
    let author = Did::from("did:example:123".to_string());
    
    // Create a node
    let node = create_signed_node(
        vec![],
        author,
        DagPayload::Raw(b"test data".to_vec()),
        Some("test-federation".to_string()),
        &signing_key,
    );
    
    // Create a bundle
    let bundle = DAGSyncBundle {
        nodes: vec![node],
        federation_id: "test-federation".to_string(),
        source_peer: Some("peer1".to_string()),
        timestamp: chrono::Utc::now(),
    };
    
    // Verify the bundle
    let result = service.verify_bundle(&bundle).unwrap();
    
    // The verification should pass for basic checks, but the node will be rejected
    // because the parents don't exist yet (there are none in this case)
    assert!(result.rejected_nodes.is_empty());
    assert!(!result.accepted_nodes.is_empty());
}

#[test]
fn test_dag_sync_with_different_federation() {
    // Create a DAG store
    let store = MemoryDagStore::new();
    
    // Create a sync service
    let service = MemoryDAGSyncService::new(
        store, 
        "test-federation".to_string(), 
        "peer1".to_string()
    );
    
    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    
    // Create a DID
    let author = Did::from("did:example:123".to_string());
    
    // Create a node with a different federation ID
    let node = create_signed_node(
        vec![],
        author,
        DagPayload::Raw(b"test data".to_vec()),
        Some("other-federation".to_string()),
        &signing_key,
    );
    
    // Create a bundle
    let bundle = DAGSyncBundle {
        nodes: vec![node],
        federation_id: "test-federation".to_string(), // Bundle federation is correct
        source_peer: Some("peer1".to_string()),
        timestamp: chrono::Utc::now(),
    };
    
    // Verify the bundle
    let result = service.verify_bundle(&bundle).unwrap();
    
    // The node should be rejected because of federation ID mismatch
    assert_eq!(result.rejected_nodes.len(), 1);
    assert!(result.accepted_nodes.is_empty());
    assert!(result.rejected_nodes[0].1.contains("federation ID"));
}

#[test]
fn test_accept_bundle() {
    // Create DAG stores
    let store1 = MemoryDagStore::new();
    let store2 = MemoryDagStore::new();
    
    // Create sync services
    let mut service1 = MemoryDAGSyncService::new(
        store1, 
        "test-federation".to_string(), 
        "peer1".to_string()
    );
    
    let mut service2 = MemoryDAGSyncService::new(
        store2, 
        "test-federation".to_string(), 
        "peer2".to_string()
    );
    
    // Add each peer to the other's known peers
    let peer1 = FederationPeer {
        id: "peer1".to_string(),
        endpoint: "http://peer1.test".to_string(),
        federation_id: "test-federation".to_string(),
        metadata: None,
    };
    
    let peer2 = FederationPeer {
        id: "peer2".to_string(),
        endpoint: "http://peer2.test".to_string(),
        federation_id: "test-federation".to_string(),
        metadata: None,
    };
    
    service1.add_peer(peer2.clone(), 100);
    service2.add_peer(peer1.clone(), 100);
    
    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    
    // Create a DID
    let author = Did::from("did:example:123".to_string());
    
    // Create and add a genesis node to service1
    let genesis_node = create_signed_node(
        vec![],
        author.clone(),
        DagPayload::Raw(b"genesis".to_vec()),
        Some("test-federation".to_string()),
        &signing_key,
    );
    
    // Add the node to service1's store
    let dag_store1 = service1.dag_store.write().unwrap();
    let genesis_cid = dag_store1.add_node(genesis_node.clone()).unwrap();
    drop(dag_store1);
    
    // Create a bundle from service1 to send to service2
    let bundle = DAGSyncBundle {
        nodes: vec![genesis_node],
        federation_id: "test-federation".to_string(),
        source_peer: Some("peer1".to_string()),
        timestamp: chrono::Utc::now(),
    };
    
    // Service2 accepts the bundle
    let result = service2.accept_bundle(bundle).unwrap();
    
    // The node should be accepted
    assert_eq!(result.accepted_nodes.len(), 1);
    assert_eq!(result.accepted_nodes[0], genesis_cid);
    assert!(result.rejected_nodes.is_empty());
    
    // Verify the node was added to service2's store
    let dag_store2 = service2.dag_store.read().unwrap();
    let retrieved_node = dag_store2.get_node(&genesis_cid).unwrap();
    assert_eq!(retrieved_node.node.author, author);
} 