use crate::dag::{DagEvent, EventId};
use sha2::{Sha256, Digest};

/// Calculate the canonical hash for a DAG event
pub fn calculate_event_hash(event: &DagEvent) -> EventId {
    // Create a canonical representation for hashing
    // This ensures consistent hashes across different implementations
    
    // 1. Serialize to a canonical format (CBOR is recommended for production)
    // For simplicity, we'll use JSON in this example
    let canonical_json = serde_json::to_vec(&CanonicalEvent::from(event))
        .expect("Event serialization should not fail");
        
    // 2. Hash the canonical representation
    EventId::new(&canonical_json)
}

/// Verify that an event's hash matches the expected value
pub fn verify_event_hash(event: &DagEvent, expected_hash: &EventId) -> bool {
    let calculated_hash = calculate_event_hash(event);
    calculated_hash == *expected_hash
}

/// A canonical representation of an event for hashing
/// Omits the signature field since the signature is calculated after the hash
#[derive(serde::Serialize)]
struct CanonicalEvent<'a> {
    event_type: &'a crate::dag::EventType,
    timestamp: u64,
    author: &'a str,
    parent_events: &'a [EventId],
    payload: &'a crate::dag::EventPayload,
}

impl<'a> From<&'a DagEvent> for CanonicalEvent<'a> {
    fn from(event: &'a DagEvent) -> Self {
        CanonicalEvent {
            event_type: &event.event_type,
            timestamp: event.timestamp,
            author: &event.author,
            parent_events: &event.parent_events,
            payload: &event.payload,
        }
    }
}

/// Calculate a merkle root from multiple events
pub fn calculate_merkle_root(event_ids: &[EventId]) -> Option<EventId> {
    if event_ids.is_empty() {
        return None;
    }
    
    if event_ids.len() == 1 {
        return Some(event_ids[0].clone());
    }
    
    // For multiple events, build a Merkle tree
    let mut current_level: Vec<[u8; 32]> = event_ids.iter()
        .map(|id| id.0)
        .collect();
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        // Process pairs of hashes
        for chunk in current_level.chunks(2) {
            let mut hasher = Sha256::new();
            hasher.update(&chunk[0]);
            
            // If odd number of elements, duplicate the last one
            if chunk.len() > 1 {
                hasher.update(&chunk[1]);
            } else {
                hasher.update(&chunk[0]);
            }
            
            let result = hasher.finalize();
            let mut array = [0u8; 32];
            array.copy_from_slice(&result);
            next_level.push(array);
        }
        
        current_level = next_level;
    }
    
    Some(EventId(current_level[0]))
} 