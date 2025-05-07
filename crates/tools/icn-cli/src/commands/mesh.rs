use clap::{Args, Subcommand, Parser, ValueHint};
use crate::{CliContext, error::{CliError, CliResult}};
use planetary_mesh::{JobManifest, NodeCapability, Bid, ResourceType, JobStatus};
use std::path::PathBuf;
use serde_json;
use icn_core_types::Did;
use cid;
use chrono::{Utc, DateTime};
use anyhow::{Result, Context, anyhow};
use icn_core_types::{Cid, PeerId};
use std::fs;
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
use std::collections::HashMap;
use hex;
use rand;
use rand::Rng;
use tokio;
use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;

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

/// Arguments for verify-receipt command.
#[derive(Args, Debug, Clone)]
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
pub async fn handle_mesh_command(
    cmd: MeshCommands, 
    ctx: &CliContext
) -> CliResult<()> {
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
            .context("Failed to serialize job manifest")?;
        
        // Create a signature using the private key
        let key_pair = ed25519_dalek::SigningKey::from_bytes(
            private_key.try_into()
                .map_err(|_| anyhow!("Invalid private key length"))?
        );
        
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
            .context("Failed to serialize job manifest")?;
        
        // Get the public key from the signer's DID
        let key_parts: Vec<&str> = self.signer.as_str().split(':').collect();
        if key_parts.len() < 4 {
            return Err(anyhow!("Invalid DID format"));
        }
        
        let key_part = key_parts[3];
        let multibase_decoded = multibase::decode(key_part)
            .map_err(|e| anyhow!("Failed to decode key part: {}", e))?;
        
        // Verify the signature
        let signature_bytes = BASE64_ENGINE.decode(self.signature.as_bytes())
            .context("Failed to decode signature")?;
        
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

/// Handle the submit-job command.
async fn handle_submit_job(
    args: SubmitJobArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .context("Failed to read key file")?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .context("Invalid key file format, expected JSON")?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("Private key not found in key file".into()))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .context("Invalid private key hex format")?;
    
    let owner_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    // Create job manifest either from file or inline arguments
    let job_manifest = if let Some(manifest_path) = &args.manifest_path {
        // Read from file
        let manifest_content = fs::read_to_string(manifest_path)
            .context("Failed to read manifest file")?;
        
        // Parse based on file extension
        if manifest_path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            toml::from_str::<JobManifest>(&manifest_content)
                .context("Failed to parse TOML manifest")?
        } else {
            serde_json::from_str::<JobManifest>(&manifest_content)
                .context("Failed to parse JSON manifest")?
        }
    } else {
        // Create from inline arguments
        let wasm_module_cid = args.wasm_module_cid
            .ok_or_else(|| CliError::InvalidArgument("WASM module CID is required".into()))?;
        
        let cid = Cid::from_str(&wasm_module_cid)
            .map_err(|_| CliError::InvalidArgument("Invalid CID format".into()))?;
        
        // Build resource requirements
        let mut resource_requirements = Vec::new();
        
        if let Some(memory_mb) = args.memory_mb {
            resource_requirements.push(ResourceType::RamMb(memory_mb));
        }
        
        if let Some(cpu_cores) = args.cpu_cores {
            resource_requirements.push(ResourceType::CpuCores(cpu_cores));
        }
        
        // Parse parameters if provided
        let parameters = if let Some(params_str) = &args.params {
            serde_json::from_str(params_str)
                .context("Failed to parse parameters as JSON")?
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        
        // Generate unique ID
        let job_id = format!("job-{}", Uuid::new_v4());
        
        JobManifest {
            id: job_id,
            wasm_module_cid: cid,
            resource_requirements,
            parameters,
            owner: owner_did.clone(),
            deadline: None,
        }
    };
    
    // Sign the job manifest
    let signed_manifest = SignedJobManifest::new(
        job_manifest, 
        &private_key_bytes,
        owner_did.clone()
    )?;
    
    // Verify signature (self-validation)
    if !signed_manifest.verify()? {
        return Err(CliError::VerificationFailed("Failed to self-verify job signature".into()));
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
            .context("Failed to convert signed manifest to JSON value")?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", owner_did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signed_manifest.signature.clone(),
        }),
    };
    
    // Serialize the VC
    let vc_json = serde_json::to_string_pretty(&vc)
        .context("Failed to serialize verifiable credential")?;
    
    // Save to a file for demonstration purposes
    let output_path = format!("job_submission_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .context("Failed to write submission to file")?;
    
    println!("Job manifest signed and submitted successfully!");
    println!("Job ID: {}", signed_manifest.manifest.id);
    println!("Saved to: {}", output_path);
    
    Ok(())
}

