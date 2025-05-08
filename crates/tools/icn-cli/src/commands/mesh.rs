use clap::{Args, Subcommand, ValueHint};
use serde_json;
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use sysinfo::{System, SystemExt};
use std::collections::HashMap;
use anyhow::{anyhow, Result, Context};

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::Cid;

use planetary_mesh::types::{
    NodeCapability,
    NodeCapabilityInfo,
    ResourceType,
    Bid,
};

use planetary_mesh::{JobManifest, JobStatus};
use icn_core_types::Did;
use std::str::FromStr;
use serde::{Serialize, Deserialize};
use icn_identity_core::{
    did::DidKey,
    vc::{VerifiableCredential, Proof},
};
use icn_types::dag::{DagNodeBuilder, DagPayload, SignedDagNode, SharedDagStore};
use uuid::Uuid;
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE;
use hex;
use rand;
use rand::Rng;
use tokio;
use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use sys_info;

/// Convert a byte slice to a 32-byte array for ed25519 keys
fn to_32_bytes(slice: &[u8]) -> Result<[u8; 32], anyhow::Error> {
    if slice.len() != 32 {
        return Err(anyhow::anyhow!("Expected 32 bytes for ed25519 key, got {}", slice.len()));
    }
    
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(slice);
    Ok(bytes)
}

/// Commands for interacting with the ICN Mesh
#[derive(Subcommand, Debug, Clone)]
pub enum MeshCommands {
    /// Submit a new job to the mesh (from a manifest file or inline).
    SubmitJob(SubmitJobArgs),
    /// List known capable nodes, with optional filtering.
    ListNodes(ListNodesArgs),
    /// Get the status of a submitted job.
    JobStatus(JobStatusArgs),
    /// Get bids for a specific job.
    GetBids(GetBidsArgs),
    /// Advertise this node's capabilities to the mesh network.
    AdvertiseCapability(AdvertiseCapabilityArgs),
    /// Submit a bid for a job on the mesh network.
    SubmitBid(SubmitBidArgs),
    /// Select and accept a bid for execution.
    SelectBid(SelectBidArgs),
    /// Verify an execution receipt.
    VerifyReceipt(VerifyReceiptArgs),
    /// Check token balances.
    CheckBalance(CheckBalanceArgs),
}

/// Arguments for the submit-job command.
#[derive(Args, Debug, Clone)]
pub struct SubmitJobArgs {
    /// Path to the job manifest file (TOML or JSON format)
    #[clap(long, value_hint = ValueHint::FilePath)]
    manifest_path: Option<PathBuf>,
    
    /// Inline WASM module CID (Content Identifier)
    #[clap(long, conflicts_with = "manifest_path")]
    wasm_module_cid: Option<String>,
    
    /// Memory requirement in MB
    #[clap(long, conflicts_with = "manifest_path")]
    memory_mb: Option<u64>,
    
    /// CPU cores requirement
    #[clap(long, conflicts_with = "manifest_path")]
    cpu_cores: Option<u32>,
    
    /// Path to key file for signing the job submission
    #[clap(long, value_hint = ValueHint::FilePath)]
    key_path: PathBuf,
    
    /// Federation ID to submit the job to (optional)
    #[clap(long)]
    federation_id: Option<String>,
    
    /// Additional parameters as JSON string
    #[clap(long, conflicts_with = "manifest_path")]
    params: Option<String>,
}

/// Arguments for getting bids for a job.
#[derive(Args, Debug, Clone)]
pub struct GetBidsArgs {
    /// The job ID (or CID) to get bids for
    #[clap(long)]
    job_id: String,
    
    /// Maximum number of bids to return
    #[clap(long, default_value = "10")]
    limit: usize,
    
    /// Sort bids by price, score, or time
    #[clap(long, default_value = "score", value_parser = ["score", "price", "time"])]
    sort_by: String,
}

/// Arguments for selecting a bid.
#[derive(Args, Debug, Clone)]
pub struct SelectBidArgs {
    /// The job ID (or CID) 
    #[clap(long)]
    job_id: String,
    
    /// The bid ID to accept
    #[clap(long)]
    bid_id: String,
    
    /// Path to key file for signing the bid selection
    #[clap(long, value_hint = ValueHint::FilePath)]
    key_path: PathBuf,
}

/// Arguments for list-nodes command.
#[derive(Args, Debug, Clone)]
pub struct ListNodesArgs {
    /// Filter by specific resource capability
    #[clap(long)]
    has_resource: Option<String>,
    
    /// Filter by minimum memory requirement (in MB)
    #[clap(long)]
    min_memory: Option<u64>,
    
    /// Filter by minimum CPU cores
    #[clap(long)]
    min_cores: Option<u32>,
    
    /// Filter by federation ID
    #[clap(long)]
    federation_id: Option<String>,
    
    /// Maximum number of nodes to display
    #[clap(long, default_value = "10")]
    limit: usize,
}

/// Arguments for job-status command.
#[derive(Args, Debug, Clone)]
pub struct JobStatusArgs {
    /// The Job ID to check status for
    #[clap(long)]
    job_id: String,
}

/// Arguments for submit-bid command.
#[derive(Args, Debug, Clone)]
pub struct SubmitBidArgs {
    /// The job ID to bid on
    #[clap(long)]
    job_id: String,
    
    /// Path to key file for signing the bid
    #[clap(long, value_hint = ValueHint::FilePath)]
    key_path: PathBuf,
    
    /// Price in tokens for this bid
    #[clap(long)]
    price: u64,
    
    /// Confidence score (0.0 - 1.0)
    #[clap(long, default_value = "0.95")]
    confidence: f32,
}

