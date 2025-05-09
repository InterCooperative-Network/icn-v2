use anyhow::{anyhow, Context, Result};
use log::{debug, info, warn};
use ed25519_dalek::Signature;
use icn_core_types::Did;
use icn_identity_core::{
    did::DidKey,
    manifest::{
        Architecture as IdArchitecture, CapabilitySelector, EnergyInfo as IdEnergyInfo, 
        EnergySource, GpuApi, GpuProfile, NodeManifest
    },
};
use icn_types::{
    dag::{DagNode, DagNodeBuilder, DagNodeMetadata, DagPayload, DagStore, SharedDagStore, SignedDagNode},
    Cid,
};
use serde::{Serialize, Deserialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use uuid::Uuid;
use hex;
use chrono::Utc;

// Use crate imports for manifest verification
use crate::manifest_verifier::{ManifestVerifier, ManifestVerificationError};
// Use our own Architecture and EnergyInfo types to avoid conflicts
use crate::cap_index::CapabilitySelector as MeshCapabilitySelector;

use icn_types::dag::NodeScope;
// Use fully qualified names for different ResourceType implementations
use icn_economics::token::ResourceType as EconomicResourceType;
use icn_economics::storage::TokenStore;
use icn_economics::transaction::{ResourceTransaction, TransactionType};
use crate::types::ResourceType as MeshResourceType;

/// Architecture type with Default implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Architecture {
    pub name: String,
    pub version: String,
}

impl Default for Architecture {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: "unknown".to_string(),
        }
    }
}

/// Energy info with Default implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyInfo {
    pub source: String,
    pub renewable_percentage: u8,
}

impl Default for EnergyInfo {
    fn default() -> Self {
        Self {
            source: "unknown".to_string(),
            renewable_percentage: 0,
        }
    }
}

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

/// Configuration for capability index behavior
#[derive(Debug, Clone)]
pub struct CapabilityIndexConfig {
    /// Whether to verify manifest signatures
    pub verify_signatures: bool,
    
    /// Whether manifest signature verification is required
    pub require_valid_signatures: bool,
    
    /// Trusted DIDs allowed to issue manifests
    pub trusted_dids: Option<Vec<Did>>,
}

impl Default for CapabilityIndexConfig {
    fn default() -> Self {
        Self {
            verify_signatures: true,
            require_valid_signatures: false,
            trusted_dids: None,
        }
    }
}

/// Capability index for tracking node manifests
pub struct CapabilityIndex {
    /// Known node manifests by node DID
    pub manifests: RwLock<HashMap<Did, (NodeManifest, String)>>,
    
    /// DAG store for retrieving manifests
    dag_store: SharedDagStore,
    
    /// Manifest verifier for signature checking
    verifier: ManifestVerifier,
    
    /// Configuration options
    config: CapabilityIndexConfig,
    
    /// Verification failure handler
    verification_failure_handler: RwLock<Option<Box<dyn Fn(&Did, &ManifestVerificationError) + Send + Sync>>>,
}

impl CapabilityIndex {
    /// Create a new capability index with default configuration
    pub fn new(dag_store: SharedDagStore) -> Self {
        Self::with_config(dag_store, CapabilityIndexConfig::default())
    }
    
    /// Create a new capability index with custom configuration
    pub fn with_config(dag_store: SharedDagStore, config: CapabilityIndexConfig) -> Self {
        let verifier = if let Some(trusted_dids) = &config.trusted_dids {
            ManifestVerifier::with_trusted_dids(trusted_dids.clone())
        } else {
            ManifestVerifier::new()
        };
        
        Self {
            manifests: RwLock::new(HashMap::new()),
            dag_store,
            verifier,
            config,
            verification_failure_handler: RwLock::new(None),
        }
    }
    
    /// Set a handler for verification failures
    pub fn set_verification_failure_handler<F>(&self, handler: Box<F>)
    where
        F: Fn(&Did, &ManifestVerificationError) + Send + Sync + 'static,
    {
        let mut failure_handler = self.verification_failure_handler.blocking_write();
        *failure_handler = Some(handler);
    }
    
