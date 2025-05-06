use serde::{Deserialize, Serialize};
use crate::{Did, Cid}; // Import necessary types from this crate

/// Placeholder for defining compute/storage/network resource requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceType {
    CpuCores(u32),
    RamMb(u64),
    StorageGb(u64),
    Gpu(String), // e.g., "NVIDIA_RTX_3080", "Any"
    NetworkBandwidthMbps(u32),
    // Add other resource types as needed
}

/// Placeholder definition for a Job Manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobManifest {
    pub id: String, // Or perhaps Cid?
    pub wasm_module_cid: Cid,
    pub resource_requirements: Vec<ResourceType>,
    pub parameters: serde_json::Value,
    pub owner: Did, // Added owner DID
    pub deadline: Option<chrono::DateTime<chrono::Utc>>, // Added optional deadline
}

/// Placeholder definition for a Node Capability advertisement.
/// (This might be closer to NodeManifest from icn-identity-core?)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCapability {
     pub node_id: Did,
     pub available_resources: Vec<ResourceType>, // Simplified representation
     pub supported_features: Vec<String>, // e.g., "wasm", "sgx"
}


/// Placeholder definition for a Bid on a job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bid {
    pub job_manifest_cid: Cid,
    pub bidder_node_id: Did,
    pub price: u64, // Price in some token unit
    pub confidence: f32, // Placeholder for scoring metric (0.0 - 1.0)
    pub offered_capabilities: Vec<ResourceType>, // What the node *offers* for this bid
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>, // Bid expiration
} 