/// Arguments for advertise-capability command.
#[derive(Args, Debug, Clone)]
pub struct AdvertiseCapabilityArgs {
    /// Path to capability manifest file
    #[clap(long, value_hint = ValueHint::FilePath)]
    manifest_path: Option<PathBuf>,
    
    /// Available memory in MB
    #[clap(long, conflicts_with = "manifest_path")]
    memory_mb: Option<u64>,
    
    /// Available CPU cores
    #[clap(long, conflicts_with = "manifest_path")]
    cpu_cores: Option<u32>,
    
    /// Path to key file for signing the capability advertisement
    #[clap(long, value_hint = ValueHint::FilePath)]
    key_path: PathBuf,
}

/// Struct for verify-receipt command arguments
#[derive(Debug, Args, Clone)]
pub struct VerifyReceiptArgs {
    /// The receipt CID to verify
    #[clap(long)]
    receipt_cid: String,
}

/// Arguments for check-balance command.
#[derive(Args, Debug, Clone)]
pub struct CheckBalanceArgs {
    /// Path to key file to check balance for
    #[clap(long, value_hint = ValueHint::FilePath)]
    key_path: PathBuf,
    
    /// Federation ID to check balance in
    #[clap(long)]
    federation_id: Option<String>,
}

/// Handle the mesh subcommands.
pub async fn handle_mesh_command(cmd: MeshCommands, ctx: &CliContext) -> Result<()> {
    match cmd {
        MeshCommands::SubmitJob(args) => handle_submit_job(args, ctx).await,
        MeshCommands::ListNodes(args) => handle_list_nodes(args, ctx).await,
        MeshCommands::JobStatus(args) => handle_job_status(args, ctx).await,
        MeshCommands::GetBids(args) => handle_get_bids(args, ctx).await,
        MeshCommands::AdvertiseCapability(args) => handle_advertise_capability(args, ctx).await,
        MeshCommands::SubmitBid(args) => handle_submit_bid(args, ctx).await,
        MeshCommands::SelectBid(args) => handle_select_bid(args, ctx).await,
        MeshCommands::VerifyReceipt(args) => handle_verify_receipt(args, ctx).await,
        MeshCommands::CheckBalance(args) => handle_check_balance(args, ctx).await,
    }
}

/// JobCredential represents a job submission as a verifiable credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobCredential {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: Vec<String>,
    pub issuer: String,
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    #[serde(rename = "credentialSubject")]
    pub credential_subject: JobSubject,
    pub proof: Option<Proof>,
}

/// JobSubject contains the actual job details in the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobSubject {
    pub id: String, // Job ID
    pub wasm_module_cid: String,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub parameters: serde_json::Value,
    pub owner: String, // Owner DID
    pub deadline: Option<String>,
    pub federation_id: Option<String>,
}

/// ResourceRequirement is a named variant for resource types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceRequirement {
    pub type_: String,
    pub value: serde_json::Value,
}

// Add a new SignedJobManifest struct that wraps JobManifest with signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedJobManifest {
    pub manifest: JobManifest,
    pub signature: String,
    pub signer: Did,
}

impl SignedJobManifest {
    /// Create a new signed job manifest by signing a JobManifest with the provided private key
    pub fn new(manifest: JobManifest, private_key: &[u8], signer_did: Did) -> Result<Self, anyhow::Error> {
        // Convert the manifest to bytes for signing
        let manifest_bytes = serde_json::to_vec(&manifest)
            .map_err(|e| anyhow!("Failed to serialize job manifest: {}", e))?;
        
        // Create a signature using the private key
        let key_bytes = to_32_bytes(private_key)?;
        let key_pair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
        
        let signature = key_pair.sign(&manifest_bytes);
        let signature_b64 = BASE64_ENGINE.encode(signature.to_bytes());
        
        Ok(SignedJobManifest {
            manifest,
            signature: signature_b64,
            signer: signer_did,
        })
    }
    
    /// Verify that the signature matches the manifest and was signed by the claimed signer
    pub fn verify(&self) -> Result<bool, anyhow::Error> {
        // Deserialize the manifest to bytes
        let manifest_bytes = serde_json::to_vec(&self.manifest)
            .map_err(|e| anyhow!("Failed to serialize job manifest: {}", e))?;
        
        // Get the public key from the signer's DID
        let did_string = self.signer.to_string();
        let key_parts: Vec<&str> = did_string.split(':').collect();
        if key_parts.len() < 4 {
            return Err(anyhow!("Invalid DID format"));
        }
        
        let key_part = key_parts[3];
        let multibase_decoded = multibase::decode(key_part)
            .map_err(|e| anyhow!("Failed to decode key part: {}", e))?;
        
        // Verify the signature
        let signature_bytes = BASE64_ENGINE.decode(self.signature.as_bytes())
            .map_err(|e| anyhow!("Failed to decode signature: {}", e))?;
        
        let signature = ed25519_dalek::Signature::from_bytes(
            &signature_bytes.try_into()
                .map_err(|_| anyhow!("Invalid signature format"))?
        );
        
        // Extract the public key from the multibase decoded data
        let key_bytes = if multibase_decoded.1[0] == 0xed && multibase_decoded.1[1] == 0x01 {
            &multibase_decoded.1[2..]
        } else {
            return Err(anyhow!("Unsupported key type"));
        };
        
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(
            &key_bytes.try_into()
                .map_err(|_| anyhow!("Invalid public key"))?
        ).map_err(|e| anyhow!("Invalid public key: {}", e))?;
        
        // Verify the signature
        Ok(verifying_key.verify(&manifest_bytes, &signature).is_ok())
    }
}

