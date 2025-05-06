use icn_types::dag::*;

#[test]
fn test_event_creation_and_hashing() {
    // Create a Genesis event
    let genesis_event = DagEvent::new(
        EventType::Genesis,
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        vec![],
        EventPayload::genesis("test-federation"),
    );
    
    // Calculate the hash
    let genesis_hash = merkle::calculate_event_hash(&genesis_event);
    println!("Genesis Hash: {}", genesis_hash);
    
    // Create a Proposal event that references the genesis
    let proposal_event = DagEvent::new(
        EventType::Proposal,
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        vec![genesis_hash.clone()],
        EventPayload::proposal("proposal-123", "bafyreihgmyh2srmmyiw7fdihrc2lw2bqdyxagrpvt2zk3aitq4hdxrhoei"),
    );
    
    // Calculate the hash
    let proposal_hash = merkle::calculate_event_hash(&proposal_event);
    println!("Proposal Hash: {}", proposal_hash);
    
    // Verify hashes are different
    assert_ne!(genesis_hash, proposal_hash);
}

#[test]
fn test_dag_node_creation() {
    // Create a Genesis event
    let genesis_event = DagEvent::new(
        EventType::Genesis,
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        vec![],
        EventPayload::genesis("test-federation"),
    );
    
    // Create a DAG node
    let genesis_node = DagNode::new(genesis_event, 0)
        .with_metadata("creator", "test-script")
        .with_cid("bafyreihgmyh2srmmyiw7fdihrc2lw2bqdyxagrpvt2zk3aitq4hdxrhzzz");
    
    // Verify the node's ID matches its content
    assert!(genesis_node.verify_id());
    
    // Access node data
    assert_eq!(genesis_node.height, 0);
    assert!(genesis_node.event().is_genesis());
    assert_eq!(
        genesis_node.metadata.get("creator"),
        Some(&"test-script".to_string())
    );
}

#[test]
fn test_merkle_root_calculation() {
    // Create some event IDs
    let id1 = EventId::new(b"event1");
    let id2 = EventId::new(b"event2");
    let id3 = EventId::new(b"event3");
    
    // Calculate Merkle root for a single event
    let root1 = merkle::calculate_merkle_root(&[id1.clone()]).unwrap();
    assert_eq!(root1, id1);
    
    // Calculate Merkle root for multiple events
    let root2 = merkle::calculate_merkle_root(&[id1, id2, id3]).unwrap();
    
    // The root should not be equal to any individual event ID
    assert_ne!(root2, id1);
    
    // Empty list should return None
    assert!(merkle::calculate_merkle_root(&[]).is_none());
} 