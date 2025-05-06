#![cfg(not(feature = "async"))] // Ensure this test runs synchronously

use super::*;
use crate::dag::memory::MemoryDagStore;
use crate::identity::{Did, DidKey, DidKeyError}; // Import necessary types
use ed25519_dalek::{SigningKey, Signer, VerifyingKey};
use rand::rngs::OsRng;
use std::collections::HashMap;
// Import underlying cid types for test_invalid_parent_refs
use ::cid::Cid as ExternalCid;
use ::cid::multihash::MultihashDigest;
use ::cid::multihash::Code as MultihashCode;
use ::cid::Version;
use ::cid::Codec;

// Helper function to create a signed node (Sync version)
fn create_signed_node(
    parents: Vec<Cid>,
    author: &Did,
    payload: DagPayload,
    signing_key: &SigningKey,
) -> SignedDagNode {
    let node = DagNodeBuilder::new() // Use new() without args
        .with_payload(payload) // Use with_*
        .with_parents(parents)
        .with_author(author.clone())
        .with_sequence(1) // Keep sequence for consistency maybe?
        .build()
        .expect("Failed to build node");

    let node_bytes = serde_ipld_dagcbor::to_vec(&node).unwrap();
    let signature = signing_key.sign(&node_bytes); // Use Signer trait

    SignedDagNode {
        node, // Correct fields
        signature, 
        cid: None, 
    }
}

// Simple PublicKeyResolver for testing (Sync version)
struct MockResolverSync {
    keys: HashMap<String, VerifyingKey>
}

impl MockResolverSync {
    fn new() -> Self { Self { keys: HashMap::new() } }
    fn add_key(&mut self, did: Did, key: VerifyingKey) {
        self.keys.insert(did.to_string(), key);
    }
}

impl PublicKeyResolver for MockResolverSync {
    fn resolve(&self, did: &Did) -> Result<VerifyingKey, DagError> {
        self.keys.get(&did.to_string())
            .cloned()
            .ok_or_else(|| DagError::PublicKeyResolutionError(did.clone(), "Key not found in mock resolver".to_string()))
    }
}

#[test]
fn test_dag_node_builder() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key); // Correct creation
    let payload = DagPayload::Raw(vec![1, 2, 3]);

    let node = DagNodeBuilder::new() // Use new() without args
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
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key); // Correct creation
    
    // Resolver needed for verify_branch
    let mut resolver = MockResolverSync::new();
    resolver.add_key(author.clone(), verifying_key);

    let genesis_payload = DagPayload::Raw(b"genesis".to_vec());
    let genesis_node = create_signed_node(vec![], &author, genesis_payload, &signing_key);

    // Use sync methods (no await), check Results
    let genesis_cid = dag_store.add_node(genesis_node.clone()).expect("add_node failed");

    let retrieved_node = dag_store.get_node(&genesis_cid).expect("get_node failed");
    assert_eq!(retrieved_node.node, genesis_node.node); // Compare inner node

    let tips = dag_store.get_tips().expect("get_tips failed");
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], genesis_cid);

    let child_payload = DagPayload::Raw(b"child".to_vec());
    let child_node = create_signed_node(
        vec![genesis_cid.clone()],
        &author,
        child_payload,
        &signing_key,
    );

    let child_cid = dag_store.add_node(child_node.clone()).expect("add child failed");

    let retrieved_child = dag_store.get_node(&child_cid).expect("get child failed");
    assert_eq!(retrieved_child.node.parents.len(), 1);
    assert_eq!(retrieved_child.node.parents[0], genesis_cid);
    assert_eq!(retrieved_child.node, child_node.node); // Compare inner node

    let tips_after = dag_store.get_tips().expect("get_tips after failed");
    assert_eq!(tips_after.len(), 1);
    assert_eq!(tips_after[0], child_cid);

    let ordered_nodes = dag_store.get_ordered_nodes().expect("get_ordered failed");
    assert_eq!(ordered_nodes.len(), 2);
    assert_eq!(ordered_nodes[0].node, genesis_node.node);
    assert_eq!(ordered_nodes[1].node, child_node.node);
    
    let author_nodes = dag_store.get_nodes_by_author(&author).expect("get_by_author failed");
    assert_eq!(author_nodes.len(), 2);
    assert!(author_nodes.iter().any(|n| n.node == genesis_node.node));
    assert!(author_nodes.iter().any(|n| n.node == child_node.node));
    
    let path = dag_store.find_path(&genesis_cid, &child_cid).expect("find_path failed");
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].node, genesis_node.node); 
    assert_eq!(path[1].node, child_node.node);
    
    // Pass resolver to verify_branch
    let result = dag_store.verify_branch(&child_cid, &resolver);
    assert!(result.is_ok()); // Check Ok() based on stub implementation
}

#[test]
fn test_dag_payload_types() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key); // Correct creation

    let raw_payload = DagPayload::Raw(b"raw data".to_vec());
    let raw_node = create_signed_node(vec![], &author, raw_payload, &signing_key);
    let raw_cid = dag_store.add_node(raw_node.clone()).unwrap();

    let json_payload = DagPayload::Json(serde_json::json!({ "key": "value" }));
    let json_node = create_signed_node(vec![raw_cid.clone()], &author, json_payload, &signing_key);
    let json_cid = dag_store.add_node(json_node.clone()).unwrap();

    let ref_payload = DagPayload::Reference(raw_cid.clone());
    let ref_node = create_signed_node(vec![json_cid.clone()], &author, ref_payload, &signing_key);
    let ref_cid = dag_store.add_node(ref_node.clone()).unwrap();

    let raw_nodes = dag_store.get_nodes_by_payload_type("raw").unwrap();
    assert_eq!(raw_nodes.len(), 1);
    assert_eq!(raw_nodes[0].node, raw_node.node); // Compare nodes

    let json_nodes = dag_store.get_nodes_by_payload_type("json").unwrap();
    assert_eq!(json_nodes.len(), 1);
    assert_eq!(json_nodes[0].node, json_node.node); // Compare nodes

    let ref_nodes = dag_store.get_nodes_by_payload_type("reference").unwrap();
    assert_eq!(ref_nodes.len(), 1);
    assert_eq!(ref_nodes[0].node, ref_node.node); // Compare nodes

    let ordered_nodes = dag_store.get_ordered_nodes().unwrap();
    assert_eq!(ordered_nodes.len(), 3);
    assert_eq!(ordered_nodes[0].node, raw_node.node);
    assert_eq!(ordered_nodes[1].node, json_node.node);
    assert_eq!(ordered_nodes[2].node, ref_node.node);
}

#[test]
fn test_invalid_parent_refs() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key); // Correct creation

    // Fix Cid creation using underlying types
    let mh = MultihashCode::Sha2_256.digest(b"non-existent");
    let external_cid = ExternalCid::new(Version::V1, Codec::Raw, mh).unwrap();
    let fake_cid = Cid::from(external_cid); // Wrap in our Cid type

    let payload = DagPayload::Raw(b"invalid parent".to_vec());
    let invalid_node = create_signed_node(vec![fake_cid.clone()], &author, payload, &signing_key);

    let result = dag_store.add_node(invalid_node);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DagError::ParentNotFound { parent, .. } if parent == fake_cid));
} 