/// Convert a resource name and value to the appropriate ResourceType enum variant
fn parse_resource_type(name: String, value: serde_json::Value) -> Option<ResourceType> {
    match name.as_str() {
        "ram_mb" | "memory" => {
            if let Some(value) = value.as_u64() {
                Some(ResourceType::RamMb(value))
            } else {
                None
            }
        },
        "cpu_cores" | "cpu" => {
            if let Some(value) = value.as_u64() {
                Some(ResourceType::CpuCores(value))
            } else {
                None
            }
        },
        "gpu_cores" | "gpu" => {
            if let Some(value) = value.as_u64() {
                Some(ResourceType::GpuCores(value))
            } else {
                None
            }
        },
        "storage_mb" | "storage" => {
            if let Some(value) = value.as_u64() {
                Some(ResourceType::StorageMb(value))
            } else {
                None
            }
        },
        _ => None
    }
}

/// Handle the submit-job command.
async fn handle_submit_job(
    args: SubmitJobArgs, 
    ctx: &CliContext
) -> Result<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .map_err(|e| anyhow!("Failed to read key file: {}", e))?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .map_err(|e| anyhow!("Invalid key file format, expected JSON: {}", e))?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| anyhow!("DID not found in key file"))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| anyhow!("Private key not found in key file"))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .map_err(|e| anyhow!("Invalid private key hex format: {}", e))?;
    
    let owner_did = Did::from_str(did)
        .map_err(|e| anyhow!("Invalid DID format: {}", e))?;
    
    // Create job manifest either from file or inline arguments
    let job_manifest = if let Some(manifest_path) = &args.manifest_path {
        // Read from file
        let manifest_content = fs::read_to_string(manifest_path)
            .map_err(|e| anyhow!("Failed to read manifest file: {}", e))?;
        
        // Parse based on file extension
        if manifest_path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            toml::from_str::<JobManifest>(&manifest_content)
                .map_err(|e| anyhow!("Failed to parse TOML manifest: {}", e))?
        } else {
            serde_json::from_str::<JobManifest>(&manifest_content)
                .map_err(|e| anyhow!("Failed to parse JSON manifest: {}", e))?
        }
    } else {
        // Create from inline arguments
        let wasm_module_cid = args.wasm_module_cid
            .ok_or_else(|| anyhow!("WASM module CID is required"))?;
        
        // Parse resource requirements
        let mut resource_requirements = HashMap::new();
        if let Some(memory) = args.memory_mb {
            resource_requirements.insert("memory".to_string(), serde_json::json!(memory));
        }
        
        if let Some(cores) = args.cpu_cores {
            resource_requirements.insert("cpu".to_string(), serde_json::json!(cores));
        }
        
        let params = if let Some(params_str) = &args.params {
            serde_json::from_str(params_str)
                .map_err(|e| anyhow!("Failed to parse parameters JSON: {}", e))?
        } else {
            serde_json::Value::Null
        };
        
        JobManifest {
            id: format!("job-{}", Uuid::new_v4()),
            wasm_module_cid,
            resource_requirements: resource_requirements.into_iter()
                .filter_map(|(k, v)| parse_resource_type(k, v))
                .collect(),
            parameters: params,
            owner: owner_did.to_string(),
            deadline: None,
            federation_id: args.federation_id.unwrap_or_else(|| "default".to_string()),
            max_compute_units: Some(1000), // Default value
            origin_coop_id: "default".to_string(), // Default value
        }
    };
    
    // Serialize the job manifest
    let manifest_json = serde_json::to_string_pretty(&job_manifest)
        .map_err(|e| anyhow!("Failed to serialize job manifest: {}", e))?;

    // Create a signed manifest
    let signed_manifest = SignedJobManifest::new(
        job_manifest, 
        &private_key_bytes,
        owner_did.clone()
    )?;
    
    // Verify signature (self-validation)
    if !signed_manifest.verify()? {
        return Err(anyhow!("Failed to self-verify job signature"));
    }
    
    // Create a verifiable credential
    let vc = VerifiableCredential {
        context: vec![
            "https://www.w3.org/2018/credentials/v1".to_string(),
            "https://icn.network/credentials/mesh/v1".to_string(),
        ],
        id: Some(format!("urn:uuid:{}", Uuid::new_v4())),
        type_: vec!["VerifiableCredential".to_string(), "JobSubmission".to_string()],
        issuer: owner_did.clone(),
        issuance_date: Utc::now(),
        credential_subject: serde_json::to_value(&signed_manifest)
            .map_err(|e| anyhow!("Failed to convert signed manifest to JSON value: {}", e))?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", owner_did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signed_manifest.signature.clone(),
        }),
    };
    
    // Convert signed manifest to JSON
    let signed_json = serde_json::to_value(&signed_manifest)
        .map_err(|e| anyhow!("Failed to convert signed manifest to JSON value: {}", e))?;

    // Serialize verifiable credential
    let vc_json = serde_json::to_string_pretty(&vc)
        .map_err(|e| anyhow!("Failed to serialize verifiable credential: {}", e))?;

    // Save to a file for demonstration purposes
    let output_path = format!("job_submission_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .map_err(|e| anyhow!("Failed to write submission to file: {}", e))?;
    
    println!("Job manifest signed and submitted successfully!");
    println!("Job ID: {}", signed_manifest.manifest.id);
    println!("Saved to: {}", output_path);
    
    Ok(())
}

