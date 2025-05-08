use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::dag::{DagEvent, EventId, merkle::calculate_event_hash};

/// A node in the DAG containing an event and its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    /// The actual event data
    pub event: DagEvent,
    
    /// The unique identifier (hash) of this event
    pub id: EventId,
    
    /// Height in the DAG (max distance from Genesis)
    pub height: u64,
    
    /// Timestamp when the node was received/processed locally
    pub received_at: u64,
    
    /// Optional CID (Content Identifier) for IPFS/IPLD compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    
    /// Additional metadata for extensibility
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl DagNode {
    /// Create a new DAG node from an event
    pub fn new(event: DagEvent, height: u64) -> Self {
        let id = calculate_event_hash(&event);
        let received_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        DagNode {
            event,
            id,
            height,
            received_at,
            cid: None,
            metadata: HashMap::new(),
        }
    }
    
    /// Add metadata to this node
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Set the CID for this node
    pub fn with_cid(mut self, cid: impl Into<String>) -> Self {
        self.cid = Some(cid.into());
        self
    }
    
    /// Verify that this node's ID matches its event content
    pub fn verify_id(&self) -> bool {
        let calculated_id = calculate_event_hash(&self.event);
        self.id == calculated_id
    }
    
    /// Get a reference to the event
    pub fn event(&self) -> &DagEvent {
        &self.event
    }
    
    /// Get the node's ID
    pub fn id(&self) -> &EventId {
        &self.id
    }
} 