    /// Add a node manifest
    pub async fn add_manifest(&self, manifest: NodeManifest, cid: String) -> Result<()> {
        // Verify the manifest signature if enabled
        if self.config.verify_signatures {
            match self.verifier.verify_manifest(&manifest) {
                Ok(true) => {
                    debug!("Manifest signature verified for DID: {}", manifest.did);
                },
                Ok(false) => {
                    warn!("Manifest has invalid signature for DID: {}", manifest.did);
                    
                    // Call the failure handler if set
                    let err = ManifestVerificationError::InvalidSignature;
                    if let Some(handler) = &*self.verification_failure_handler.read().await {
                        handler(&manifest.did, &err);
                    }
                    
                    // If valid signatures are required, reject this manifest
                    if self.config.require_valid_signatures {
                        return Err(anyhow!("Manifest signature verification failed"));
                    }
                },
                Err(e) => {
                    warn!("Error verifying manifest signature: {:?}", e);
                    
                    // Call the failure handler if set
                    if let Some(handler) = &*self.verification_failure_handler.read().await {
                        handler(&manifest.did, &e);
                    }
                    
                    // If valid signatures are required, reject this manifest
                    if self.config.require_valid_signatures {
                        return Err(anyhow!("Manifest signature verification error: {:?}", e));
                    }
                }
            }
        }
        
        // Add the manifest to the index
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
    pub async fn filter_manifests(&self, selector: &MeshCapabilitySelector) -> Vec<(NodeManifest, String)> {
        let manifests = self.manifests.read().await;
        manifests
            .values()
            .filter(|(manifest, _)| selector.matches(manifest))
            .cloned()
            .collect()
    }
    
    /// Add a manifest from DAG by its CID
    pub async fn add_manifest_by_cid(&self, cid: &Cid) -> Result<()> {
        // Get the node from the DAG
        let node = self.dag_store.get_node(cid).await
            .context("Failed to get node from DAG")?;
        
        // Check if it's a manifest
        if let DagPayload::Json(payload) = &node.node.payload {
            // Check if it's a NodeManifestCredential
            let is_manifest = payload
                .get("type")
                .and_then(|t| t.as_array())
                .map(|types| types.iter().any(|t| t.as_str() == Some("NodeManifestCredential")))
                .unwrap_or(false);
            
            if is_manifest {
                // Verify the manifest VC if enabled
                if self.config.verify_signatures {
                    match self.verifier.verify_manifest_vc(payload) {
                        Ok(true) => {
                            debug!("Manifest VC signature verified for CID: {}", cid);
                        },
                        Ok(false) => {
                            warn!("Manifest VC has invalid signature for CID: {}", cid);
                            
                            // If valid signatures are required, reject this manifest
                            if self.config.require_valid_signatures {
                                return Err(anyhow!("Manifest VC signature verification failed"));
                            }
                        },
                        Err(e) => {
                            warn!("Error verifying manifest VC signature: {:?}", e);
                            
                            // If valid signatures are required, reject this manifest
                            if self.config.require_valid_signatures {
                                return Err(anyhow!("Manifest VC signature verification error: {:?}", e));
                            }
                        }
                    }
                }
                
                // Extract the DID from the credential subject
                if let Some(subject) = payload.get("credentialSubject") {
                    if let Some(did_str) = subject.get("id").and_then(|i| i.as_str()) {
                        let did = Did::from(did_str.to_string());
                        
                        // Create a NodeManifest from the payload
                        // This is a simplified conversion that would be more complete in production
                        let manifest = NodeManifest {
                            did: did.clone(),
                            arch: serde_json::from_value(subject.get("architecture").cloned().unwrap_or(serde_json::Value::Null))
                                .unwrap_or_default(),
                            cores: subject.get("cores").and_then(|c| c.as_u64()).unwrap_or(0) as u16,
                            ram_mb: subject.get("ramMb").and_then(|r| r.as_u64()).unwrap_or(0) as u32,
                            storage_bytes: subject.get("storageBytes").and_then(|s| s.as_u64()).unwrap_or(0),
                            gpu: serde_json::from_value(subject.get("gpu").cloned().unwrap_or(serde_json::Value::Null)).ok(),
                            sensors: serde_json::from_value(subject.get("sensors").cloned().unwrap_or(serde_json::json!([]))).unwrap_or_default(),
                            actuators: serde_json::from_value(subject.get("actuators").cloned().unwrap_or(serde_json::json!([]))).unwrap_or_default(),
                            energy_profile: serde_json::from_value(subject.get("energyProfile").cloned().unwrap_or(serde_json::json!({}))).unwrap_or_default(),
                            trust_fw_hash: subject.get("trustFirmwareHash").and_then(|h| h.as_str()).unwrap_or("unknown").to_string(),
                            mesh_protocols: serde_json::from_value(subject.get("meshProtocols").cloned().unwrap_or(serde_json::json!([]))).unwrap_or_default(),
                            last_seen: serde_json::from_value(subject.get("lastSeen").cloned().unwrap_or(serde_json::json!(chrono::Utc::now()))).unwrap_or_else(|_| chrono::Utc::now()),
                            signature: payload.get("proof").and_then(|p| p.get("proofValue")).and_then(|pv| pv.as_str())
                                .and_then(|s| hex::decode(s).ok()).unwrap_or_default(),
                        };
                        
                        // Add the manifest to the index
                        self.add_manifest(manifest, cid.to_string()).await?;
                        return Ok(());
                    }
                }
                
                return Err(anyhow!("Invalid manifest VC format"));
            }
        }
        
        Err(anyhow!("Node is not a manifest"))
    }
}

/// Match result with cooperative context
#[derive(Debug, Clone)]
pub struct ScopedMatchResult {
    /// Original match result
    pub base: MatchResult,
    
