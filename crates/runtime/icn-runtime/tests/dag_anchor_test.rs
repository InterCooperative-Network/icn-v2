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

// --- Mock DagStore --- 
#[derive(Default, Clone)]
struct InMemoryDagStore {
    events: Arc<Mutex<HashMap<EventId, DagEvent>>>,
    head_cids: Arc<Mutex<Vec<Cid>>>, // Using Cid as EventId for heads
}

impl InMemoryDagStore {
    fn new() -> Self {
        Default::default()
    }
}

#[async_trait]
impl DagStore for InMemoryDagStore {
    async fn add_event(&mut self, event: DagEvent) -> Result<EventId, DagError> {
        // For simplicity, let's assume EventId can be derived from the event content (e.g., its CID)
        // Or, if DagEvent had an `id()` method that computes its CID.
        // Here, we'll just use a placeholder or hash its CBOR representation.
        let event_cbor = serde_ipld_dagcbor::to_vec(&event).map_err(|e| DagError::Serialization(e.to_string()))?;
        let hash = multihash::Code::Sha2_256.digest(&event_cbor);
        let cid = Cid::new_v1(0x71, hash); // 0x71 is DAG-CBOR codec
        let event_id = EventId::from(cid);

        let mut events_guard = self.events.lock().unwrap();
        events_guard.insert(event_id, event);
        
        let mut heads_guard = self.head_cids.lock().unwrap();
        heads_guard.clear(); // Simple head management: new event is the only head
        heads_guard.push(cid);

        Ok(event_id)
    }

    async fn get_event(&self, id: &EventId) -> Result<Option<DagEvent>, DagError> {
        let events_guard = self.events.lock().unwrap();
        Ok(events_guard.get(id).cloned())
    }

    async fn get_head_cids(&self) -> Result<Vec<Cid>, DagError> {
        let heads_guard = self.head_cids.lock().unwrap();
        Ok(heads_guard.clone())
    }
    
    // Implement other DagStore methods as needed for tests, or with dummy implementations
    async fn has_event(&self, id: &EventId) -> Result<bool, DagError> { Ok(self.events.lock().unwrap().contains_key(id)) }
    async fn get_event_children(&self, _id: &EventId) -> Result<Vec<EventId>, DagError> { Ok(vec![]) }
    async fn get_event_parents(&self, id: &EventId) -> Result<Vec<EventId>, DagError> { 
        self.events.lock().unwrap().get(id).map(|e| e.parent_events.clone()).ok_or(DagError::EventNotFound(*id))
    }
    async fn get_events_by_type(&self, event_type: EventType) -> Result<Vec<DagEvent>, DagError> {
        let events_guard = self.events.lock().unwrap();
        Ok(events_guard.values().filter(|e| e.event_type == event_type).cloned().collect())
    }
    async fn get_events_by_author(&self, author: &Did) -> Result<Vec<DagEvent>, DagError> {
        let events_guard = self.events.lock().unwrap();
        Ok(events_guard.values().filter(|e| e.author == *author).cloned().collect())
    }
    // ... add other methods with Ok(Default::default()) or Ok(vec![]) if not used by these tests
}

// --- Mock ExecutionReceipt --- 
fn mock_receipt(issuer_did_key: &DidKey) -> ExecutionReceipt {
    let node_did_key = DidKey::generate().unwrap();
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
    let mut dag_store = InMemoryDagStore::new();
    let issuer_key = DidKey::generate().unwrap();
    let receipt = mock_receipt(&issuer_key);
    let original_receipt_cid = receipt.to_cid().unwrap(); // Get CID for later check

    let trigger_event_bytes = [1u8; 32];
    let trigger_event_id = EventId(trigger_event_bytes);

    let anchor_result = anchor_execution_receipt(&receipt, &mut dag_store, Some(trigger_event_id)).await;
    assert!(anchor_result.is_ok());
    let anchored_event_id = anchor_result.unwrap();

    // Verify the event was added to the store
    let stored_event_option = dag_store.get_event(&anchored_event_id).await.unwrap();
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
    let heads = dag_store.get_head_cids().await.unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0], Cid::from(anchored_event_id));
}

#[tokio::test]
async fn test_anchor_without_triggering_event() {
    let mut dag_store = InMemoryDagStore::new();
    let issuer_key = DidKey::generate().unwrap();
    let receipt = mock_receipt(&issuer_key);

    // Simulate an existing event to be picked as parent by get_head_cids
    let initial_event = DagEvent::new(EventType::Genesis, issuer_key.did().to_string(), vec![], EventPayload::Custom { fields: serde_json::Value::Null });
    let initial_event_id = dag_store.add_event(initial_event).await.unwrap();

    let anchor_result = anchor_execution_receipt(&receipt, &mut dag_store, None).await;
    assert!(anchor_result.is_ok());
    let anchored_event = dag_store.get_event(&anchor_result.unwrap()).await.unwrap().unwrap();
    
    assert!(anchored_event.parent_events.contains(&initial_event_id));
} 