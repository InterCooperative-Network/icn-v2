use crate::cap_index::{CapabilitySelector};
use anyhow::Result;
use icn_identity_core::{Did, manifest::NodeManifest};
use icn_types::dag::{DagStore, Cid, DagNodeBuilder, DagPayload, SignedDagNode};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use log::{debug, info, warn, error};

/// Task request with requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// The DID of the requestor
    pub requestor: Did,
    
    /// WASM module hash
    pub wasm_hash: String,
    
    /// WASM module size in bytes
    pub wasm_size: usize,
    
    /// Input data URIs
    pub inputs: Vec<String>,
    
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: u64,
    
    /// Required memory in MB
    pub memory_mb: u64,
    
    /// Required CPU cores
    pub cores: u64,
    
    /// Task priority (1-100)
    pub priority: u8,
    
    /// Task timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Federation ID
    pub federation_id: String,
}

/// Bid response from a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBid {
    /// The DID of the bidder
    pub bidder: Did,
    
    /// Task ticket CID
    pub task_cid: String,
    
    /// Offered latency in milliseconds
    pub latency: u64,
    
    /// Available memory in MB
    pub memory: u64,
    
    /// Available CPU cores
    pub cores: u64,
    
    /// Bidder's reputation score
    pub reputation: u8,
    
    /// Renewable energy percentage
    pub renewable: u8,
    
    /// Bid timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of a task bid selection process
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Selected bid CID
    pub bid_cid: String,
    
    /// Selected bid
    pub bid: TaskBid,
    
    /// Score that determined selection
    pub score: f64,
}

/// Capability index for tracking node manifests
pub struct CapabilityIndex {
    /// Known node manifests by node DID
    pub manifests: RwLock<HashMap<Did, (NodeManifest, String)>>,
    
    /// DAG store for retrieving manifests
    dag_store: Arc<Box<dyn DagStore>>,
}

impl CapabilityIndex {
    /// Create a new capability index
    pub fn new(dag_store: Arc<Box<dyn DagStore>>) -> Self {
        Self {
            manifests: RwLock::new(HashMap::new()),
            dag_store,
        }
    }
    
    /// Add a node manifest
    pub async fn add_manifest(&self, manifest: NodeManifest, cid: String) -> Result<()> {
        let mut manifests = self.manifests.write().await;
        manifests.insert(manifest.did.clone(), (manifest, cid));
        Ok(())
    }
    
    /// Get a node's manifest
    pub async fn get_manifest(&self, did: &Did) -> Option<(NodeManifest, String)> {
        let manifests = self.manifests.read().await;
        manifests.get(did).cloned()
    }
    
    /// List all known manifests
    pub async fn list_manifests(&self) -> Vec<(NodeManifest, String)> {
        let manifests = self.manifests.read().await;
        manifests.values().cloned().collect()
    }
    
    /// Filter manifests based on a capability selector
    pub async fn filter_manifests(&self, selector: &CapabilitySelector) -> Vec<(NodeManifest, String)> {
        let manifests = self.manifests.read().await;
        manifests
            .values()
            .filter(|(manifest, _)| selector.matches(manifest))
            .cloned()
            .collect()
    }
}

/// Mesh scheduler for task-node matching
pub struct Scheduler {
    /// Federation ID this scheduler belongs to
    federation_id: String,
    
    /// Capability index for tracking node capabilities
    cap_index: Arc<CapabilityIndex>,
    
    /// DAG store for publishing task tickets and bids
    dag_store: Arc<Box<dyn DagStore>>,
    
    /// Scheduler's DID
    scheduler_did: Did,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(
        federation_id: String,
        cap_index: Arc<CapabilityIndex>,
        dag_store: Arc<Box<dyn DagStore>>,
        scheduler_did: Did,
    ) -> Self {
        Self {
            federation_id,
            cap_index,
            dag_store,
            scheduler_did,
        }
    }
    
    /// Listen for incoming task requests and bids
    pub async fn start_listening(&self) -> Result<()> {
        // In a real implementation, this would listen for incoming
        // task requests and bids over the network or DAG
        Ok(())
    }
    
