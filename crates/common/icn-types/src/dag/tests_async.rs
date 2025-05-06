#![cfg(feature = "async")]

use super::*;
use crate::dag::memory::MemoryDagStore;
use crate::identity::Did;
use chrono::Utc;
use ed25519_dalek::{Signature, SigningKey};
use rand::rngs::OsRng;

// Helper function to create a signed node
async fn create_signed_node_async(
    parents: Vec<Cid>,
    author: Did,
    payload: DagPayload,
    signing_key: &SigningKey,
) -> SignedDagNode {
    // Create a node
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_parents(parents)
        .with_author(author)
        .with_sequence(1)
        .build()
        .unwrap();

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

#[tokio::test]
async fn test_memory_dag_store_async() {
    // Create a new in-memory DAG store
    let mut dag_store = MemoryDagStore::new();

    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Create a DID
    let author = Did::from("did:example:123".to_string());

    // Create a genesis node
    let genesis_payload = DagPayload::Raw(b"genesis".to_vec());
    let genesis_node = create_signed_node_async(vec![], author.clone(), genesis_payload, &signing_key).await;

    // Add the genesis node to the store
    let genesis_cid = dag_store.add_node(genesis_node.clone()).await.unwrap();

    // Verify the node was added
    let retrieved_node = dag_store.get_node(&genesis_cid).await.unwrap();
    assert_eq!(retrieved_node.node.author, author);

    // Verify it's a tip
    let tips = dag_store.get_tips().await.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], genesis_cid);

    // Add a child node
    let child_payload = DagPayload::Raw(b"child".to_vec());
    let child_node = create_signed_node_async(
        vec![genesis_cid.clone()],
        author.clone(),
        child_payload,
        &signing_key,
    ).await;

    let child_cid = dag_store.add_node(child_node).await.unwrap();

    // Verify the child was added
    let retrieved_child = dag_store.get_node(&child_cid).await.unwrap();
    assert_eq!(retrieved_child.node.parents.len(), 1);
    assert_eq!(retrieved_child.node.parents[0], genesis_cid);

    // Verify tips have been updated
    let tips = dag_store.get_tips().await.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], child_cid);

    // Verify ordering
    let ordered_nodes = dag_store.get_ordered_nodes().await.unwrap();
    assert_eq!(ordered_nodes.len(), 2);
    assert_eq!(ordered_nodes[0].node.payload, genesis_node.node.payload);
    
    // Get nodes by author
    let author_nodes = dag_store.get_nodes_by_author(&author).await.unwrap();
    assert_eq!(author_nodes.len(), 2);
    
    // Find path
    let path = dag_store.find_path(&genesis_cid, &child_cid).await.unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].cid.as_ref().unwrap(), &genesis_cid);
    assert_eq!(path[1].cid.as_ref().unwrap(), &child_cid);
    
    // Verify branch
    let is_valid = dag_store.verify_branch(&child_cid).await.unwrap();
    assert!(is_valid);
}

#[tokio::test]
async fn test_concurrent_dag_operations() {
    // Create a new in-memory DAG store
    let store = MemoryDagStore::new();
    let store_arc = std::sync::Arc::new(tokio::sync::Mutex::new(store));

    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Create a DID
    let author = Did::from("did:example:123".to_string());

    // Create a genesis node
    let genesis_payload = DagPayload::Raw(b"genesis".to_vec());
    let genesis_node = create_signed_node_async(vec![], author.clone(), genesis_payload, &signing_key).await;

    // Add the genesis node to the store
    let genesis_cid = {
        let mut store = store_arc.lock().await;
        store.add_node(genesis_node.clone()).await.unwrap()
    };

    // Create multiple child nodes concurrently
    let mut handles = Vec::new();
    let node_count = 10;

    for i in 0..node_count {
        let store_clone = store_arc.clone();
        let author_clone = author.clone();
        let genesis_cid_clone = genesis_cid.clone();
        let signing_key_clone = signing_key.clone();
        
        let handle = tokio::spawn(async move {
            // Create a child node
            let payload = DagPayload::Raw(format!("child-{}", i).into_bytes());
            let child_node = create_signed_node_async(
                vec![genesis_cid_clone],
                author_clone,
                payload,
                &signing_key_clone,
            ).await;

            // Add the node to the store
            let mut store = store_clone.lock().await;
            store.add_node(child_node).await
        });
        
        handles.push(handle);
    }

    // Wait for all nodes to be added
    let results: Vec<Result<_, _>> = futures::future::join_all(handles).await;
    
    // Verify all nodes were added successfully
    for result in results {
        assert!(result.unwrap().is_ok());
    }

    // Verify the tips count
    let tips = {
        let store = store_arc.lock().await;
        store.get_tips().await.unwrap()
    };
    assert_eq!(tips.len(), node_count);

    // Verify the total node count (genesis + children)
    let all_nodes = {
        let store = store_arc.lock().await;
        store.get_ordered_nodes().await.unwrap()
    };
    assert_eq!(all_nodes.len(), node_count + 1);
} 