    /// Origin cooperative ID
    pub origin_coop: String,
    
    /// Executor cooperative ID
    pub executor_coop: String,
    
    /// Transaction CIDs for resource tracking
    pub transaction_cids: Vec<String>,
}

/// Mesh scheduler for task-node matching
pub struct Scheduler {
    /// Federation ID this scheduler belongs to
    federation_id: String,
    
    /// Capability index for tracking node capabilities
    cap_index: Arc<CapabilityIndex>,
    
    /// DAG store for publishing task tickets and bids
    dag_store: SharedDagStore,
    
    /// Scheduler's DID
    scheduler_did: Did,
    
    /// Scheduler's DID key for signing
    did_key: Option<DidKey>,
    
    /// Token store for resource accounting
    token_store: Option<Arc<dyn TokenStore + Send + Sync>>,
}

impl Scheduler {
    /// Create a new scheduler without a signing key
    pub fn new(
        federation_id: String,
        cap_index: Arc<CapabilityIndex>,
        dag_store: SharedDagStore,
        scheduler_did: Did,
    ) -> Self {
        Self {
            federation_id,
            cap_index,
            dag_store,
            scheduler_did,
            did_key: None,
            token_store: None,
        }
    }
    
    /// Create a new scheduler with a signing key
    pub fn new_with_key(
        federation_id: String,
        cap_index: Arc<CapabilityIndex>,
        dag_store: SharedDagStore,
        did_key: DidKey,
    ) -> Self {
        let scheduler_did = did_key.did().clone();
        Self {
            federation_id,
            cap_index,
            dag_store,
            scheduler_did,
            did_key: Some(did_key),
            token_store: None,
        }
    }
    
    /// Create a new scheduler with token store for resource accounting
    pub fn new_with_token_store(
        federation_id: String,
        cap_index: Arc<CapabilityIndex>,
        dag_store: SharedDagStore,
        scheduler_did: Did,
        token_store: Arc<dyn TokenStore + Send + Sync>,
    ) -> Self {
        Self {
            federation_id,
            cap_index,
            dag_store,
            scheduler_did,
            did_key: None,
            token_store: Some(token_store),
        }
    }
    
    /// Set the token store for resource accounting
    pub fn set_token_store(&mut self, token_store: Arc<dyn TokenStore + Send + Sync>) {
        self.token_store = Some(token_store);
    }
    
    /// Listen for incoming task requests and bids
    pub async fn start_listening(&self) -> Result<()> {
        // In a real implementation, this would listen for incoming
        // task requests and bids over the network or DAG
        Ok(())
    }
    
