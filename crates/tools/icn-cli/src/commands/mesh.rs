use clap::{Args, Subcommand, Parser, ValueHint};
use crate::{CliContext, error::{CliError, CliResult}};
use planetary_mesh::{JobManifest, NodeCapability, Bid, ResourceType, JobStatus};
use std::path::PathBuf;
use serde_json;
use icn_core_types::Did;
use cid;
use chrono::Utc;
use anyhow::Result;
use icn_core_types::{Cid, PeerId};

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
}

#[derive(Args, Debug, Clone)]
pub struct SubmitJobArgs {
    /// Path to a job manifest file (JSON format). If provided, inline arguments are ignored.
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub manifest_path: Option<PathBuf>,

    // Inline arguments (used if manifest_path is not provided)
    /// CID of the Wasm module for the job.
    #[arg(long, required_unless_present = "manifest_path")]
    pub wasm_cid: Option<String>,
    /// DID of the job owner.
    #[arg(long, required_unless_present = "manifest_path")]
    pub owner_did: Option<String>,
    /// Resource requirements (format: "Type:Value", e.g., "CpuCores:4"). Can be specified multiple times.
    #[arg(long, value_delimiter = ',', required_unless_present = "manifest_path")]
    pub resource: Vec<String>, // Example: ["CpuCores:4", "RamMb:2048"]
    /// Job parameters as a JSON string.
    #[arg(long, default_value_if("manifest_path", None, Some("{}")))] // Default to empty JSON if inline
    pub params_json: Option<String>,
    /// Optional job ID (string). If not provided, one might be generated.
    #[arg(long)]
    pub job_id: Option<String>,
    /// Optional deadline for the job (RFC3339 format, e.g., "2024-12-31T23:59:59Z").
    #[arg(long)]
    pub deadline: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct ListNodesArgs {
    /// Minimum number of CPU cores required.
    #[arg(long)]
    pub min_cpu_cores: Option<u32>,
    /// Minimum RAM in MB required.
    #[arg(long)]
    pub min_ram_mb: Option<u64>,
    /// Specific GPU type required (e.g., "NVIDIA_A100").
    #[arg(long)]
    pub gpu_type: Option<String>,
    /// Required feature (e.g., "wasm3", "sgx"). Can be specified multiple times.
    #[arg(long)]
    pub feature: Vec<String>,
    /// Maximum number of nodes to list.
    #[arg(long, default_value = "10")]
    pub limit: usize,
}

#[derive(Args, Debug, Clone)]
pub struct JobStatusArgs {
    /// The CID or ID of the job manifest.
    #[arg(long)]
    pub job_cid_or_id: String,
}

#[derive(Args, Debug, Clone)]
pub struct GetBidsArgs {
    /// The CID or ID of the job manifest to get bids for.
    #[arg(long)]
    pub job_cid_or_id: String,
}

#[derive(Args, Debug, Clone)]
pub struct AdvertiseCapabilityArgs {
    /// DID of the node advertising its capability. If not provided, will attempt to use default key.
    #[arg(long)]
    pub node_id: Option<String>,
    /// Number of CPU cores available.
    #[arg(long)]
    pub cpu_cores: Option<u32>,
    /// RAM in MB available.
    #[arg(long)]
    pub ram_mb: Option<u64>,
    /// Storage in GB available.
    #[arg(long)]
    pub storage_gb: Option<u64>,
    /// GPU type available (e.g., "NVIDIA_RTX_3090").
    #[arg(long)]
    pub gpu_type: Option<String>,
    /// Network bandwidth in Mbps available.
    #[arg(long)]
    pub network_mbps: Option<u32>,
    /// Supported feature (e.g., "sgx", "wasm3"). Can be specified multiple times.
    #[arg(long)]
    pub feature: Vec<String>,
    /// Geographic location of the node (e.g., "us-east-1").
    #[arg(long)]
    pub location: Option<String>,
    // TODO: Add options for key_file if node_id is not self-identifying from context
}

#[derive(Args, Debug, Clone)]
pub struct SubmitBidArgs {
    /// CID or ID of the job to bid on.
    #[arg(long)]
    pub job_cid_or_id: String,
    /// DID of the bidding node. If not provided, will attempt to use default key.
    #[arg(long)]
    pub node_id: Option<String>,
    /// Price for the bid (in some token unit).
    #[arg(long)]
    pub price: u64,
    /// Optional estimated time to complete in seconds.
    #[arg(long)]
    pub estimated_seconds: Option<u32>,
    /// Optional comment for the bid.
    #[arg(long)]
    pub comment: Option<String>,
    // TODO: Add options for key_file if node_id is not self-identifying from context
    // TODO: Consider how offered_capabilities are determined. From node's advertised caps or specified here?
}

pub async fn handle_mesh_command(context: &mut CliContext, cmd: &MeshCommands) -> CliResult {
    if context.verbose { println!("Handling Mesh command: {:?}", cmd); }
    match cmd {
        MeshCommands::SubmitJob(args) => handle_submit_job(context, args).await,
        MeshCommands::ListNodes(args) => handle_list_nodes(context, args).await,
        MeshCommands::JobStatus(args) => handle_job_status(context, args).await,
        MeshCommands::GetBids(args) => handle_get_bids(context, args).await,
        MeshCommands::AdvertiseCapability(args) => handle_advertise_capability(context, args).await,
        MeshCommands::SubmitBid(args) => handle_submit_bid(context, args).await,
    }
}

// Placeholder handlers
async fn handle_submit_job(_context: &mut CliContext, args: &SubmitJobArgs) -> CliResult {
    println!("Executing mesh submit-job with args: {:?}", args);
    if let Some(path) = &args.manifest_path {
        println!("  Loading from manifest file: {}", path.display());
        // TODO: Read file, deserialize JobManifest, submit to mesh scheduler
    } else {
        println!("  Creating manifest from inline arguments.");
        // TODO: Parse resources, deadline, construct JobManifest, submit to mesh scheduler
        // Ensure wasm_cid, owner_did, resource are present due to `required_unless_present`
    }
    Err(CliError::Unimplemented("mesh submit-job".to_string()))
}

async fn handle_list_nodes(_context: &mut CliContext, args: &ListNodesArgs) -> CliResult {
    println!("Executing mesh list-nodes with args: {:?}", args);
    // TODO: Fetch live peer info, apply filters, display NodeCapability list
    // Mocked example from before:
    let nodes: Vec<NodeCapability> = vec![
        NodeCapability {
            node_id: Did::from_string("did:icn:node:alpha").unwrap(),
            available_resources: vec![ResourceType::CpuCores(4), ResourceType::RamMb(8192)],
            supported_features: vec!["wasm3".to_string()],
        }
    ];
    println!("Mocked nodes: {:?}", nodes);
    Err(CliError::Unimplemented("mesh list-nodes".to_string()))
}

async fn handle_job_status(_context: &mut CliContext, args: &JobStatusArgs) -> CliResult {
    println!("Executing mesh job-status for job: {}", args.job_cid_or_id);
    // TODO: Parse job_cid_or_id, query actual JobStatus from scheduler/store
    let mock_status = JobStatus::Pending;
    println!("Mocked status: {:?}", mock_status);
    Err(CliError::Unimplemented("mesh job-status".to_string()))
}

async fn handle_get_bids(_context: &mut CliContext, args: &GetBidsArgs) -> CliResult {
    println!("Executing mesh get-bids for job: {}", args.job_cid_or_id);
    // TODO: Parse job_cid_or_id, query actual Bids from scheduler/store
    let mock_bids: Vec<Bid> = vec![];
    println!("Mocked bids: {:?}", mock_bids);
    Err(CliError::Unimplemented("mesh get-bids".to_string()))
}

async fn handle_advertise_capability(_context: &mut CliContext, args: &AdvertiseCapabilityArgs) -> CliResult {
    println!("Executing mesh advertise-capability with args: {:?}", args);
    // TODO: Construct NodeCapability from args, propagate to mesh network (libp2p / DAG anchor)
    Err(CliError::Unimplemented("mesh advertise-capability".to_string()))
}

async fn handle_submit_bid(_context: &mut CliContext, args: &SubmitBidArgs) -> CliResult {
    println!("Executing mesh submit-bid for job {} with args: {:?}", args.job_cid_or_id, args);
    // TODO: Construct Bid from args, validate against node capability, propagate to mesh network
    Err(CliError::Unimplemented("mesh submit-bid".to_string()))
} 