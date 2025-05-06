use super::{EventId, EventPayload, EventType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEvent {
    pub event_type: EventType,
    pub timestamp: u64,
    pub author: String, // DID
    pub signature: Vec<u8>,
    pub parent_events: Vec<EventId>,
    pub payload: EventPayload,
}

impl DagEvent {
    /// Create a new DAG event
    pub fn new(
        event_type: EventType,
        author: impl Into<String>,
        parent_events: Vec<EventId>,
        payload: EventPayload,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        DagEvent {
            event_type,
            timestamp,
            author: author.into(),
            signature: Vec::new(), // Empty signature, to be filled by signing
            parent_events,
            payload,
        }
    }
    
    /// Set the signature for this event
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = signature;
        self
    }
    
    /// Check if this is a Genesis event
    pub fn is_genesis(&self) -> bool {
        matches!(self.event_type, EventType::Genesis)
    }
    
    /// Check if this event has any parents
    pub fn has_parents(&self) -> bool {
        !self.parent_events.is_empty()
    }
} 