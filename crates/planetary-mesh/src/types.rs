use serde::{Deserialize, Serialize};
use icn_core_types::{Cid, Did}; // Updated from use crate::{Did, Cid}

/// Job status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job has been submitted but not yet scheduled
    Submitted,
    /// Job has been scheduled and is waiting for execution
    Scheduled,
    /// Job is currently executing
    Running,
    /// Job has completed successfully
    Completed,
    /// Job has failed
    Failed,
    /// Job has been canceled
    Canceled,
}

/// Resource types that can be requested for a job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// RAM in MB
    RamMb(u64),
    /// CPU cores
    CpuCores(u64),
    /// GPU cores
    GpuCores(u64),
    /// Disk space in MB
    StorageMb(u64),
}

/// Node capability for resource matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapability {
    /// Type of capability
    pub resource_type: ResourceType,
    /// Whether this resource is currently available
    pub available: bool,
    /// When this capability was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Bid for a job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bid {
    /// Node ID making the bid
    pub node_id: String,
    /// Cooperative ID this node belongs to
    pub coop_id: String,
    /// Price of the bid in compute units
    pub price: u64,
    /// Expected completion time 
    pub eta: chrono::DateTime<chrono::Utc>,
    /// Submitted at timestamp
    pub submitted_at: chrono::DateTime<chrono::Utc>,
}

/// Job manifest for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobManifest {
    /// Job ID
    pub id: String,
    /// Federation ID
    pub federation_id: String,
    /// Cooperative ID that submitted this job
    pub origin_coop_id: String,
    /// CID of the WASM module to execute
    pub wasm_module_cid: String,
    /// Resource requirements
    pub resource_requirements: Vec<ResourceType>,
    /// Job parameters as JSON
    pub parameters: serde_json::Value,
    /// Owner DID
    pub owner: String,
    /// Optional deadline
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional maximum compute units willing to spend
    pub max_compute_units: Option<u64>,
}

/// Placeholder definition for a Node Capability advertisement.
/// (This might be closer to NodeManifest from icn-identity-core?)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCapability {
     pub node_id: Did,
     pub available_resources: Vec<ResourceType>, // Simplified representation
     pub supported_features: Vec<String>, // e.g., "wasm", "sgx"
} 