#[cfg(feature = "ipld")]
use {
    cid::{Cid, Version},
    sha2::{Sha256, Digest},
    multihash::Multihash,
};

use crate::dag::{DagEvent, EventId};

/// Convert a DAG event to a CID string (requires "ipld" feature)
#[cfg(feature = "ipld")]
pub fn event_to_cid(event: &DagEvent) -> String {
    // Serialize the event to JSON
    let json = serde_json::to_vec(event).expect("Event serialization should not fail");
    
    // Hash the JSON with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(&json);
    let digest = hasher.finalize();
    
    // Create a Multihash (0x12 is the code for SHA2-256)
    let mh = Multihash::wrap(0x12, &digest).expect("Creating multihash should not fail");
    
    // Create a CIDv1 with dag-json codec
    let cid = Cid::new_v1(0x0129, mh);
    
    // Return the string representation
    cid.to_string()
}

/// Convert an EventId to a CID string (requires "ipld" feature)
#[cfg(feature = "ipld")]
pub fn event_id_to_cid(id: &EventId) -> String {
    // Hash the ID bytes with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    let digest = hasher.finalize();
    
    // Create a Multihash (0x12 is the code for SHA2-256)
    let mh = Multihash::wrap(0x12, &digest).expect("Creating multihash should not fail");
    
    // Create a CIDv1 with raw codec
    let cid = Cid::new_v1(0x55, mh); // 0x55 is raw codec
    
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