    /// Dispatch a task request to suitable nodes
    pub async fn dispatch(&self, request: TaskRequest, capabilities: Option<MeshCapabilitySelector>) -> Result<MatchResult> {
        info!("Dispatching task request from {}", request.requestor);
        
        // Create a default capability selector if none was provided
        let selector = capabilities.unwrap_or_else(|| {
            let mut selector = MeshCapabilitySelector::new();
            
            // Set minimum requirements based on the task request
            selector.min_cores = Some(request.cores as u16);
            selector.min_ram_mb = Some(request.memory_mb as u32);
            
            selector
        });
        
        // Query the capability index for matching nodes
        let matching_manifests = self.cap_index.filter_manifests(&selector).await;
        
        if matching_manifests.is_empty() {
            warn!("No suitable nodes found for task request");
            return Err(anyhow!("No suitable nodes found for task"));
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
        
        // Create a payload that includes both the bid and the capability requirements
        // This anchors the capability selector in the DAG for future auditing
        let bid_payload = serde_json::json!({
            "type": "TaskBid",
            "bid": bid,
            "capability_requirements": {
                "selector": selector,
                "matching_nodes": matching_manifests.len(),
                "selected_node": first_node.did.to_string()
            },
            "dispatch_time": chrono::Utc::now().to_rfc3339(),
            "dispatch_by": self.scheduler_did.to_string()
        });
        
        let bid_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(bid_payload))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_label("TaskBid".to_string())
            .build()?;
            
        // Create a signed node
        let signed_bid_node = create_empty_signed_node(bid_node);
        
        // Add to DAG to get CID
        let bid_cid = self.dag_store.add_node(signed_bid_node).await?;
        
        // Calculate a score for this bid
        // In a real implementation, we would calculate scores for all bids
        let score = self.calculate_bid_score(&bid, &request);
        
        info!("Selected bid from {} with score {}", bid.bidder, score);
        
        // Generate a dispatch audit record
        self.create_dispatch_audit_record(&request, &selector, &matching_manifests, &bid, &bid_cid, score).await?;
        
        Ok(MatchResult {
            bid_cid: bid_cid.to_string(),
            bid,
            score,
        })
    }
    
    /// Create an audit record in the DAG for a dispatch decision
    async fn create_dispatch_audit_record(
        &self,
        request: &TaskRequest,
        selector: &MeshCapabilitySelector,
        matching_manifests: &[(NodeManifest, String)],
        selected_bid: &TaskBid,
        bid_cid: &Cid,
        score: f64,
    ) -> Result<Cid> {
        // Create a list of matching node DIDs for the audit record
        let matching_nodes: Vec<String> = matching_manifests
            .iter()
            .map(|(manifest, _)| manifest.did.to_string())
            .collect();

        // Generate a unique credential ID
        let credential_id = format!("urn:icn:dispatch:{}", uuid::Uuid::new_v4());
        let timestamp = chrono::Utc::now();
        
        // Create the credential subject with dispatch details
        let credential_subject = serde_json::json!({
            "id": request.requestor.to_string(),
            "taskRequest": {
                "wasm_hash": request.wasm_hash,
                "wasm_size": request.wasm_size,
                "inputs": request.inputs,
                "max_latency_ms": request.max_latency_ms,
                "memory_mb": request.memory_mb,
                "cores": request.cores,
                "priority": request.priority,
                "timestamp": request.timestamp,
                "federation_id": request.federation_id,
            },
            "capabilities": selector,
            "selectedNode": selected_bid.bidder.to_string(),
            "score": score,
            "dispatchTime": timestamp,
            "matchingNodeCount": matching_nodes.len(),
            "bid": {
                "bidCid": bid_cid.to_string(),
                "latency": selected_bid.latency,
                "memory": selected_bid.memory,
                "cores": selected_bid.cores,
                "reputation": selected_bid.reputation,
                "renewable": selected_bid.renewable
            }
        });
        
        // Create the verifiable credential skeleton for the dispatch record
        let mut dispatch_credential = serde_json::json!({
            "@context": [
                "https://www.w3.org/2018/credentials/v1",
                "https://icn.network/context/mesh-compute/v1"
            ],
            "id": credential_id,
            "type": ["VerifiableCredential", "DispatchReceipt"],
            "issuer": self.scheduler_did.to_string(),
            "issuanceDate": timestamp.to_rfc3339(),
            "credentialSubject": credential_subject,
        });
        
        // Sign the credential if we have a DID key
        if let Some(did_key) = &self.did_key {
            // Clone the credential without the proof (we'll add it after signing)
            let credential_to_sign = dispatch_credential.clone();
            
            // Convert to bytes for signing
            let credential_bytes = serde_json::to_vec(&credential_to_sign)
                .context("Failed to serialize credential")?;
            
            // Sign the credential
            let signature = did_key.sign(&credential_bytes);
            
            // Add the proof to the credential
            dispatch_credential["proof"] = serde_json::json!({
                "type": "Ed25519Signature2020",
                "verificationMethod": format!("{}#keys-1", self.scheduler_did.to_string()),
                "created": timestamp.to_rfc3339(),
                "proofValue": hex::encode(&signature.to_bytes()),
            });
        }
        
        // Create the full dispatch audit record that includes the VC
        let audit_payload = serde_json::json!({
            "type": "DispatchAuditRecord",
            "credential": dispatch_credential,
            "task_request": {
                "requestor": request.requestor.to_string(),
                "wasm_hash": request.wasm_hash,
                "wasm_size": request.wasm_size,
                "inputs": request.inputs,
                "max_latency_ms": request.max_latency_ms,
                "memory_mb": request.memory_mb,
                "cores": request.cores,
                "priority": request.priority,
                "timestamp": request.timestamp,
                "federation_id": request.federation_id,
            },
            "capability_selector": selector,
            "matching_nodes": matching_nodes,
            "selected_bid": {
                "bid_cid": bid_cid.to_string(),
                "bidder": selected_bid.bidder.to_string(),
                "score": score,
            },
            "dispatch_time": timestamp.to_rfc3339(),
            "scheduler": self.scheduler_did.to_string(),
        });
        
        // Create a DAG node for the audit record
        let audit_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(audit_payload))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_label("DispatchAuditRecord".to_string())
            .build()?;
            
