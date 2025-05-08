// This file is temporarily renamed to .old.rs because it uses an outdated event-based API
// for DagStore and needs to be modernized to use SignedDagNode and the current DagStore trait.

use icn_runtime::dag_anchor::{anchor_execution_receipt, AnchorError};
use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionSubject, ExecutionScope, ExecutionStatus};
use icn_identity_core::did::DidKey;
use icn_types::dag::{DagStore, DagEvent, EventId, EventType, EventPayload, DagError};
use icn_types::{Cid, Did};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use multihash::{Code, MultihashDigest};
use cid::Cid as ExternalCid;
use icn_types::dag::SharedDagStore;
use icn_types::dag::memory::MemoryDagStore;

// --- Mock DagStore --- 

// --- Mock ExecutionReceipt --- 
fn mock_receipt(issuer_did_key: &DidKey) -> ExecutionReceipt {
    let node_did_key = DidKey::new();
    let subject = ExecutionSubject {
        id: node_did_key.did().to_string(),
        scope: ExecutionScope::Federation {
            federation_id: issuer_did_key.did().to_string(),
        },
        submitter: None,
        module_cid: Cid::default().to_string(),
        result_cid: Cid::default().to_string(),
        event_id: None,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };
    ExecutionReceipt::new(
        Uuid::new_v4().urn().to_string(),
        issuer_did_key.did().to_string(),
        subject,
    ).sign(issuer_did_key).unwrap() // Sign with the issuer key
}


#[tokio::test]
async fn test_anchor_execution_receipt_to_dag() {
    let store = SharedDagStore::new(Box::new(MemoryDagStore::new()));
    let issuer_key = DidKey::new();
    let receipt = mock_receipt(&issuer_key);
    let original_receipt_cid = receipt.to_cid().unwrap(); // Get CID for later check

    let trigger_event_bytes = [1u8; 32];
    let trigger_event_id = EventId(trigger_event_bytes);

    let anchor_result = anchor_execution_receipt(&receipt, &mut store, Some(trigger_event_id)).await;
    assert!(anchor_result.is_ok());
    let anchored_event_id = anchor_result.unwrap();

    // Verify the event was added to the store
    let stored_event_option = store.get_event(&anchored_event_id).await.unwrap();
    assert!(stored_event_option.is_some());
    let stored_event = stored_event_option.unwrap();

    // Check event details
    assert_eq!(stored_event.event_type, EventType::Receipt);
    assert_eq!(stored_event.author, receipt.issuer);
    assert!(stored_event.parent_events.contains(&trigger_event_id));

    match stored_event.payload {
        EventPayload::Receipt { receipt_cid } => {
            assert_eq!(receipt_cid, original_receipt_cid);
        }
        _ => panic!("Incorrect event payload type"),
    }

    // Check that InMemoryDagStore head was updated (simple check)
    let heads = store.get_head_cids().await.unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0], Cid::from(anchored_event_id));
}

#[tokio::test]
async fn test_anchor_without_triggering_event() {
    let store = SharedDagStore::new(Box::new(MemoryDagStore::new()));
    let issuer_key = DidKey::new();
    let receipt = mock_receipt(&issuer_key);

    // Simulate an existing event to be picked as parent by get_head_cids
    let initial_event = DagEvent::new(EventType::Genesis, issuer_key.did().to_string(), vec![], EventPayload::Custom { fields: serde_json::Value::Null });
    let initial_event_id = store.add_event(initial_event).await.unwrap();

    let anchor_result = anchor_execution_receipt(&receipt, &mut store, None).await;
    assert!(anchor_result.is_ok());
    let anchored_event = store.get_event(&anchor_result.unwrap()).await.unwrap().unwrap();
    
    assert!(anchored_event.parent_events.contains(&initial_event_id));
} 