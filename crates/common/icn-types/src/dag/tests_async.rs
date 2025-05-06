#![cfg(feature = "async")]

use super::*;
use crate::dag::memory::MemoryDagStore;
use crate::dag::{DagNodeBuilder, DagPayload, SignedDagNode, DagError, PublicKeyResolver, DagStore};
use crate::identity::Did;
use icn_identity_core::DidKey;
use ed25519_dalek::{SigningKey, Signer, VerifyingKey};
use crate::cid::Cid;
use ::cid::Cid as ExternalCid;
use ::cid::multihash::MultihashDigest;
use ::cid::multihash::Code as MultihashCode;
use ::cid::Version;
use ::cid::Codec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Helper to create a simple test node
fn create_test_signed_node_async(parents: Vec<Cid>, author: &Did, signing_key: &SigningKey) -> SignedDagNode {
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

// Simple PublicKeyResolver for testing
struct MockResolver {
    keys: HashMap<String, VerifyingKey>
}

impl MockResolver {
    fn new() -> Self { Self { keys: HashMap::new() } }
    fn add_key(&mut self, did: Did, key: VerifyingKey) {
        self.keys.insert(did.to_string(), key);
    }
}

// Corrected and only PublicKeyResolver impl for MockResolver
impl PublicKeyResolver for MockResolver {
    fn resolve(&self, did: &Did) -> Result<VerifyingKey, DagError> {
        self.keys.get(&did.to_string())
            .cloned()
            .ok_or_else(|| DagError::PublicKeyResolutionError(did.clone(), "Key not found in mock resolver".to_string()))
    }
}

#[tokio::test]
async fn test_memory_dag_store_async_add_get() {
    let mut dag_store = MemoryDagStore::new();
    
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key);

    let genesis_node = create_test_signed_node_async(Vec::new(), &author, &signing_key);
    let genesis_cid = dag_store.add_node(genesis_node.clone()).await.unwrap();

    let retrieved_node = dag_store.get_node(&genesis_cid).await.unwrap();
    assert_eq!(retrieved_node.node, genesis_node.node);
}

#[tokio::test]
async fn test_memory_dag_store_async_tips_and_ordering() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key);

    let genesis_node = create_test_signed_node_async(Vec::new(), &author, &signing_key);
    let genesis_cid = dag_store.add_node(genesis_node.clone()).await.unwrap();

    let tips = dag_store.get_tips().await.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], genesis_cid);

    let child_node = create_test_signed_node_async(vec![genesis_cid.clone()], &author, &signing_key);
    let child_cid = dag_store.add_node(child_node.clone()).await.unwrap();

    let tips_after_child = dag_store.get_tips().await.unwrap();
    assert_eq!(tips_after_child.len(), 1);
    assert_eq!(tips_after_child[0], child_cid);

    let ordered_nodes = dag_store.get_ordered_nodes().await.unwrap();
    assert_eq!(ordered_nodes.len(), 2);
    assert_eq!(ordered_nodes[0].node, genesis_node.node);
    assert_eq!(ordered_nodes[1].node, child_node.node);
}

#[tokio::test]
async fn test_memory_dag_store_async_queries() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key);

    let genesis_node = create_test_signed_node_async(Vec::new(), &author, &signing_key);
    let genesis_cid = dag_store.add_node(genesis_node.clone()).await.unwrap();
    let child_node = create_test_signed_node_async(vec![genesis_cid.clone()], &author, &signing_key);
    let child_cid = dag_store.add_node(child_node.clone()).await.unwrap();

    let author_nodes = dag_store.get_nodes_by_author(&author).await.unwrap();
    assert_eq!(author_nodes.len(), 2);
    assert!(author_nodes.iter().any(|n| n.node == genesis_node.node));
    assert!(author_nodes.iter().any(|n| n.node == child_node.node));

    let raw_nodes = dag_store.get_nodes_by_payload_type("raw").await.unwrap();
    assert_eq!(raw_nodes.len(), 2);
    assert!(raw_nodes.iter().any(|n| n.node == genesis_node.node));
    assert!(raw_nodes.iter().any(|n| n.node == child_node.node));

    let json_nodes = dag_store.get_nodes_by_payload_type("json").await.unwrap();
    assert!(json_nodes.is_empty());

    let path = dag_store.find_path(&genesis_cid, &child_cid).await.unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].node, genesis_node.node);
    assert_eq!(path[1].node, child_node.node);
}

#[tokio::test]
async fn test_memory_dag_store_async_verify_branch() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key);

    let mut resolver = MockResolver::new();
    resolver.add_key(author.clone(), verifying_key);

    let genesis_node = create_test_signed_node_async(Vec::new(), &author, &signing_key);
    let genesis_cid = dag_store.add_node(genesis_node.clone()).await.unwrap();
    let child_node = create_test_signed_node_async(vec![genesis_cid.clone()], &author, &signing_key);
    let child_cid = dag_store.add_node(child_node.clone()).await.unwrap();

    let result = dag_store.verify_branch(&child_cid, &resolver).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_dag_store_async_invalid_parent() {
    let mut dag_store = MemoryDagStore::new();
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let author = Did::new(&verifying_key);

    let mh = MultihashCode::Sha2_256.digest(b"non-existent");
    let external_cid = ExternalCid::new(Version::V1, Codec::Raw, mh).unwrap();
    let non_existent_parent = Cid::from(external_cid);

    let invalid_node = create_test_signed_node_async(vec![non_existent_parent.clone()], &author, &signing_key);
    
    let result = dag_store.add_node(invalid_node).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DagError::ParentNotFound { parent, .. } => assert_eq!(parent, non_existent_parent),
        e => panic!("Expected ParentNotFound error, got {:?}", e),
    }
} 