    /// Dispatch a task request to suitable nodes
    pub async fn dispatch(&self, request: TaskRequest, capabilities: Option<CapabilitySelector>) -> Result<MatchResult> {
        info!("Dispatching task request from {}", request.requestor);
        
        // Create a default capability selector if none was provided
        let selector = capabilities.unwrap_or_else(|| {
            let mut selector = CapabilitySelector::new();
            
            // Set minimum requirements based on the task request
            selector.min_cores = Some(request.cores as u16);
            selector.min_ram_mb = Some(request.memory_mb as u32);
            
            selector
        });
        
        // Query the capability index for matching nodes
        let matching_manifests = self.cap_index.filter_manifests(&selector).await;
        
        if matching_manifests.is_empty() {
            warn!("No suitable nodes found for task request");
            return Err(anyhow::anyhow!("No suitable nodes found for task"));
        }
        
        info!("Found {} suitable nodes for task", matching_manifests.len());
        
        // In a real implementation, we would:
        // 1. Request bids from matching nodes
        // 2. Wait for responses
        // 3. Score and select the best bid
        
        // Create a simulated bid from the first matching node for demonstration
        let first_node = &matching_manifests[0].0;
        
        // Create a TaskBid
        let bid = TaskBid {
            bidder: first_node.did.clone(),
            task_cid: "simulated-task-cid".to_string(), // Would be the real CID in production
            latency: 100, // Simulated low latency
            memory: first_node.ram_mb as u64,
            cores: first_node.cores as u64,
            reputation: 90, // Simulated high reputation
            renewable: first_node.energy_profile.renewable_percentage,
            timestamp: chrono::Utc::now(),
        };
        
        // Create a DAG node for the bid
        let bid_payload = serde_json::json!({
            "type": "TaskBid",
            "bid": bid,
        });
        
        let bid_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(bid_payload))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_label("TaskBid".to_string())
            .build()?;
            
        // Create a signed node (in a real implementation this would be properly signed)
        let signed_bid_node = SignedDagNode {
            node: bid_node,
            signature: vec![], // Would be properly signed in production
            cid: None,
        };
        
        // Add to DAG to get CID
        let bid_cid = self.dag_store.add_node(signed_bid_node).await?;
        
        // Calculate a score for this bid
        // In a real implementation, we would calculate scores for all bids
        let score = self.calculate_bid_score(&bid, &request);
        
        info!("Selected bid from {} with score {}", bid.bidder, score);
        