/// Handle the list-nodes command.
async fn handle_list_nodes(args: ListNodesArgs, ctx: &CliContext) -> Result<()> {
    println!("Listing capable nodes in the network:");
    
    // For demo purposes, we'll create some sample nodes
    #[derive(Debug)]
    struct NodeInfo {
        node_id: Did,
        capabilities: Vec<NodeCapability>,
        supported_features: Vec<String>,
    }
    
    let nodes = vec![
        NodeInfo {
            node_id: Did::from_str("did:icn:node:12345")
                .map_err(|e| anyhow!("Invalid node DID: {}", e))?,
            capabilities: vec![
                NodeCapability {
                    resource_type: ResourceType::CpuCores(4),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::RamMb(2048),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::StorageMb(500 * 1024),
                    available: true,
                    updated_at: Utc::now(),
                },
            ],
            supported_features: vec!["wasm".to_string(), "sgx".to_string()],
        },
        NodeInfo {
            node_id: Did::from_str("did:icn:node:67890")
                .map_err(|e| anyhow!("Invalid node DID: {}", e))?,
            capabilities: vec![
                NodeCapability {
                    resource_type: ResourceType::CpuCores(2),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::RamMb(1024),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::StorageMb(200 * 1024),
                    available: true,
                    updated_at: Utc::now(),
                },
            ],
            supported_features: vec!["wasm".to_string()],
        },
        NodeInfo {
            node_id: Did::from_str("did:icn:node:abcde")
                .map_err(|e| anyhow!("Invalid node DID: {}", e))?,
            capabilities: vec![
                NodeCapability {
                    resource_type: ResourceType::CpuCores(8),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::RamMb(4096),
                    available: true,
                    updated_at: Utc::now(),
                },
                NodeCapability {
                    resource_type: ResourceType::StorageMb(1000 * 1024),
                    available: true,
                    updated_at: Utc::now(),
                },
            ],
            supported_features: vec!["wasm".to_string(), "sgx".to_string(), "gpu".to_string()],
        },
    ];
    
    // Filtering code below needs to be updated to use the new structure
    let filtered_nodes = nodes.into_iter()
        .filter(|node| {
            // Filter by minimum memory
            if let Some(min_memory) = args.min_memory {
                if !node.capabilities.iter().any(|cap| {
                    if let ResourceType::RamMb(ram) = cap.resource_type {
                        ram >= min_memory
                    } else {
                        false
                    }
                }) {
                    return false;
                }
            }
            
            // Filter by minimum cores
            if let Some(min_cores) = args.min_cores {
                if !node.capabilities.iter().any(|cap| {
                    if let ResourceType::CpuCores(cores) = cap.resource_type {
                        cores >= min_cores.into()
                    } else {
                        false
                    }
                }) {
                    return false;
                }
            }
            
            // Filter by specific resource
            if let Some(resource) = &args.has_resource {
                if resource.to_lowercase() == "gpu" {
                    // Check for GPU in supported_features
                    if !node.supported_features.iter().any(|f| f.to_lowercase() == "gpu") {
                        return false;
                    }
                }
            }
            
            true
        })
        .take(args.limit)
        .collect::<Vec<_>>();

    // Update the display code as well
    println!("Found {} nodes:", filtered_nodes.len());
    println!("{:<25} {:<30} {:<20}", "Node ID", "Resources", "Features");
    println!("{:-<75}", "");

    for node in filtered_nodes {
        let resources = node.capabilities.iter()
            .map(|cap| match cap.resource_type {
                ResourceType::CpuCores(cores) => format!("{}c", cores),
                ResourceType::RamMb(ram) => format!("{}MB", ram),
                ResourceType::StorageMb(storage) => format!("{}MB storage", storage),
                _ => "unknown".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ");

        let features = node.supported_features.join(", ");

        println!("{:<25} {:<30} {:<20}",
            node.node_id.to_string(),
            resources,
            features
        );
    }

    Ok(())
}

/// Handle the job-status command.
async fn handle_job_status(args: JobStatusArgs, ctx: &CliContext) -> Result<()> {
    println!("Checking status for job: {}", args.job_id);
    
    // In a real implementation, this would query the network or local store
    // For demo purposes, we'll simulate a job status
    
    // Generate a random status
    let status = match rand::thread_rng().gen_range(0..5) {
        0 => JobStatus::Submitted,
        1 => JobStatus::Scheduled,
        2 => JobStatus::Running,
        3 => {
            let result_cid = Cid::from_str("bafybeihykld7uyxzogax6vgyvag42y7464eywpf55hnrwvgzxwvjmnx7fy")
                .map_err(|e| anyhow!("Failed to parse CID: {}", e))?;
            JobStatus::Completed
        },
        _ => JobStatus::Failed,
    };
    
    println!("Status: {:?}", status);
    
    match status {
        JobStatus::Running => {
            println!("Progress: 45%"); // Simplified since JobStatus::Running doesn't have progress_percent
            println!("Estimated completion: 5 seconds");
        },
        JobStatus::Completed => {
            // In a real implementation, look up the result CID from a separate mapping
            let result_cid = "bafybeihykld7uyxzogax6vgyvag42y7464eywpf55hnrwvgzxwvjmnx7fy";
            println!("Result CID: {}", result_cid);
            println!("Use 'icn-cli mesh get-result --result-cid {}' to retrieve the result", result_cid);
        },
        JobStatus::Failed => {
            println!("Error: Job execution failed");
        },
        _ => {}
    }
    
    Ok(())
}

/// BidDetails represents a single bid response for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BidDetails {
    /// The bidder's DID
    pub bidder_did: String,
    
    /// Bid CID in the DAG
    pub bid_cid: String,
    
    /// Bid price/cost
    pub price: u64,
    
    /// Offered latency in milliseconds
    pub latency: Option<u64>,
    
    /// Available memory in MB
    pub memory: u64,
    
    /// Available CPU cores
    pub cores: u64,
    
    /// Bidder's reputation score
    pub reputation: Option<u8>,
    
    /// Renewable energy percentage
    pub renewable: Option<u8>,
    
    /// Comment
    pub comment: Option<String>,
    
    /// Timestamp 
    pub timestamp: DateTime<Utc>,
}

async fn handle_get_bids(args: GetBidsArgs, ctx: &CliContext) -> Result<()> {
    println!("Fetching bids for job: {}", args.job_id);
    
    // 1. Parse the job CID/ID
    let job_id = args.job_id.clone();
    
    // 2. Check if we're dealing with a CID or an ID
    let is_cid = job_id.starts_with("bafy") || job_id.starts_with("Qm");
    
    if is_cid {
        println!("Interpreting '{}' as a content identifier (CID)", job_id);
    } else {
        println!("Interpreting '{}' as a job ID", job_id);
    }
    
    // 3. Fetch bids from the mesh network
    // For demo purposes, we'll create some sample bids
    let bids = vec![
        BidDetails {
            bidder_did: "did:icn:node:12345".to_string(),
            bid_cid: "bafybeihykld7uyxzogax6vgyvag42y7464eywpf55hnrwvgzxwvjmnx7fy".to_string(),
            price: 100,
            latency: Some(50),
            memory: 2048,
            cores: 4,
            reputation: Some(95),
            renewable: Some(80),
            comment: Some("High performance node with green energy".to_string()),
            timestamp: Utc::now(),
        },
        BidDetails {
            bidder_did: "did:icn:node:67890".to_string(),
            bid_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            price: 80,
            latency: Some(100),
            memory: 1024,
            cores: 2,
            reputation: Some(85),
            renewable: Some(30),
            comment: Some("Budget-friendly option".to_string()),
            timestamp: Utc::now(),
        },
        BidDetails {
            bidder_did: "did:icn:node:abcde".to_string(),
            bid_cid: "bafybeihdwdcefgh4dfw4ls5fhwrvqxt225tczacdwvqxtdxgpqr3efghij".to_string(),
            price: 150,
            latency: Some(30),
            memory: 4096,
            cores: 8,
            reputation: Some(98),
            renewable: Some(100),
            comment: Some("Premium node with 100% renewable energy".to_string()),
            timestamp: Utc::now(),
        },
    ];
    
    // 4. Display the bids
    println!("\nFound {} bids:", bids.len());
    println!("{:<15} {:<10} {:<8} {:<8} {:<6} {:<10} {:<10}", 
        "Price", "Latency", "Memory", "Cores", "Rep", "Renewable", "Bidder");
    println!("{:-<70}", "");
    
    for bid in bids {
        println!("{:<15} {:<10} {:<8} {:<8} {:<6} {:<10} {:<10}",
            format!("{} COMPUTE", bid.price),
            bid.latency.map_or("N/A".into(), |l| format!("{}ms", l)),
            format!("{}MB", bid.memory),
            bid.cores,
            bid.reputation.map_or("N/A".into(), |r| format!("{}%", r)),
            bid.renewable.map_or("N/A".into(), |r| format!("{}%", r)),
            bid.bidder_did
        );
        
        if let Some(comment) = bid.comment {
            println!("Comment: {}", comment);
        }
        println!("Bid CID: {}", bid.bid_cid);
        println!();
    }
    
    Ok(())
}

/// DispatchCredential represents a job dispatch as a verifiable credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DispatchCredential {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: Vec<String>,
    pub issuer: String,
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    #[serde(rename = "credentialSubject")]
    pub credential_subject: DispatchSubject,
    pub proof: Option<Proof>,
}