        // Create a signed node
        let mut signed_audit_node = create_empty_signed_node(audit_node);
        
        // Sign the DAG node if we have a DID key
        if let Some(did_key) = &self.did_key {
            // Replace manual signing with our utility function
            let node_bytes = serde_json::to_vec(&signed_audit_node.node)
                .context("Failed to serialize audit node")?;
            
            // Create a properly signed node
            let signed_node = create_signed_node(signed_audit_node.node.clone(), did_key)?;
            signed_audit_node = signed_node;
        }
        
        // Add to DAG to get CID
        let audit_cid = self.dag_store.add_node(signed_audit_node).await?;
        
        debug!("Created dispatch audit record with CID: {}", audit_cid);
        
        Ok(audit_cid)
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
    
    /// Dispatch a job across cooperatives
    pub async fn dispatch_cross_coop(
        &self,
        request: TaskRequest,
        origin_coop: String,
    ) -> Result<ScopedMatchResult> {
        // Verify the origin cooperative has enough resource tokens
        self.verify_resource_budget(&request, &origin_coop).await?;
        
        // Find the best node for the job
        let match_result = self.dispatch(request.clone(), None).await?;
        
        // Get the executor cooperative ID from the selected node's manifest
        let executor_coop = self.get_coop_for_node(&match_result.bid.bidder).await?;
        
        // Create a resource debit transaction for the origin cooperative
        let resource_cost = self.calculate_resource_cost(&request)?;
        let debit_tx = ResourceTransaction::new_debit(
            EconomicResourceType::ComputeUnit,
            resource_cost,
            &origin_coop,
            &self.federation_id,
            self.scheduler_did.clone(),
        );
        
        // Create a resource credit transaction for the executor cooperative
        let execution_fee = self.calculate_execution_fee(&request)?;
        let credit_tx = ResourceTransaction::new_credit(
            EconomicResourceType::ComputeUnit,
            execution_fee,
            &executor_coop,
            &self.federation_id,
            self.scheduler_did.clone(),
        );
        
        // Record the transactions in the DAG
        let mut transaction_cids = Vec::new();
        
        // Record debit in origin cooperative's DAG
        let debit_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(serde_json::to_value(&debit_tx)?))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(origin_coop.clone())
            .with_label("ResourceDebit".to_string())
            .build()?;
        
        let signed_debit_node = if let Some(key) = &self.did_key {
            create_signed_node(debit_node, key)?
        } else {
            create_empty_signed_node(debit_node)
        };
        
