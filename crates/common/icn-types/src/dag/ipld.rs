#[cfg(feature = "ipld")]
use {
    cid::{Cid, Version},
    multihash::{Code, MultihashDigest},
};

use crate::dag::{DagEvent, EventId};

/// Convert a DAG event to a CID string (requires "ipld" feature)
#[cfg(feature = "ipld")]
pub fn event_to_cid(event: &DagEvent) -> String {
    // Serialize the event to JSON
    let json = serde_json::to_vec(event).expect("Event serialization should not fail");
    
    // Hash the JSON with SHA-256
    let hash = Code::Sha2_256.digest(&json);
    
    // Create a CIDv1 with dag-json codec
    let cid = Cid::new_v1(0x0129, hash);
    
    // Return the string representation
    cid.to_string()
}

/// Convert an EventId to a CID string (requires "ipld" feature)
#[cfg(feature = "ipld")]
pub fn event_id_to_cid(id: &EventId) -> String {
    // Create a CIDv1 with raw codec using the EventId bytes
    let hash = Code::Sha2_256.digest(id.as_bytes());
    let cid = Cid::new_v1(0x55, hash); // 0x55 is raw codec
    
    // Return the string representation
    cid.to_string()
}

/// Stub function for non-IPLD builds
#[cfg(not(feature = "ipld"))]
pub fn event_to_cid(_event: &DagEvent) -> String {
    "ipld-feature-not-enabled".to_string()
}

/// Stub function for non-IPLD builds
#[cfg(not(feature = "ipld"))]
pub fn event_id_to_cid(_id: &EventId) -> String {
    "ipld-feature-not-enabled".to_string()
} 