/// DispatchSubject contains the actual dispatch details in the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DispatchSubject {
    pub id: String, // Dispatch ID
    pub job_id: String, // Job ID
    pub wasm_module_cid: String,
    pub node_id: String, // DID of the selected node
    pub bid_cid: String, // CID of the selected bid
    pub requester: String, // DID of the job requester
    pub timestamp: DateTime<Utc>,
    pub federation_id: Option<String>,
}

/// Handle the select-bid command.
async fn handle_select_bid(
    args: SelectBidArgs, 
    ctx: &CliContext
) -> Result<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .map_err(|e| anyhow!("Failed to read key file: {}", e))?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .map_err(|e| anyhow!("Invalid key file format, expected JSON: {}", e))?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("Private key not found in key file".into()))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .map_err(|e| anyhow!("Invalid private key hex format: {}", e))?;
    
    let requester_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    // In a real implementation, this would look up the actual job and bid from the network
    // For demo purposes, we'll simulate accepting a specific bid
    
    // Read the previously saved bids file
    let bids_file = format!("bids_for_job_{}.json", args.job_id);
    let bids_data = match fs::read_to_string(&bids_file) {
        Ok(data) => data,
        Err(_) => {
            return Err(anyhow!(
                "Bids file not found. Run 'icn-cli mesh get-bids --job-id {}' first",
                args.job_id
            ));
        }
    };
    
    let bids: Vec<Bid> = serde_json::from_str(&bids_data)
        .map_err(|e| anyhow!("Failed to parse bids file: {}", e))?;
    
    // Find the selected bid
    let bid_index = args.bid_id.parse::<usize>()
        .map_err(|_| CliError::InvalidArgument("Bid ID must be a number".into()))?;
    
    if bid_index == 0 || bid_index > bids.len() {
        return Err(anyhow!(
            "Invalid bid ID. Must be between 1 and {}", bids.len()
        ));
    }
    
    let selected_bid = &bids[bid_index - 1];
    
    // Create the bid acceptance credential
    #[derive(Serialize, Deserialize)]
    struct BidAcceptance {
        job_id: String,
        bid_index: usize,
        bidder_did: String,
        price: u64,
        timestamp: DateTime<Utc>,
    }
    
    let acceptance = BidAcceptance {
        job_id: args.job_id.clone(),
        bid_index,
        bidder_did: selected_bid.node_id.to_string(),
        price: selected_bid.price,
        timestamp: Utc::now(),
    };
    
    // Sign the acceptance
    let acceptance_json = serde_json::to_string(&acceptance)
        .map_err(|e| anyhow!("Failed to serialize bid acceptance: {}", e))?;
    
    let key_bytes = to_32_bytes(private_key_bytes.as_slice())?;
    let key_pair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
    let signature = key_pair.sign(acceptance_json.as_bytes());
    let signature_b64 = BASE64_ENGINE.encode(signature.to_bytes());
    
    // Create verifiable credential
    let vc = VerifiableCredential {
        context: vec![
            "https://www.w3.org/2018/credentials/v1".to_string(),
            "https://icn.network/credentials/mesh/v1".to_string(),
        ],
        id: Some(format!("urn:uuid:{}", Uuid::new_v4())),
        type_: vec!["VerifiableCredential".to_string(), "BidAcceptance".to_string()],
        issuer: requester_did.clone(),
        issuance_date: Utc::now(),
        credential_subject: serde_json::to_value(&acceptance)
            .map_err(|e| anyhow!("Failed to convert acceptance to JSON value: {}", e))?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", requester_did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signature_b64,
        }),
    };
    
    // Serialize verifiable credential
    let vc_json = serde_json::to_string_pretty(&vc)
        .map_err(|e| anyhow!("Failed to serialize verifiable credential: {}", e))?;
    
    // Save to a file for demonstration purposes
    let output_path = format!("bid_acceptance_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .map_err(|e| anyhow!("Failed to write acceptance to file: {}", e))?;
    
    // Create an execution receipt to simulate job completion
    println!("Bid #{} for job {} accepted successfully!", bid_index, args.job_id);
    println!("Provider: {}", selected_bid.node_id);
    println!("Price: {} tokens", selected_bid.price);
    println!("Acceptance credential saved to: {}", output_path);
    
    // Simulate job execution with a brief delay
    println!("\nSimulating job execution...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Create execution receipt
    simulate_execution_receipt(&args.job_id, selected_bid, &output_path)
        .map_err(|e| anyhow!("Failed to create execution receipt: {}", e))?;
    
    println!("\nJob execution complete! Results and execution receipt have been saved.");
    
    Ok(())
}

/// Simulate the creation of an execution receipt for demonstration purposes.
fn simulate_execution_receipt(job_id: &str, bid: &Bid, acceptance_path: &str) -> Result<()> {
    // Create a sample result and receipt
    let execution_time_ms = rand::thread_rng().gen_range(500..3000);
    let memory_peak_mb = rand::thread_rng().gen_range(256..2048); // Use fixed range instead
    
    let cpu_usage_pct = rand::thread_rng().gen_range(40..95);
    
    // Generate a random result hash
    let mut result_hash = [0u8; 32];
    rand::thread_rng().fill(&mut result_hash);
    
    #[derive(Serialize)]
    struct ExecutionResult {
        job_id: String,
        bid_id: String,
        result_hash: String,
        execution_metrics: ExecutionMetrics,
        token_compensation: TokenCompensation,
    }
    
    #[derive(Serialize)]
    struct ExecutionMetrics {
        execution_time_ms: u64,
        memory_peak_mb: u64,
        cpu_usage_pct: u8,
        io_read_bytes: u64,
        io_write_bytes: u64,
    }
    
    #[derive(Serialize)]
    struct TokenCompensation {
        amount: f64,
        token_type: String,
        from: String,
        to: String,
        timestamp: DateTime<Utc>,
    }
    
    let result = ExecutionResult {
        job_id: job_id.to_string(),
        bid_id: bid.node_id.to_string(),
        result_hash: hex::encode(result_hash),
        execution_metrics: ExecutionMetrics {
            execution_time_ms,
            memory_peak_mb,
            cpu_usage_pct,
            io_read_bytes: 6242880,
            io_write_bytes: 2197152,
        },
        token_compensation: TokenCompensation {
            amount: bid.price as f64,
            token_type: "COMPUTE".to_string(),
            from: "did:icn:requester".to_string(),
            to: bid.node_id.to_string(),
            timestamp: Utc::now(),
        },
    };
    
    // Save the result
    let result_json = serde_json::to_string_pretty(&result)
        .map_err(|e| anyhow!("Failed to serialize execution result: {}", e))?;
    
    let result_path = format!("execution_result_{}.json", job_id);
    fs::write(&result_path, &result_json)
        .map_err(|e| anyhow!("Failed to write execution result to file: {}", e))?;
    
    println!("Execution Result:");
    println!("  Execution time: {} ms", execution_time_ms);
    println!("  Memory peak: {} MB", memory_peak_mb);
    println!("  CPU usage: {}%", cpu_usage_pct);
    println!("  Result hash: {}", hex::encode(&result_hash[0..8]));
    println!("  Token compensation: {} COMPUTE", bid.price);
    println!("\nExecution result saved to: {}", result_path);
    
    Ok(())
}

/// Handle the advertise-capability command.
pub async fn handle_advertise_capability(args: AdvertiseCapabilityArgs, ctx: &CliContext) -> Result<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .map_err(|e| anyhow!("Failed to read key file: {}", e))?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .map_err(|e| anyhow!("Invalid key file format, expected JSON: {}", e))?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let node_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;

    // Get system memory information
    let mut sys = System::new_all();
    sys.refresh_all();
    let total_memory_mb = sys.total_memory() / 1024; // Convert KB to MB

    // Create a NodeCapabilityInfo struct for advertising capabilities
    let capabilities = NodeCapabilityInfo {
        node_id: node_did.clone(),
        available_resources: vec![
            ResourceType::RamMb(total_memory_mb as u64),
            ResourceType::CpuCores(num_cpus::get() as u64),
            ResourceType::StorageMb(500 * 1024), // Default 500GB storage
        ],
        supported_features: vec!["wasm".to_string()],
    };

    // Display info about the capabilities
    println!("Capability advertisement created successfully!");
    println!("Node DID: {}", node_did);
    println!("Resources: {:?}", capabilities.available_resources);
    println!("Features: {:?}", capabilities.supported_features);

    // Serialize and save to file
    let output_path = format!("node_capability_{}.json", node_did.to_string().replace(":", "_"));
    let capabilities_json = serde_json::to_string_pretty(&capabilities)
        .map_err(|e| CliError::SerializationError(format!("Failed to serialize node capabilities: {}", e)))?;

    fs::write(&output_path, &capabilities_json)
        .map_err(|e| CliError::Io(e))?;

    println!("Node capabilities advertised successfully!");
    println!("Saved to: {}", output_path);

    Ok(())
}

