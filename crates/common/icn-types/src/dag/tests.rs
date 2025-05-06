use super::*;
use crate::dag::memory::MemoryDagStore;
use crate::identity::Did;
use chrono::Utc;
use ed25519_dalek::{Signature, SigningKey};
use rand::rngs::OsRng;

// Helper function to create a signed node
fn create_signed_node(
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

#[test]
fn test_dag_node_builder() {
    let author = Did::from("did:example:123".to_string());
    let payload = DagPayload::Raw(vec![1, 2, 3]);

    let node = DagNodeBuilder::new()
        .with_payload(payload.clone())
        .with_author(author.clone())
        .with_sequence(42)
        .with_federation_id("test-federation".to_string())
        .with_label("test-label".to_string())
        .build()
        .unwrap();

    assert_eq!(node.payload, payload);
    assert_eq!(node.author, author);
    assert_eq!(node.metadata.sequence, Some(42));
    assert_eq!(node.metadata.federation_id, Some("test-federation".to_string()));
    assert_eq!(node.metadata.labels, Some(vec!["test-label".to_string()]));
    assert!(node.parents.is_empty());
}

#[test]
fn test_memory_dag_store() {
    // Create a new in-memory DAG store
    let mut dag_store = MemoryDagStore::new();

    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Create a DID
    let author = Did::from("did:example:123".to_string());

    // Create a genesis node
    let genesis_payload = DagPayload::Raw(b"genesis".to_vec());
    let genesis_node = create_signed_node(vec![], author.clone(), genesis_payload, &signing_key);

    // Add the genesis node to the store
    let genesis_cid = dag_store.add_node(genesis_node.clone()).unwrap();

    // Verify the node was added
    let retrieved_node = dag_store.get_node(&genesis_cid).unwrap();
    assert_eq!(retrieved_node.node.author, author);

    // Verify it's a tip
    let tips = dag_store.get_tips().unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], genesis_cid);

    // Add a child node
    let child_payload = DagPayload::Raw(b"child".to_vec());
    let child_node = create_signed_node(
        vec![genesis_cid.clone()],
        author.clone(),
        child_payload,
        &signing_key,
    );

    let child_cid = dag_store.add_node(child_node).unwrap();

    // Verify the child was added
    let retrieved_child = dag_store.get_node(&child_cid).unwrap();
    assert_eq!(retrieved_child.node.parents.len(), 1);
    assert_eq!(retrieved_child.node.parents[0], genesis_cid);

    // Verify tips have been updated
    let tips = dag_store.get_tips().unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], child_cid);

    // Verify ordering
    let ordered_nodes = dag_store.get_ordered_nodes().unwrap();
    assert_eq!(ordered_nodes.len(), 2);
    assert_eq!(ordered_nodes[0].node.payload, genesis_node.node.payload);
    
    // Get nodes by author
    let author_nodes = dag_store.get_nodes_by_author(&author).unwrap();
    assert_eq!(author_nodes.len(), 2);
    
    // Find path
    let path = dag_store.find_path(&genesis_cid, &child_cid).unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].cid.as_ref().unwrap(), &genesis_cid);
    assert_eq!(path[1].cid.as_ref().unwrap(), &child_cid);
    
    // Verify branch
    let is_valid = dag_store.verify_branch(&child_cid).unwrap();
    assert!(is_valid);
}

#[test]
fn test_dag_payload_types() {
    // Create a new in-memory DAG store
    let mut dag_store = MemoryDagStore::new();

    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Create a DID
    let author = Did::from("did:example:123".to_string());

    // Create nodes with different payload types
    let raw_payload = DagPayload::Raw(b"raw data".to_vec());
    let raw_node = create_signed_node(vec![], author.clone(), raw_payload, &signing_key);
    let raw_cid = dag_store.add_node(raw_node).unwrap();

    let json_payload = DagPayload::Json(serde_json::json!({ "key": "value" }));
    let json_node = create_signed_node(vec![raw_cid.clone()], author.clone(), json_payload, &signing_key);
    let json_cid = dag_store.add_node(json_node).unwrap();

    let ref_payload = DagPayload::Reference(raw_cid.clone());
    let ref_node = create_signed_node(vec![json_cid.clone()], author.clone(), ref_payload, &signing_key);
    let ref_cid = dag_store.add_node(ref_node).unwrap();

    // Get nodes by payload type
    let raw_nodes = dag_store.get_nodes_by_payload_type("raw").unwrap();
    assert_eq!(raw_nodes.len(), 1);
    assert_eq!(raw_nodes[0].cid.as_ref().unwrap(), &raw_cid);

    let json_nodes = dag_store.get_nodes_by_payload_type("json").unwrap();
    assert_eq!(json_nodes.len(), 1);
    assert_eq!(json_nodes[0].cid.as_ref().unwrap(), &json_cid);

    let ref_nodes = dag_store.get_nodes_by_payload_type("reference").unwrap();
    assert_eq!(ref_nodes.len(), 1);
    assert_eq!(ref_nodes[0].cid.as_ref().unwrap(), &ref_cid);

    // Verify topological order
    let ordered_nodes = dag_store.get_ordered_nodes().unwrap();
    assert_eq!(ordered_nodes.len(), 3);
    
    // The raw node should come before the json node, which should come before the ref node
    assert_eq!(ordered_nodes[0].cid.as_ref().unwrap(), &raw_cid);
    assert_eq!(ordered_nodes[1].cid.as_ref().unwrap(), &json_cid);
    assert_eq!(ordered_nodes[2].cid.as_ref().unwrap(), &ref_cid);
}

#[test]
fn test_invalid_parent_refs() {
    // Create a new in-memory DAG store
    let mut dag_store = MemoryDagStore::new();

    // Create a signing key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Create a DID
    let author = Did::from("did:example:123".to_string());

    // Create a random CID that doesn't exist in the DAG
    let fake_cid = Cid::from_bytes(b"fake_cid").unwrap();

    // Try to create a node with a non-existent parent
    let payload = DagPayload::Raw(b"invalid parent".to_vec());
    let invalid_node = create_signed_node(vec![fake_cid], author, payload, &signing_key);

    // Adding the node should fail
    let result = dag_store.add_node(invalid_node);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DagError::InvalidParentRefs));
} 