        Ok(MatchResult {
            bid_cid: bid_cid.to_string(),
            bid,
            score,
        })
    }
    
    /// Calculate a score for a bid relative to a task request
    fn calculate_bid_score(&self, bid: &TaskBid, request: &TaskRequest) -> f64 {
        // Calculate score components
        
        // 1. Latency score (lower is better)
        let latency_score = if bid.latency <= request.max_latency_ms {
            // If within max latency, score based on how much better than max
            1.0 - (bid.latency as f64 / request.max_latency_ms as f64)
        } else {
            // Exceeds max latency, but might still be usable in some cases
            -0.5
        };
        
        // 2. Resource match score
        let memory_ratio = bid.memory as f64 / request.memory_mb as f64;
        let core_ratio = bid.cores as f64 / request.cores as f64;
        
        // Prefer nodes with resources closer to what's needed (avoid over-provisioning)
        let resource_score = if memory_ratio >= 1.0 && core_ratio >= 1.0 {
            1.0 / (1.0 + (memory_ratio - 1.0) * 0.2 + (core_ratio - 1.0) * 0.2)
        } else {
            // Under-provisioned, not ideal
            0.2
        };
        
        // 3. Reputation score
        let reputation_score = bid.reputation as f64 / 100.0;
        
        // 4. Energy score (renewable energy percentage)
        let energy_score = bid.renewable as f64 / 100.0;
        
        // 5. Task priority factor
        let priority_factor = request.priority as f64 / 50.0; // Normalize around 1.0
        
        // Calculate weighted total score
        // Weights can be adjusted based on importance
        let total_score = (
            latency_score * 0.3 +
            resource_score * 0.25 +
            reputation_score * 0.2 +
            energy_score * 0.25
        ) * priority_factor;
        
        total_score
    }
    
    /// Accept a bid and notify the winning node
    pub async fn accept_bid(&self, result: &MatchResult) -> Result<()> {
        // In a real implementation, this would:
        // 1. Create a bid acceptance record in the DAG
        // 2. Notify the winning node
        // 3. Update relevant state
        
        info!("Accepted bid {} from {}", result.bid_cid, result.bid.bidder);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap_index::CapabilitySelector;
    use icn_identity_core::manifest::{
        Architecture, GpuApi, EnergySource,
        NodeManifest, EnergyInfo, GpuProfile
    };
    use icn_types::dag::memory::MemoryDagStore;
    
    async fn create_test_scheduler() -> (Scheduler, Arc<CapabilityIndex>) {
        let dag_store = Arc::new(Box::new(MemoryDagStore::new()) as Box<dyn DagStore>);
        let cap_index = Arc::new(CapabilityIndex::new(dag_store.clone()));
        
        // Add some test manifests
        let manifest1 = NodeManifest {
            did: "did:icn:node1".into(),
            arch: Architecture::X86_64,
            cores: 8,
            gpu: Some(GpuProfile {
                model: "Test GPU".to_string(),
                api: vec![GpuApi::Cuda, GpuApi::Vulkan],
                vram_mb: 8192,
                cores: 4096,
                tensor_cores: true,
                features: vec!["ray-tracing".to_string(), "ai-acceleration".to_string()],
            }),
            ram_mb: 16384,
            storage_bytes: 1_000_000_000_000, // 1TB
            sensors: vec![],
            actuators: vec![],
            energy_profile: EnergyInfo {
                renewable_percentage: 75,
                battery_percentage: Some(80),
                charging: Some(true),
                power_consumption_watts: Some(45.5),
                source: vec![EnergySource::Solar, EnergySource::Battery],
            },
            trust_fw_hash: "test-hash".to_string(),
            mesh_protocols: vec!["gossipsub".to_string()],
            last_seen: chrono::Utc::now(),
            signature: vec![],
        };
        
        let manifest2 = NodeManifest {
            did: "did:icn:node2".into(),
            arch: Architecture::Arm64,
            cores: 4,
            gpu: None,
            ram_mb: 8192,
            storage_bytes: 500_000_000_000, // 500GB
            sensors: vec![],
            actuators: vec![],
            energy_profile: EnergyInfo {
                renewable_percentage: 0,
                battery_percentage: None,
                charging: None,
                power_consumption_watts: Some(65.0),
                source: vec![EnergySource::Grid],
            },
            trust_fw_hash: "test-hash".to_string(),
            mesh_protocols: vec!["gossipsub".to_string()],
            last_seen: chrono::Utc::now(),
            signature: vec![],
        };
        
        cap_index.add_manifest(manifest1, "manifest1-cid".to_string()).await.unwrap();
        cap_index.add_manifest(manifest2, "manifest2-cid".to_string()).await.unwrap();
        
        let scheduler = Scheduler::new(
            "test-federation".to_string(),
            cap_index.clone(),
            dag_store,
            "did:icn:scheduler".into(),
        );
        
        (scheduler, cap_index)
    }
    
    #[tokio::test]
    async fn test_dispatch_with_selector() {
        let (scheduler, _) = create_test_scheduler().await;
        
        let request = TaskRequest {
            requestor: "did:icn:requestor".into(),
            wasm_hash: "test-hash".to_string(),
            wasm_size: 1024,
            inputs: vec![],
            max_latency_ms: 1000,
            memory_mb: 8192,
            cores: 4,
            priority: 50,
            timestamp: chrono::Utc::now(),
            federation_id: "test-federation".to_string(),
        };
        
        // Create a selector that only matches x86_64 architecture
        let mut selector = CapabilitySelector::new();
        selector.arch = Some(Architecture::X86_64);
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_ok());
        
        let match_result = result.unwrap();
        assert_eq!(match_result.bid.bidder, "did:icn:node1");
    }
    
    #[tokio::test]
    async fn test_dispatch_with_gpu_requirement() {
        let (scheduler, _) = create_test_scheduler().await;
        
        let request = TaskRequest {
            requestor: "did:icn:requestor".into(),
            wasm_hash: "test-hash".to_string(),
            wasm_size: 1024,
            inputs: vec![],
            max_latency_ms: 1000,
            memory_mb: 4096,
            cores: 2,
            priority: 50,
            timestamp: chrono::Utc::now(),
            federation_id: "test-federation".to_string(),
        };
        
        // Create a selector that requires a GPU with CUDA
        let mut selector = CapabilitySelector::new();
        selector.gpu_requirements = Some(crate::cap_index::GpuRequirements {
            min_vram_mb: Some(4096),
            min_cores: None,
            requires_tensor_cores: true,
            required_api: Some(vec![GpuApi::Cuda]),
            required_features: None,
        });
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_ok());
        
        let match_result = result.unwrap();
        assert_eq!(match_result.bid.bidder, "did:icn:node1");
    }
    
    #[tokio::test]
    async fn test_dispatch_with_energy_requirement() {
        let (scheduler, _) = create_test_scheduler().await;
        
        let request = TaskRequest {
            requestor: "did:icn:requestor".into(),
            wasm_hash: "test-hash".to_string(),
            wasm_size: 1024,
            inputs: vec![],
            max_latency_ms: 1000,
            memory_mb: 4096,
            cores: 2,
            priority: 50,
            timestamp: chrono::Utc::now(),
            federation_id: "test-federation".to_string(),
        };
        
        // Create a selector that requires renewable energy
        let mut selector = CapabilitySelector::new();
        selector.energy_requirements = Some(crate::cap_index::EnergyRequirements {
            min_renewable_percentage: Some(50),
            required_sources: Some(vec![EnergySource::Solar]),
            requires_battery: false,
            requires_charging: false,
            max_power_consumption: None,
        });
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_ok());
        
        let match_result = result.unwrap();
        assert_eq!(match_result.bid.bidder, "did:icn:node1");
    }
    
    #[tokio::test]
    async fn test_dispatch_no_matching_nodes() {
        let (scheduler, _) = create_test_scheduler().await;
        
        let request = TaskRequest {
            requestor: "did:icn:requestor".into(),
            wasm_hash: "test-hash".to_string(),
            wasm_size: 1024,
            inputs: vec![],
            max_latency_ms: 1000,
            memory_mb: 4096,
            cores: 2,
            priority: 50,
            timestamp: chrono::Utc::now(),
            federation_id: "test-federation".to_string(),
        };
        
        // Create a selector with requirements that no node can meet
        let mut selector = CapabilitySelector::new();
        selector.min_cores = Some(32); // Much more than any node has
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_err());
    }
} 