/// Handle the submit-bid command.
async fn handle_submit_bid(args: SubmitBidArgs, ctx: &CliContext) -> Result<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .map_err(|e| anyhow!("Failed to read key file: {}", e))?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .map_err(|e| anyhow!("Invalid key file format, expected JSON: {}", e))?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| anyhow!("DID not found in key file"))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| anyhow!("Private key not found in key file"))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .map_err(|e| anyhow!("Invalid private key hex format: {}", e))?;
    
    let bidder_did = Did::from_str(did)
        .map_err(|e| anyhow!("Invalid DID format: {}", e))?;

    // Create a bid
    let bid = Bid {
        node_id: bidder_did.to_string(),
        coop_id: "default".to_string(),
        price: args.price,
        eta: Utc::now() + chrono::Duration::hours(1),
        submitted_at: Utc::now(),
    };

    // Sign the bid
    let bid_json = serde_json::to_string(&bid)
        .map_err(|e| anyhow!("Failed to serialize bid: {}", e))?;

    let key_bytes = to_32_bytes(private_key_bytes.as_slice())?;
    let key_pair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
    let signature = key_pair.sign(bid_json.as_bytes());
    let signature_b64 = BASE64_ENGINE.encode(signature.to_bytes());

    // Create a verifiable credential
    let vc = VerifiableCredential {
        context: vec![
            "https://www.w3.org/2018/credentials/v1".to_string(),
            "https://icn.network/credentials/mesh/v1".to_string(),
        ],
        id: Some(format!("urn:uuid:{}", Uuid::new_v4())),
        type_: vec!["VerifiableCredential".to_string(), "BidSubmission".to_string()],
        issuer: bidder_did.clone(),
        issuance_date: Utc::now(),
        credential_subject: serde_json::to_value(&bid)
            .map_err(|e| anyhow!("Failed to convert bid to JSON value: {}", e))?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            verification_method: format!("{}#keys-1", bidder_did),
            created: Utc::now(),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signature_b64,
        }),
    };

    // Serialize verifiable credential
    let vc_json = serde_json::to_string_pretty(&vc)
        .map_err(|e| anyhow!("Failed to serialize verifiable credential: {}", e))?;

    // Save to a file for demonstration purposes
    let output_path = format!("bid_submission_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .map_err(|e| anyhow!("Failed to write bid to file: {}", e))?;

    println!("Bid submitted successfully for job: {}", args.job_id);
    println!("Bidder: {}", bidder_did);
    println!("Price: {} tokens", args.price);
    println!("Bid saved to: {}", output_path);

    Ok(())
}