        let debit_cid = self.dag_store.add_node(signed_debit_node).await?;
        transaction_cids.push(debit_cid.to_string());
        
        // Record credit in executor cooperative's DAG
        let credit_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(serde_json::to_value(&credit_tx)?))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_scope(NodeScope::Cooperative)
            .with_scope_id(executor_coop.clone())
            .with_label("ResourceCredit".to_string())
            .build()?;
        
        let signed_credit_node = if let Some(key) = &self.did_key {
            create_signed_node(credit_node, key)?
        } else {
            create_empty_signed_node(credit_node)
        };
        
        let credit_cid = self.dag_store.add_node(signed_credit_node).await?;
        transaction_cids.push(credit_cid.to_string());
        
        // Record cross-cooperative transaction in federation DAG
        let cross_coop_tx = ResourceTransaction::new_transfer(
            EconomicResourceType::ComputeUnit,
            resource_cost,
            &origin_coop,
            &executor_coop,
            &self.federation_id,
            self.scheduler_did.clone(),
        );
        
        let transfer_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(serde_json::to_value(&cross_coop_tx)?))
            .with_author(self.scheduler_did.clone())
            .with_federation_id(self.federation_id.clone())
            .with_scope(NodeScope::Federation)
            .with_label("CrossCoopTransaction".to_string())
            .build()?;
        
        let signed_transfer_node = if let Some(key) = &self.did_key {
            create_signed_node(transfer_node, key)?
        } else {
            create_empty_signed_node(transfer_node)
        };
        
        let transfer_cid = self.dag_store.add_node(signed_transfer_node).await?;
        transaction_cids.push(transfer_cid.to_string());
        
        // Apply transactions to the token store if available
        if let Some(token_store) = &self.token_store {
            token_store.apply_transaction(&debit_tx).await.map_err(|e| 
                anyhow!("Failed to apply debit transaction: {}", e)
            )?;
            
            token_store.apply_transaction(&credit_tx).await.map_err(|e| 
                anyhow!("Failed to apply credit transaction: {}", e)
            )?;
        }
        
        // Create the scoped match result
        let scoped_result = ScopedMatchResult {
            base: match_result,
            origin_coop,
            executor_coop,
            transaction_cids,
        };
        
        Ok(scoped_result)
    }
    
    /// Verify that a cooperative has sufficient resources for a job
    async fn verify_resource_budget(&self, request: &TaskRequest, coop_id: &str) -> Result<bool> {
        if let Some(token_store) = &self.token_store {
            let required_tokens = self.calculate_resource_cost(request)?;
            let available_tokens = token_store
                .get_balance(coop_id, &EconomicResourceType::ComputeUnit)
                .await
                .map_err(|e| anyhow!("Failed to get balance: {}", e))?;
            
            if available_tokens < required_tokens {
                return Err(anyhow!("Insufficient compute units. Required: {}, Available: {}", 
                    required_tokens, available_tokens));
            }
            
            Ok(true)
        } else {
            // If no token store is available, just approve the job
            Ok(true)
        }
    }
    
    /// Calculate resource cost based on task requirements
    fn calculate_resource_cost(&self, request: &TaskRequest) -> Result<u64> {
        // Base cost for any job
        let mut cost: u64 = 10;
        
        // Add cost for memory
        cost += request.memory_mb / 10;
        
        // Add cost for cores
        cost += request.cores * 5;
        
        // Add cost for priority (1-100 scale)
        cost += (request.priority as u64).max(1);
        
        Ok(cost)
    }
    
    /// Calculate execution fee (what the executor cooperative receives)
    fn calculate_execution_fee(&self, request: &TaskRequest) -> Result<u64> {
        // Executor gets 80% of the total cost
        let total_cost = self.calculate_resource_cost(request)?;
        let fee = (total_cost * 80) / 100;
        Ok(fee)
    }
    
    /// Get the cooperative ID for a node
    async fn get_coop_for_node(&self, node_did: &Did) -> Result<String> {
        let manifest_opt = self.cap_index.get_manifest(node_did).await;
        
        if let Some((manifest, _)) = manifest_opt {
            // Check for cooperative ID in various fields
            // Since NodeManifest doesn't have a metadata field directly,
            // check other relevant fields that might contain this information
            
            // Check in trust_fw_hash, which might encode the coop ID
            if manifest.trust_fw_hash.contains("coop:") {
                let parts: Vec<&str> = manifest.trust_fw_hash.split("coop:").collect();
                if parts.len() > 1 {
                    let coop_part = parts[1].split(":").next().unwrap_or("unknown");
                    return Ok(coop_part.to_string());
                }
            }
            
            // Check mesh protocols for coop info
            for protocol in &manifest.mesh_protocols {
                if protocol.starts_with("coop:") {
                    return Ok(protocol.trim_start_matches("coop:").to_string());
                }
            }
            
            // Fallback: use the first part of the DID as the coop ID
            let did_str = node_did.to_string();
            let parts: Vec<&str> = did_str.split(':').collect();
            if parts.len() >= 3 {
                let possible_coop = parts[2];
                if possible_coop.contains('-') {
                    let coop_parts: Vec<&str> = possible_coop.split('-').collect();
                    if !coop_parts.is_empty() {
                        return Ok(coop_parts[0].to_string());
                    }
                }
                return Ok(possible_coop.to_string());
            }
            
            // Ultimate fallback
            Ok("unknown-coop".to_string())
        } else {
            Err(anyhow!("Could not find manifest for node {}", node_did))
        }
    }
}

