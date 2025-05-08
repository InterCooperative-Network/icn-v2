use icn_identity_core::trustbundle::{
    TrustBundle, QuorumConfig, QuorumType, TrustBundleStore, MemoryTrustBundleStore
};
use icn_types::dag::EventId;
use tokio::runtime::Runtime;
use std::collections::HashMap;
use rand::rngs::OsRng;
use ed25519_dalek::{SigningKey, Signer};

// Helper function to create a key pair
fn create_key_pair() -> (String, SigningKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let did = format!("did:key:test{}", hex::encode(&signing_key.verifying_key().to_bytes()[0..8]));
    
    (did, signing_key)
}

// Helper function to create a test TrustBundle
fn create_test_bundle(federation_id: &str, num_events: usize) -> TrustBundle {
    // Create test DID/keys
    let (did1, _) = create_key_pair();
    let (did2, _) = create_key_pair();
    let (did3, _) = create_key_pair();
    
    // Create a quorum config
    let quorum_config = QuorumConfig {
        quorum_type: QuorumType::Threshold(67), // 67% threshold
        participants: vec![did1.clone(), did2.clone(), did3.clone()],
    };
    
    // Create test event IDs
    let mut event_ids = Vec::new();
    for i in 0..num_events {
        let mut bytes = [0u8; 32];
        bytes[0] = i as u8;
        bytes[1] = (i >> 8) as u8;
        let event_id = EventId(bytes);
        event_ids.push(event_id);
    }
    
    // Create the bundle
    let bundle = TrustBundle::new(
        federation_id.to_string(),
        event_ids,
        quorum_config,
    );
    
    bundle
}

#[test]
fn test_memory_storage_store_and_get() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create a memory store
        let store = MemoryTrustBundleStore::new();
        
        // Create a test bundle
        let bundle = create_test_bundle("test-federation", 3);
        
        // Store the bundle
        let bundle_id = store.store(bundle.clone()).await.unwrap();
        
        // Retrieve the bundle
        let retrieved_bundle = store.get(&bundle_id).await.unwrap();
        
        // Verify it's the same
        assert_eq!(retrieved_bundle.federation_id, bundle.federation_id);
        assert_eq!(retrieved_bundle.referenced_events.len(), bundle.referenced_events.len());
        assert_eq!(retrieved_bundle.timestamp, bundle.timestamp);
    });
}

#[test]
fn test_memory_storage_federation_operations() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create a memory store
        let store = MemoryTrustBundleStore::new();
        
        // Create test bundles for two federations
        let bundle1 = create_test_bundle("federation-1", 1);
        let bundle2 = create_test_bundle("federation-1", 2);
        let bundle3 = create_test_bundle("federation-2", 1);
        
        // Store the bundles
        let id1 = store.store(bundle1).await.unwrap();
        let id2 = store.store(bundle2).await.unwrap();
        let _id3 = store.store(bundle3).await.unwrap();
        
        // List bundles for federation-1
        let federation1_bundles = store.list_by_federation("federation-1").await.unwrap();
        assert_eq!(federation1_bundles.len(), 2);
        
        // Get latest bundle for federation-1
        let latest = store.get_latest("federation-1").await.unwrap();
        // The latest one should be bundle2 (was added last)
        assert_eq!(latest.referenced_events.len(), 2);
        
        // Delete a bundle
        store.delete(&id1).await.unwrap();
        
        // Check federation-1 now has only one bundle
        let federation1_bundles = store.list_by_federation("federation-1").await.unwrap();
        assert_eq!(federation1_bundles.len(), 1);
        assert_eq!(store.get(&id2).await.unwrap().referenced_events.len(), 2);
        
        // Verify get_latest still works after deletion
        let latest = store.get_latest("federation-1").await.unwrap();
        assert_eq!(latest.referenced_events.len(), 2);
    });
}

#[test]
fn test_memory_storage_errors() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create a memory store
        let store = MemoryTrustBundleStore::new();
        
        // Try to get a non-existent bundle
        let result = store.get("non-existent").await;
        assert!(result.is_err());
        
        // Try to get the latest bundle for a non-existent federation
        let result = store.get_latest("non-existent").await;
        assert!(result.is_err());
        
        // Try to list bundles for a non-existent federation
        let result = store.list_by_federation("non-existent").await;
        assert!(result.is_err());
        
        // Try to delete a non-existent bundle
        let result = store.delete("non-existent").await;
        assert!(result.is_err());
    });
} 