/// ExecutionReceipt represents a job execution receipt as a verifiable credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionReceipt {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: Vec<String>,
    pub issuer: String,
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    #[serde(rename = "credentialSubject")]
    pub credential_subject: ExecutionSubject,
    pub proof: Option<Proof>,
}

/// ExecutionSubject contains the actual execution details in the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionSubject {
    pub id: String, // Task requestor DID
    pub job_id: String, // Job ID
    pub wasm_module_cid: String,
    pub result_hash: String,
    pub execution_metrics: ExecutionMetrics,
    pub token_compensation: TokenCompensation,
}

/// ExecutionMetrics contains performance metrics for the execution
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionMetrics {
    pub execution_time_ms: u64,
    pub memory_peak_mb: u64,
    pub cpu_usage_pct: u8,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// TokenCompensation contains details of the token transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCompensation {
    pub amount: f64,
    pub token_type: String,
    pub from: String,
    pub to: String,
    pub timestamp: DateTime<Utc>,
}

/// Handle the verify-receipt command.
async fn handle_verify_receipt(args: VerifyReceiptArgs, ctx: &CliContext) -> Result<()> {
    // Read the receipt CID
    let receipt_cid = args.receipt_cid.clone();
    
    println!("Verifying execution receipt: {}", receipt_cid);
    
    // In a real implementation, this would retrieve the receipt from the DAG
    // and verify its cryptographic proof.
    // For demonstration purposes, we'll use a sample receipt.
    
    // Create a sample receipt
    let receipt = ExecutionReceipt {
        context: vec![
            "https://www.w3.org/2018/credentials/v1".to_string(),
            "https://icn.network/credentials/execution/v1".to_string(),
        ],
        id: format!("urn:uuid:{}", Uuid::new_v4()),
        type_: vec!["VerifiableCredential".to_string(), "ExecutionReceipt".to_string()],
        issuer: "did:icn:worker01".to_string(),
        issuance_date: Utc::now(),
        credential_subject: ExecutionSubject {
            id: "did:icn:requester".to_string(),
            job_id: "sample-job-001".to_string(),
            wasm_module_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            result_hash: "d74d97a8957c4b6a88704b8cb88adcd49d797f9ea5d8a091f31e2cdc".to_string(),
            execution_metrics: ExecutionMetrics {
                execution_time_ms: 1853,
                memory_peak_mb: 642,
                cpu_usage_pct: 78,
                io_read_bytes: 6242880,
                io_write_bytes: 2197152,
            },
            token_compensation: TokenCompensation {
                amount: 30.0,
                token_type: "COMPUTE".to_string(),
                from: "did:icn:requester".to_string(),
                to: "did:icn:worker01".to_string(),
                timestamp: Utc::now(),
            },
        },
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: "did:icn:worker01#keys-1".to_string(),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: "VALID_SIGNATURE_PLACEHOLDER".to_string(),
        }),
    };
    
    // Print the verification result
    println!("ExecutionReceipt verification successful!");
    println!("Receipt CID: {}", receipt_cid);
    println!("Task CID: QmTaskZ7xP9F2cj5YhN8YgWz5DrtGE3vTqMW9W");
    println!("Bid CID: QmBidX3pQ2KLmfG8s6NrZE4yT9jK5RwuVmFa");
    
    println!("\nCredential details:");
    println!("  Issuer: {}", receipt.issuer);
    println!("  Issuance date: {}", receipt.issuance_date);
    println!("  Subject ID: {}", receipt.credential_subject.id);
    
    println!("\nExecution metrics:");
    println!("  Execution time: {} ms", receipt.credential_subject.execution_metrics.execution_time_ms);
    println!("  Memory: {} MB", receipt.credential_subject.execution_metrics.memory_peak_mb);
    println!("  CPU usage: {}%", receipt.credential_subject.execution_metrics.cpu_usage_pct);
    println!("  I/O read: {} bytes", receipt.credential_subject.execution_metrics.io_read_bytes);
    println!("  I/O write: {} bytes", receipt.credential_subject.execution_metrics.io_write_bytes);
    println!("  Result hash: {}", receipt.credential_subject.result_hash);
    println!("  Token compensation: {} {}", 
             receipt.credential_subject.token_compensation.amount,
             receipt.credential_subject.token_compensation.token_type);
    
    Ok(())
}