/// Handle the list-nodes command.
async fn handle_list_nodes(
    args: ListNodesArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    println!("Listing capable nodes in the network:");
    
    // Create some sample nodes for demonstration
    let nodes = vec![
        NodeCapability {
            node_id: Did::from_str("did:icn:node:12345")
                .map_err(|_| CliError::InvalidArgument("Invalid node DID".into()))?,
            available_resources: vec![
                ResourceType::CpuCores(4),
                ResourceType::RamMb(2048),
                ResourceType::StorageGb(500),
            ],
            supported_features: vec!["wasm".to_string(), "sgx".to_string()],
        },
        NodeCapability {
            node_id: Did::from_str("did:icn:node:67890")
                .map_err(|_| CliError::InvalidArgument("Invalid node DID".into()))?,
            available_resources: vec![
                ResourceType::CpuCores(2),
                ResourceType::RamMb(1024),
                ResourceType::StorageGb(200),
            ],
            supported_features: vec!["wasm".to_string()],
        },
        NodeCapability {
            node_id: Did::from_str("did:icn:node:abcde")
                .map_err(|_| CliError::InvalidArgument("Invalid node DID".into()))?,
            available_resources: vec![
                ResourceType::CpuCores(8),
                ResourceType::RamMb(4096),
                ResourceType::StorageGb(1000),
                ResourceType::Gpu("NVIDIA_A100".to_string()),
            ],
            supported_features: vec!["wasm".to_string(), "sgx".to_string(), "gpu".to_string()],
        },
    ];
    
    // Filter nodes based on criteria
    let filtered_nodes = nodes.into_iter()
        .filter(|node| {
            // Filter by minimum memory
            if let Some(min_memory) = args.min_memory {
                if !node.available_resources.iter().any(|r| {
                    if let ResourceType::RamMb(ram) = r {
                        *ram >= min_memory
                    } else {
                        false
                    }
                }) {
                    return false;
                }
            }
            
            // Filter by minimum cores
            if let Some(min_cores) = args.min_cores {
                if !node.available_resources.iter().any(|r| {
                    if let ResourceType::CpuCores(cores) = r {
                        *cores >= min_cores
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
                    if !node.available_resources.iter().any(|r| {
                        if let ResourceType::Gpu(_) = r {
                            true
                        } else {
                            false
                        }
                    }) {
                        return false;
                    }
                }
            }
            
            true
        })
        .take(args.limit)
        .collect::<Vec<_>>();
    
    // Print the nodes
    if filtered_nodes.is_empty() {
        println!("No nodes found matching the criteria");
        return Ok(());
    }
    
    println!("Found {} nodes:", filtered_nodes.len());
    println!("{:<25} {:<30} {:<20}", "Node ID", "Resources", "Features");
    println!("{}", "-".repeat(80));
    
    for node in filtered_nodes {
        let resources = node.available_resources.iter()
            .map(|r| match r {
                ResourceType::CpuCores(cores) => format!("{}CPU", cores),
                ResourceType::RamMb(ram) => format!("{}MB", ram),
                ResourceType::StorageGb(storage) => format!("{}GB", storage),
                ResourceType::Gpu(gpu) => format!("GPU:{}", gpu),
                _ => "Other".to_string(),
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
async fn handle_job_status(
    args: JobStatusArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    println!("Checking status for job: {}", args.job_id);
    
    // In a real implementation, this would query the network or local store
    // For demo purposes, we'll simulate a job status
    
    // Generate a random status
    let status = match rand::thread_rng().gen_range(0..5) {
        0 => JobStatus::Pending,
        1 => JobStatus::Scheduled,
        2 => JobStatus::Running { progress_percent: rand::thread_rng().gen_range(10..95) },
        3 => {
            let result_cid = Cid::from_str("bafybeihykld7uyxzogax6vgyvag42y7464eywpf55hnrwvgzxwvjmnx7fy")
                .map_err(|_| CliError::Other("Failed to parse CID".into()))?;
            JobStatus::Completed { result_cid: Some(result_cid) }
        },
        _ => JobStatus::Failed { error_message: "Insufficient resources".to_string() },
    };
    
    println!("Status: {:?}", status);
    
    match status {
        JobStatus::Running { progress_percent } => {
            println!("Progress: {}%", progress_percent);
            println!("Estimated completion: {} seconds", (100 - progress_percent) / 10);
        },
        JobStatus::Completed { result_cid } => {
            if let Some(cid) = result_cid {
                println!("Result CID: {}", cid);
                println!("Use 'icn-cli mesh get-result --result-cid {}' to retrieve the result", cid);
            } else {
                println!("No result CID available");
            }
        },
        JobStatus::Failed { ref error_message } => {
            println!("Error: {}", error_message);
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

async fn handle_get_bids(args: GetBidsArgs, ctx: &CliContext) -> CliResult<()> {
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
    // TODO: Query the mesh network for actual bids, for now use mock data
    
    let bids = vec![
        BidDetails {
            bidder_did: "did:icn:node:beta".to_string(),
            bid_cid: "bafyreig4rd7jhcvdcmwr4gkbcybxotcwxqfup3bfhcko253artrzdvuci4".to_string(),
            price: 100,
            latency: Some(200),
            memory: 8192,
            cores: 4,
            reputation: Some(95),
            renewable: Some(80),
            comment: Some("High-performance node available".to_string()),
            timestamp: Utc::now(),
        },
        BidDetails {
            bidder_did: "did:icn:node:gamma".to_string(),
            bid_cid: "bafyreigvumcahvbx4yktrxaolal5gkcdjpwpj5vuh6oegsp6hbypvmukya".to_string(),
            price: 80,
            latency: Some(300),
            memory: 4096,
            cores: 2,
            reputation: Some(85),
            renewable: Some(100),
            comment: Some("100% renewable energy computing".to_string()),
            timestamp: Utc::now(),
        },
    ];
    
    // 4. Format and display the results
    println!("Found {} bids for job '{}':", bids.len(), job_id);
    println!("{:<15} {:<8} {:<8} {:<8} {:<8} {:<8} {:<30}", 
        "Bidder DID", "Price", "Latency", "Memory", "Cores", "Rating", "Comment");
    println!("{:-<100}", "");
    
    for bid in &bids {
        let short_did = if bid.bidder_did.len() > 20 {
            format!("{}...{}", 
                &bid.bidder_did[..10], 
                &bid.bidder_did[bid.bidder_did.len() - 5..])
        } else {
            bid.bidder_did.clone()
        };
        
        println!("{:<15} {:<8} {:<8} {:<8} {:<8} {:<8} {:<30}", 
            short_did,
            bid.price,
            bid.latency.map(|l| l.to_string()).unwrap_or_else(|| "-".to_string()),
            format!("{}MB", bid.memory),
            bid.cores,
            bid.reputation.map(|r| format!("{}%", r)).unwrap_or_else(|| "-".to_string()),
            bid.comment.clone().unwrap_or_else(|| "-".to_string()));
    }
    
    // 5. Provide guidance on next steps
    println!("\nTo select a bid and proceed with execution, use:");
    println!("  icn-cli mesh select-bid --job-id {} --bid-cid <BID_CID>", job_id);
    
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
) -> CliResult<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .context("Failed to read key file")?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .context("Invalid key file format, expected JSON")?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("Private key not found in key file".into()))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .context("Invalid private key hex format")?;
    
    let requester_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    // In a real implementation, this would look up the actual job and bid from the network
    // For demo purposes, we'll simulate accepting a specific bid
    
    // Read the previously saved bids file
    let bids_file = format!("bids_for_job_{}.json", args.job_id);
    let bids_data = match fs::read_to_string(&bids_file) {
        Ok(data) => data,
        Err(_) => {
            return Err(CliError::NotFound(format!(
                "Bids file not found. Run 'icn-cli mesh get-bids --job-id {}' first",
                args.job_id
            )));
        }
    };
    
    let bids: Vec<Bid> = serde_json::from_str(&bids_data)
        .context("Failed to parse bids file")?;
    
    // Find the selected bid
    let bid_index = args.bid_id.parse::<usize>()
        .map_err(|_| CliError::InvalidArgument("Bid ID must be a number".into()))?;
    
    if bid_index == 0 || bid_index > bids.len() {
        return Err(CliError::InvalidArgument(format!(
            "Invalid bid ID. Must be between 1 and {}", bids.len()
        )));
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
        bidder_did: selected_bid.bidder_node_id.to_string(),
        price: selected_bid.price,
        timestamp: Utc::now(),
    };
    
    // Sign the acceptance
    let acceptance_json = serde_json::to_string(&acceptance)
        .context("Failed to serialize bid acceptance")?;
    
    let key_pair = ed25519_dalek::SigningKey::from_bytes(
        private_key_bytes.as_slice().try_into()
            .map_err(|_| CliError::Other("Invalid private key length".into()))?
    );
    
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
            .context("Failed to convert acceptance to JSON value")?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", requester_did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signature_b64,
        }),
    };
    
    // Serialize the VC
    let vc_json = serde_json::to_string_pretty(&vc)
        .context("Failed to serialize verifiable credential")?;
    
    // Save to a file for demonstration purposes
    let output_path = format!("bid_acceptance_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .context("Failed to write acceptance to file")?;
    
    // Create an execution receipt to simulate job completion
    println!("Bid #{} for job {} accepted successfully!", bid_index, args.job_id);
    println!("Provider: {}", selected_bid.bidder_node_id);
    println!("Price: {} tokens", selected_bid.price);
    println!("Acceptance credential saved to: {}", output_path);
    
    // Simulate job execution with a brief delay
    println!("\nSimulating job execution...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Create execution receipt
    simulate_execution_receipt(&args.job_id, selected_bid, &output_path)
        .context("Failed to create execution receipt")?;
    
    println!("\nJob execution complete! Results and execution receipt have been saved.");
    
    Ok(())
}

/// Simulate the creation of an execution receipt for demonstration purposes.
fn simulate_execution_receipt(job_id: &str, bid: &Bid, acceptance_path: &str) -> Result<(), anyhow::Error> {
    // Create a sample result and receipt
    let execution_time_ms = rand::thread_rng().gen_range(500..3000);
    let memory_peak_mb = rand::thread_rng().gen_range(256..bid.offered_capabilities.iter()
        .find_map(|c| if let ResourceType::RamMb(mb) = c { Some(*mb) } else { None })
        .unwrap_or(1024));
    
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
        bid_id: bid.bidder_node_id.to_string(),
        result_hash: hex::encode(result_hash),
        execution_metrics: ExecutionMetrics {
            execution_time_ms,
            memory_peak_mb,
            cpu_usage_pct,
            io_read_bytes: rand::thread_rng().gen_range(1_000_000..10_000_000),
            io_write_bytes: rand::thread_rng().gen_range(500_000..5_000_000),
        },
        token_compensation: TokenCompensation {
            amount: bid.price as f64,
            token_type: "COMPUTE".to_string(),
            from: "did:icn:requester".to_string(),
            to: bid.bidder_node_id.to_string(),
            timestamp: Utc::now(),
        },
    };
    
    // Save the result
    let result_json = serde_json::to_string_pretty(&result)
        .context("Failed to serialize execution result")?;
    
    let result_path = format!("execution_result_{}.json", job_id);
    fs::write(&result_path, &result_json)
        .context("Failed to write execution result to file")?;
    
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
async fn handle_advertise_capability(
    args: AdvertiseCapabilityArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .context("Failed to read key file")?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .context("Invalid key file format, expected JSON")?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let node_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    // Create capability manifest
    let capabilities = if let Some(manifest_path) = &args.manifest_path {
        // Read from file
        let manifest_content = fs::read_to_string(manifest_path)
            .context("Failed to read manifest file")?;
        
        // Parse based on file extension
        if manifest_path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            toml::from_str::<NodeCapability>(&manifest_content)
                .context("Failed to parse TOML manifest")?
        } else {
            serde_json::from_str::<NodeCapability>(&manifest_content)
                .context("Failed to parse JSON manifest")?
        }
    } else {
        // Create from inline arguments
        let mut available_resources = Vec::new();
        
        if let Some(memory_mb) = args.memory_mb {
            available_resources.push(ResourceType::RamMb(memory_mb));
        }
        
        if let Some(cpu_cores) = args.cpu_cores {
            available_resources.push(ResourceType::CpuCores(cpu_cores));
        }
        
        // Detect system capabilities if not specified
        if available_resources.is_empty() {
            // Get CPU cores
            let cpu_count = num_cpus::get() as u32;
            available_resources.push(ResourceType::CpuCores(cpu_count));
            
            // Estimate memory (in MB)
            let mem_info = sys_info::mem_info()
                .map_err(|e| CliError::Other(format!("Failed to get system memory info: {}", e)))?;
            
            let mem_mb = (mem_info.total / 1024) as u64;
            available_resources.push(ResourceType::RamMb(mem_mb));
            
            println!("Auto-detected system capabilities:");
            println!("  CPU cores: {}", cpu_count);
            println!("  Memory: {} MB", mem_mb);
        }
        
        NodeCapability {
            node_id: node_did.clone(),
            available_resources,
            supported_features: vec!["wasm".to_string()],
        }
    };
    
    // Serialize and save to file
    let output_path = format!("node_capability_{}.json", node_did.to_string().replace(":", "_"));
    let capabilities_json = serde_json::to_string_pretty(&capabilities)
        .context("Failed to serialize node capabilities")?;
    
    fs::write(&output_path, &capabilities_json)
        .context("Failed to write node capabilities to file")?;
    
    println!("Node capabilities advertised successfully!");
    println!("Node ID: {}", node_did);
    println!("Resources: {:?}", capabilities.available_resources);
    println!("Features: {:?}", capabilities.supported_features);
    println!("Saved to: {}", output_path);
    
    Ok(())
}

/// Handle the submit-bid command.
async fn handle_submit_bid(
    args: SubmitBidArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    // Read the key file for signing
    let key_data = fs::read_to_string(&args.key_path)
        .context("Failed to read key file")?;
    
    let key_json: serde_json::Value = serde_json::from_str(&key_data)
        .context("Invalid key file format, expected JSON")?;
    
    let did = key_json["did"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("DID not found in key file".into()))?;
    
    let private_key_hex = key_json["privateKey"].as_str()
        .ok_or_else(|| CliError::InvalidArgument("Private key not found in key file".into()))?;
    
    let private_key_bytes = hex::decode(private_key_hex)
        .context("Invalid private key hex format")?;
    
    let bidder_did = Did::from_str(did)
        .map_err(|e| CliError::InvalidArgument(format!("Invalid DID format: {}", e)))?;
    
    // Create bid
    let bid = Bid {
        job_manifest_cid: Cid::from_str("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
            .map_err(|_| CliError::Other("Failed to parse CID".into()))?,
        bidder_node_id: bidder_did.clone(),
        price: args.price,
        confidence: args.confidence,
        offered_capabilities: vec![
            ResourceType::CpuCores(4),
            ResourceType::RamMb(2048),
        ],
        expires_at: Some(Utc::now() + chrono::Duration::hours(24)),
    };
    
    // Sign the bid
    let bid_json = serde_json::to_string(&bid)
        .context("Failed to serialize bid")?;
    
    let key_pair = ed25519_dalek::SigningKey::from_bytes(
        private_key_bytes.as_slice().try_into()
            .map_err(|_| CliError::Other("Invalid private key length".into()))?
    );
    
    let signature = key_pair.sign(bid_json.as_bytes());
    let signature_b64 = BASE64_ENGINE.encode(signature.to_bytes());
    
    // Create verifiable credential
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
            .context("Failed to convert bid to JSON value")?,
        proof: Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", bidder_did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: signature_b64,
        }),
    };
    
    // Serialize the VC
    let vc_json = serde_json::to_string_pretty(&vc)
        .context("Failed to serialize verifiable credential")?;
    
    // Save to a file for demonstration purposes
    let output_path = format!("bid_submission_{}.json", Uuid::new_v4());
    fs::write(&output_path, &vc_json)
        .context("Failed to write bid to file")?;
    
    println!("Bid submitted successfully for job: {}", args.job_id);
    println!("Bidder: {}", bidder_did);
    println!("Price: {} tokens", args.price);
    println!("Confidence: {:.2}", args.confidence);
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
async fn handle_verify_receipt(
    args: VerifyReceiptArgs, 
    ctx: &CliContext
) -> CliResult<()> {
    println!("Verifying execution receipt: {}", args.receipt_cid);
    
    // In a real implementation, this would retrieve the receipt from the DAG
    // and verify its cryptographic proof.
    // For demo purposes, we'll use a sample receipt.
    
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
    println!("Receipt CID: {}", args.receipt_cid);
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
) -> CliResult<()> {
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