// Utility function to create an empty signature DAG node
fn create_empty_signed_node(node: DagNode) -> SignedDagNode {
    // Create an empty signature (all zeros)
    // Use try_from instead of from_bytes to handle errors correctly
    let empty_sig = Signature::try_from([0u8; 64].as_ref())
        .expect("Invalid empty signature data");
    
    SignedDagNode {
        node,
        signature: empty_sig,
        cid: None,
    }
}

// Utility function to create a properly signed DAG node
fn create_signed_node(node: DagNode, did_key: &DidKey) -> Result<SignedDagNode, anyhow::Error> {
    let node_bytes = serde_json::to_vec(&node)
        .context("Failed to serialize node")?;
    
    let signature = did_key.sign(&node_bytes);
    
    Ok(SignedDagNode {
        node,
        signature,
        cid: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_identity_core::manifest::{
        NodeManifest, EnergyInfo, GpuProfile
    };
    use icn_types::dag::memory::MemoryDagStore;
    
    async fn create_test_scheduler() -> (Scheduler, Arc<CapabilityIndex>) {
        // Create a memory DagStore and wrap it in SharedDagStore
        let memory_store = MemoryDagStore::new();
        let dag_store = SharedDagStore::new(Box::new(memory_store) as Box<dyn DagStore + Send + Sync>);
        
        // Create the capability index with shared store
        let cap_index = Arc::new(CapabilityIndex::new(dag_store.clone()));
        
        // Add some test manifests
        let manifest1 = NodeManifest {
            did: "did:icn:node1".into(),
            arch: IdArchitecture::X86_64,
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
            energy_profile: IdEnergyInfo {
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
            arch: IdArchitecture::Arm64,
            cores: 4,
            gpu: None,
            ram_mb: 8192,
            storage_bytes: 500_000_000_000, // 500GB
            sensors: vec![],
            actuators: vec![],
            energy_profile: IdEnergyInfo {
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
        let mut selector = MeshCapabilitySelector::new();
        selector.arch = Some(IdArchitecture::X86_64);
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_ok());
        
        let match_result = result.unwrap();
        // Use a proper Did object for comparison
        let expected_did: Did = "did:icn:node1".into();
        assert_eq!(match_result.bid.bidder, expected_did);
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
        let mut selector = MeshCapabilitySelector::new();
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
        // Use a proper Did object for comparison
        let expected_did: Did = "did:icn:node1".into();
        assert_eq!(match_result.bid.bidder, expected_did);
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
        let mut selector = MeshCapabilitySelector::new();
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
        // Use a proper Did object for comparison
        let expected_did: Did = "did:icn:node1".into();
        assert_eq!(match_result.bid.bidder, expected_did);
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
        let mut selector = MeshCapabilitySelector::new();
        selector.min_cores = Some(32); // Much more than any node has
        
        let result = scheduler.dispatch(request, Some(selector)).await;
        assert!(result.is_err());
    }
} 