/// Represents a token transfer between DIDs
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenTransfer {
    /// Transaction ID
    pub id: String,
    
    /// The token amount
    pub amount: f64,
    
    /// The token type
    pub token_type: String,
    
    /// Sender DID
    pub from: String,
    
    /// Receiver DID
    pub to: String,
    
    /// Transaction timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Optional reference to a job or task
    pub reference: Option<String>,
}

/// Handle the check-balance command.
async fn handle_check_balance(
    args: CheckBalanceArgs, 
    ctx: &CliContext
) -> Result<()> {
    // Read the key file
    let key_data = fs::read_to_string(&args.key_path)
        .context("Failed to read key file")?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .context("Invalid key file format, expected JSON")?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let did_obj = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    let federation_id = args.federation_id.unwrap_or_else(|| "solar-farm-coop".to_string());
    
    println!("Checking token balance for DID: {}", did);
    println!("Federation: {}", federation_id);
    
    // In a real implementation, this would query the ledger or DAG
    // For demo purposes, we'll use sample data based on the DID
    let is_worker = did.contains("worker");
    
    // Create sample transfers
    let transfers = if is_worker {
        vec![
            TokenTransfer {
                id: "tx-12345".to_string(),
                amount: 30.0,
                token_type: "COMPUTE".to_string(),
                from: "did:icn:requester".to_string(),
                to: did.to_string(),
                timestamp: Utc::now(),
                reference: Some("sample-job-001".to_string()),
            },
        ]
    } else {
        vec![
            TokenTransfer {
                id: "tx-12345".to_string(),
                amount: 30.0,
                token_type: "COMPUTE".to_string(),
                from: did.to_string(),
                to: "did:icn:worker01".to_string(),
                timestamp: Utc::now(),
                reference: Some("sample-job-001".to_string()),
            },
        ]
    };
    
    // Calculate balances
    let mut received = 0.0;
    let mut sent = 0.0;
    
    for transfer in &transfers {
        if transfer.to == did {
            received += transfer.amount;
        }
        if transfer.from == did {
            sent += transfer.amount;
        }
    }
    
    let net_balance = received - sent;
    
    // Print the balance
    println!("\nToken balance:");
    println!("  Received: {:.1} COMPUTE", received);
    println!("  Sent: {:.1} COMPUTE", sent);
    println!("  Net balance: {:.1} COMPUTE", net_balance);
    
    // Print recent transfers
    println!("\nRecent transfers:");
    
    for transfer in transfers {
        if transfer.to == did {
            println!("  [{}] RECEIVED {:.1} {}",
                     transfer.timestamp.format("%Y-%m-%dT%H:%M:%SZ"),
                     transfer.amount,
                     transfer.token_type);
        } else if transfer.from == did {
            println!("  [{}] SENT {:.1} {}",
                     transfer.timestamp.format("%Y-%m-%dT%H:%M:%SZ"), 
                     transfer.amount,
                     transfer.token_type);
        }
    }
    
    Ok(())
} 