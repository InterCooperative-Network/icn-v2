use icn_identity_core::trustbundle::{
    TrustBundle, QuorumConfig, QuorumType, QuorumProof, TrustError
};
use icn_types::dag::{
    DagEvent, EventType, EventPayload, EventId, merkle::calculate_event_hash
};
use std::collections::HashMap;
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Signature};
use rand::rngs::OsRng;

// Helper function to create a key pair and return (did, signing_key, verifying_key)
fn create_key_pair() -> (String, SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    let did = format!("did:key:test{}", hex::encode(&verifying_key.to_bytes()[0..8]));
    
    (did, signing_key, verifying_key)
}

#[test]
fn test_trustbundle_creation_and_signing() {
    // Create some test keys
    let (did1, key1, vkey1) = create_key_pair();
    let (did2, key2, vkey2) = create_key_pair();
    let (did3, key3, vkey3) = create_key_pair();
    
    // Create test events
    let event1 = DagEvent::new(
        EventType::Genesis, 
        did1.clone(), 
        vec![], 
        EventPayload::genesis("test-federation")
    );
    
    let event2 = DagEvent::new(
        EventType::Proposal, 
        did2.clone(), 
        vec![calculate_event_hash(&event1)], 
        EventPayload::proposal("proposal-1", "test-cid")
    );
    
    // Create a quorum config requiring 2 of 3 signatures
    let quorum_config = QuorumConfig {
        quorum_type: QuorumType::Threshold(67), // 67% of 3 = 2
        participants: vec![did1.clone(), did2.clone(), did3.clone()],
    };
    
    // Create events to reference
    let events = vec![event1, event2];
    let event_ids: Vec<EventId> = events.iter()
        .map(|e| calculate_event_hash(e))
        .collect();
    
    // Create the TrustBundle
    let mut bundle = TrustBundle::new(
        "test-federation".to_string(),
        event_ids.clone(),
        quorum_config,
    );
    
    // Sign with key1
    bundle.sign(did1.clone(), |message| {
        let signature = key1.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Sign with key2
    bundle.sign(did2.clone(), |message| {
        let signature = key2.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Verify we have 2 signatures in the proof
    assert_eq!(bundle.proof.signatures.len(), 2);
    
    // Create a map of public keys for verification
    let mut public_keys = HashMap::new();
    public_keys.insert(did1.clone(), vkey1);
    public_keys.insert(did2.clone(), vkey2);
    public_keys.insert(did3.clone(), vkey3);
    
    // Verify the bundle
    let verify_result = bundle.verify(&events, &public_keys);
    assert!(verify_result.is_ok(), "Bundle verification failed: {:?}", verify_result);
}

#[test]
fn test_trustbundle_insufficient_quorum() {
    // Create some test keys
    let (did1, key1, vkey1) = create_key_pair();
    let (did2, _key2, vkey2) = create_key_pair();
    let (did3, _key3, vkey3) = create_key_pair();
    
    // Create test events
    let event1 = DagEvent::new(
        EventType::Genesis, 
        did1.clone(), 
        vec![], 
        EventPayload::genesis("test-federation")
    );
    
    // Create a quorum config requiring 2 of 3 signatures (67%)
    let quorum_config = QuorumConfig {
        quorum_type: QuorumType::Threshold(67),
        participants: vec![did1.clone(), did2.clone(), did3.clone()],
    };
    
    // Create events to reference
    let events = vec![event1];
    let event_ids: Vec<EventId> = events.iter()
        .map(|e| calculate_event_hash(e))
        .collect();
    
    // Create the TrustBundle
    let mut bundle = TrustBundle::new(
        "test-federation".to_string(),
        event_ids.clone(),
        quorum_config,
    );
    
    // Sign with only key1 (insufficient for quorum)
    bundle.sign(did1.clone(), |message| {
        let signature = key1.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Create a map of public keys for verification
    let mut public_keys = HashMap::new();
    public_keys.insert(did1.clone(), vkey1);
    public_keys.insert(did2.clone(), vkey2);
    public_keys.insert(did3.clone(), vkey3);
    
    // Verify the bundle - should fail with InsufficientQuorum
    let verify_result = bundle.verify(&events, &public_keys);
    assert!(verify_result.is_err());
    
    match verify_result {
        Err(TrustError::InsufficientQuorum { required, found }) => {
            assert_eq!(required, 2);
            assert_eq!(found, 1);
        },
        other => panic!("Expected InsufficientQuorum error, got: {:?}", other),
    }
}

#[test]
fn test_trustbundle_weighted_quorum() {
    // Create some test keys
    let (did1, key1, vkey1) = create_key_pair();
    let (did2, key2, vkey2) = create_key_pair();
    let (did3, _key3, vkey3) = create_key_pair();
    
    // Create test event
    let event1 = DagEvent::new(
        EventType::Genesis, 
        did1.clone(), 
        vec![], 
        EventPayload::genesis("test-federation")
    );
    
    // Create a weighted quorum config 
    // did1 has 60 weight, did2 has 30 weight, did3 has 10 weight
    // Total weight is 100, we need > 50 to pass
    let quorum_config = QuorumConfig {
        quorum_type: QuorumType::Weighted(vec![
            (did1.clone(), 60),
            (did2.clone(), 30),
            (did3.clone(), 10),
        ]),
        participants: vec![did1.clone(), did2.clone(), did3.clone()],
    };
    
    // Create events to reference
    let events = vec![event1];
    let event_ids: Vec<EventId> = events.iter()
        .map(|e| calculate_event_hash(e))
        .collect();
    
    // Create the TrustBundle
    let mut bundle = TrustBundle::new(
        "test-federation".to_string(),
        event_ids.clone(),
        quorum_config,
    );
    
    // Sign with key1 (60% weight)
    bundle.sign(did1.clone(), |message| {
        let signature = key1.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Create a map of public keys for verification
    let mut public_keys = HashMap::new();
    public_keys.insert(did1.clone(), vkey1);
    public_keys.insert(did2.clone(), vkey2);
    public_keys.insert(did3.clone(), vkey3);
    
    // Verify the bundle - should pass with just did1 (60% weight)
    let verify_result = bundle.verify(&events, &public_keys);
    assert!(verify_result.is_ok(), "Bundle verification failed: {:?}", verify_result);
    
    // Create a new bundle with did2 and did3 (40% weight) - should fail
    let mut bundle2 = TrustBundle::new(
        "test-federation".to_string(),
        event_ids.clone(),
        quorum_config,
    );
    
    // Sign with key2 (30% weight)
    bundle2.sign(did2.clone(), |message| {
        let signature = key2.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Verify the bundle - should fail due to insufficient weight
    let verify_result2 = bundle2.verify(&events, &public_keys);
    assert!(verify_result2.is_err());
}

#[test]
fn test_trustbundle_invalid_event() {
    // Create some test keys
    let (did1, key1, vkey1) = create_key_pair();
    let (did2, key2, vkey2) = create_key_pair();
    
    // Create test events
    let event1 = DagEvent::new(
        EventType::Genesis, 
        did1.clone(), 
        vec![], 
        EventPayload::genesis("test-federation")
    );
    
    // Create another event that won't be included in the verification set
    let event2 = DagEvent::new(
        EventType::Proposal, 
        did2.clone(), 
        vec![calculate_event_hash(&event1)], 
        EventPayload::proposal("proposal-1", "test-cid")
    );
    
    // Create a quorum config
    let quorum_config = QuorumConfig {
        quorum_type: QuorumType::All,
        participants: vec![did1.clone(), did2.clone()],
    };
    
    // Create event IDs for both events
    let event_ids = vec![
        calculate_event_hash(&event1),
        calculate_event_hash(&event2),
    ];
    
    // Create the TrustBundle
    let mut bundle = TrustBundle::new(
        "test-federation".to_string(),
        event_ids,
        quorum_config,
    );
    
    // Sign with both keys
    bundle.sign(did1.clone(), |message| {
        let signature = key1.sign(message);
        signature.to_bytes().to_vec()
    });
    
    bundle.sign(did2.clone(), |message| {
        let signature = key2.sign(message);
        signature.to_bytes().to_vec()
    });
    
    // Create a map of public keys for verification
    let mut public_keys = HashMap::new();
    public_keys.insert(did1.clone(), vkey1);
    public_keys.insert(did2.clone(), vkey2);
    
    // Verify with only event1 (event2 is missing)
    let events = vec![event1];
    let verify_result = bundle.verify(&events, &public_keys);
    
    // Should fail because event2 is referenced but not provided
    assert!(verify_result.is_err());
    match verify_result {
        Err(TrustError::InvalidEvent(_)) => {
            // Expected error
        },
        other => panic!("Expected InvalidEvent error, got: {:?}", other),
    }
} 