use crate::dag::memory::MemoryDagStore;
use crate::dag::sync::memory::MemoryDAGSyncService;
use crate::dag::sync::network::{DAGSyncService, FederationPeer, VerificationResult, SyncError};
use crate::dag::{DagNode, DagStore, SignedDagNode, DagPayload};
use crate::identity::Did;
use icn_identity_core::DidKey;
use crate::cid::Cid;
use ed25519_dalek::{SigningKey, Signer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::dag::DagNodeBuilder;

// Helper to create a simple test node
fn create_test_signed_node(parents: Vec<Cid>, author: &Did, signing_key: &SigningKey) -> SignedDagNode {
    let node = DagNodeBuilder::new()
        .with_payload(DagPayload::Raw(b"test".to_vec()))
        .with_author(author.clone())
        .with_parents(parents)
        .build()
        .expect("Failed to build node");
        
    let node_bytes = serde_ipld_dagcbor::to_vec(&node).unwrap();
    let signature = signing_key.sign(&node_bytes);
    SignedDagNode {
        node,
        signature,
        cid: None,
    }
}

#[tokio::test]
async fn test_memory_dag_sync_service_peer_management() {
    let store1 = Arc::new(RwLock::new(MemoryDagStore::new()));
    let store2 = Arc::new(RwLock::new(MemoryDagStore::new()));

    // Correct instantiation using Arc<RwLock<MemoryDagStore>>
    let service1 = MemoryDAGSyncService::new(
        "peer1".to_string(), 
        "test-federation".to_string(), 
        store1.clone() // Pass Arc<RwLock<Store>>
    );
    let service2 = MemoryDAGSyncService::new(
        "peer2".to_string(), 
        "test-federation".to_string(), 
        store2.clone() // Pass Arc<RwLock<Store>>
    );

    // Create FederationPeer instances with correct fields
    let peer1_info = FederationPeer {
        peer_id: "peer1".to_string(),
        addresses: vec!["/memory/1".to_string()],
        last_seen: None,
        metadata: HashMap::new(), // Correct type
    };
    let peer2_info = FederationPeer {
        peer_id: "peer2".to_string(),
        addresses: vec!["/memory/2".to_string()],
        last_seen: None,
        metadata: HashMap::new(), // Correct type
    };

    // Use connect_peer from the DAGSyncService trait
    service1.connect_peer(&peer2_info).await.unwrap();
    service2.connect_peer(&peer1_info).await.unwrap();

    // Use discover_peers from the trait
    let discovered1 = service1.discover_peers().await.unwrap();
    assert_eq!(discovered1.len(), 1);
    assert_eq!(discovered1[0], peer2_info);

    let discovered2 = service2.discover_peers().await.unwrap();
    assert_eq!(discovered2.len(), 1);
    assert_eq!(discovered2[0], peer1_info);

    // Use disconnect_peer from the trait
    service1.disconnect_peer("peer2").await.unwrap();
    let discovered1_after_disconnect = service1.discover_peers().await.unwrap();
    assert!(discovered1_after_disconnect.is_empty());

     // Check peer2 still sees peer1 initially
     let discovered2_still_connected = service2.discover_peers().await.unwrap();
     assert_eq!(discovered2_still_connected.len(), 1); 
}

#[tokio::test]
async fn test_memory_dag_sync_service_verify_nodes() {
    let store = Arc::new(RwLock::new(MemoryDagStore::new()));
    let service = MemoryDAGSyncService::new(
        "peer1".to_string(), 
        "test-federation".to_string(), 
        store.clone()
    );
    
    // Need a DID and key to create a node
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::Ed25519(signing_key.verifying_key());
    // Correct Did creation
    let author = Did::parse(&did_key.to_string()).unwrap(); 

    let node = DagNodeBuilder::new(DagPayload::Raw(b"test".to_vec()))
        .author(author.clone())
        .parents(Vec::new())
        .federation("test-federation".to_string()) // Add matching federation id
        .build();
    
    let node_wrong_fed = DagNodeBuilder::new(DagPayload::Raw(b"wrong".to_vec()))
        .author(author.clone())
        .parents(Vec::new())
        .federation("wrong-federation".to_string()) // Add mismatching federation id
        .build();

    // Call verify_nodes with a slice of DagNode
    let result_ok = service.verify_nodes(&[node.clone()]).await;
    assert_eq!(result_ok, VerificationResult::Verified);

    let result_rejected = service.verify_nodes(&[node_wrong_fed.clone()]).await;
    assert!(matches!(result_rejected, VerificationResult::Rejected { .. }));
    
    let result_mixed = service.verify_nodes(&[node, node_wrong_fed]).await;
    assert!(matches!(result_mixed, VerificationResult::Rejected { .. }));
}

#[tokio::test]
async fn test_memory_dag_sync_service_sync_flow() {
    let store1_arc = Arc::new(RwLock::new(MemoryDagStore::new()));
    let store2_arc = Arc::new(RwLock::new(MemoryDagStore::new()));

    let service1 = MemoryDAGSyncService::new("peer1".to_string(), "test-federation".to_string(), store1_arc.clone());
    let service2 = MemoryDAGSyncService::new("peer2".to_string(), "test-federation".to_string(), store2_arc.clone());

    // Connect peers (though MemoryDAGSyncService doesn't strictly use this internally)
    let peer1_info = FederationPeer { peer_id: "peer1".to_string(), addresses: vec![], last_seen: None, metadata: HashMap::new() };
    let peer2_info = FederationPeer { peer_id: "peer2".to_string(), addresses: vec![], last_seen: None, metadata: HashMap::new() };
    service1.connect_peer(&peer2_info).await.unwrap();
    service2.connect_peer(&peer1_info).await.unwrap();

    // Create a node in store1
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::Ed25519(signing_key.verifying_key());
    let author = Did::parse(&did_key.to_string()).unwrap();
    
    let genesis_node = create_test_signed_node(Vec::new(), &author, &signing_key);
    let genesis_cid = store1_arc.write().await.add_node(genesis_node.clone()).await.unwrap();

    // --- Offer ---
    // Service 2 offers the genesis CID to Service 1 (who already has it)
    let needed_by_1 = service1.offer_nodes("peer2", &[genesis_cid.clone()]).await.unwrap();
    assert!(needed_by_1.is_empty()); // Service 1 doesn't need it

    // Service 1 offers the genesis CID to Service 2 (who needs it)
    let needed_by_2 = service2.offer_nodes("peer1", &[genesis_cid.clone()]).await.unwrap();
    assert!(needed_by_2.contains(&genesis_cid));
    assert_eq!(needed_by_2.len(), 1);

    // --- Accept Offer (implicitly done by offer_nodes in this impl) & Fetch ---
    // Service 2 fetches the needed CID from Service 1
    let fetched_bundle = service1.fetch_nodes("peer2", &vec![genesis_cid.clone()]).await.unwrap();
    
    // Verify fetched bundle
    assert_eq!(fetched_bundle.nodes.len(), 1);
    assert_eq!(fetched_bundle.nodes[0], genesis_node.node); // Compare DagNode
    assert_eq!(fetched_bundle.federation_id, "test-federation");
    assert_eq!(fetched_bundle.source_peer, Some("peer1".to_string()));

    // --- Store fetched node ---
    // Simulate Service 2 receiving and verifying the bundle (using verify_nodes)
    let verification_result = service2.verify_nodes(&fetched_bundle.nodes).await;
    assert_eq!(verification_result, VerificationResult::Verified);

    // If verified, Service 2 would store the nodes. Need to manually create SignedDagNode again for storing.
    // In a real scenario, the bundle might contain SignedDagNodes or enough info to reconstruct them.
    // For this test, we'll just reconstruct the one we know.
    let fetched_signed_node = SignedDagNode {
        node: fetched_bundle.nodes[0].clone(),
        auth: genesis_node.auth, // Re-use auth info from original node
        cid_cache: None,
    };
    store2_arc.write().await.add_node(fetched_signed_node).await.unwrap();

    // --- Verify Store 2 has the node ---
    let node_in_store2 = store2_arc.read().await.get_node(&genesis_cid).await;
    assert!(node_in_store2.is_ok());
    assert_eq!(node_in_store2.unwrap().node, genesis_node.node);
}


#[tokio::test]
async fn test_memory_dag_sync_service_offer_multiple_nodes() {
    let store1_arc = Arc::new(RwLock::new(MemoryDagStore::new()));
    let store2_arc = Arc::new(RwLock::new(MemoryDagStore::new()));

    let service1 = MemoryDAGSyncService::new("peer1".to_string(), "test-federation".to_string(), store1_arc.clone());
    let service2 = MemoryDAGSyncService::new("peer2".to_string(), "test-federation".to_string(), store2_arc.clone());
    
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::Ed25519(signing_key.verifying_key());
    let author = Did::parse(&did_key.to_string()).unwrap();

    // Store 1 has node A and B
    let node_a = create_test_signed_node(Vec::new(), &author, &signing_key);
    let cid_a = store1_arc.write().await.add_node(node_a.clone()).await.unwrap();
    let node_b = create_test_signed_node(vec![cid_a.clone()], &author, &signing_key);
    let cid_b = store1_arc.write().await.add_node(node_b.clone()).await.unwrap();

    // Store 2 has node A
    store2_arc.write().await.add_node(node_a.clone()).await.unwrap();
    
    // Service 1 offers A and B to Service 2
    let needed_by_2 = service2.offer_nodes("peer1", &[cid_a.clone(), cid_b.clone()]).await.unwrap();
    
    // Service 2 should only need B
    assert!(needed_by_2.contains(&cid_b));
    assert!(!needed_by_2.contains(&cid_a));
    assert_eq!(needed_by_2.len(), 1);
}