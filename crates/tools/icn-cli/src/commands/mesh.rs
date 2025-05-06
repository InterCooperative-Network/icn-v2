use clap::Subcommand;
use crate::{CliContext, error::{CliError, CliResult}};
use icn_types::mesh::{JobManifest, NodeCapability, Bid, ResourceType, JobStatus};
use std::path::PathBuf;
use serde_json;
use icn_core_types::Did;
use cid;
use chrono::Utc;

/// Commands for interacting with the ICN Mesh
#[derive(Subcommand, Debug, Clone)]
pub enum MeshCommands {
    /// Submit a new job to the mesh from a manifest file.
    SubmitJob {
        #[arg(short, long, value_name = "FILE_PATH")]
        manifest_path: PathBuf,
    },
    /// List known capable nodes for a given requirement
    ListNodes {
        // Example: filtering by capability, region etc.
        // This needs more concrete definition based on NodeCapability structure
        // For now, let's assume listing all.
    },
    /// Get the status of a submitted job
    JobStatus {
        /// The CID of the job manifest
        job_cid: String, // Assuming CID is represented as String for CLI
    },
    /// Get bids for a specific job
    GetBids {
         /// The CID of the job manifest
         job_cid: String,
    },
}

pub async fn handle_mesh_command(context: &mut CliContext, cmd: &MeshCommands) -> CliResult {
    match cmd {
        MeshCommands::SubmitJob { manifest_path } => {
            use std::fs;
            // PathBuf is already in scope from struct definition
            // use serde_json; // serde_json::from_str will be used

            // Step 1: Read manifest file
            let manifest_data = fs::read_to_string(&manifest_path)
                .map_err(CliError::Io)?; // Use existing Io variant

            // Step 2: Deserialize to JobManifest
            let manifest: JobManifest = serde_json::from_str(&manifest_data)
                .map_err(CliError::Json)?; // Use existing Json variant

            // Step 3: Log parsed manifest
            println!("Loaded Job Manifest: {:?}", manifest);

            // Step 4: Placeholder for mesh interaction
            println!("TODO: Submit this job to the mesh scheduler via libp2p or AgoraNet.");

            Ok(())
        }
        MeshCommands::ListNodes { /* filter params */ } => {
            // TODO: In the future, fetch live peer info from a running mesh daemon or p2p discovery module.
            // For now, using mocked list of node capabilities.
            // Also, the `context` parameter is unused for now in this arm.
            let _ = context; // Mark context as used to avoid warnings

            let nodes: Vec<NodeCapability> = vec![
                NodeCapability {
                    node_id: Did::from_string("did:icn:node:alpha").expect("Failed to parse DID for mock node alpha"),
                    available_resources: vec![
                        ResourceType::CpuCores(4),
                        ResourceType::RamMb(8 * 1024), // 8 GB
                        ResourceType::StorageGb(256),
                        ResourceType::Gpu("NVIDIA_RTX_3080".to_string()),
                    ],
                    supported_features: vec!["test".to_string(), "fast-processing".to_string(), "sgx".to_string()],
                },
                NodeCapability {
                    node_id: Did::from_string("did:icn:node:beta").expect("Failed to parse DID for mock node beta"),
                    available_resources: vec![
                        ResourceType::CpuCores(2),
                        ResourceType::RamMb(4 * 1024), // 4 GB
                        ResourceType::StorageGb(128),
                        ResourceType::NetworkBandwidthMbps(1000),
                    ],
                    supported_features: vec!["edge-compute".to_string(), "low-latency".to_string()],
                },
                NodeCapability {
                    node_id: Did::from_string("did:icn:node:gamma").expect("Failed to parse DID for mock node gamma"),
                    available_resources: vec![
                        ResourceType::CpuCores(16),
                        ResourceType::RamMb(64 * 1024), // 64 GB
                        ResourceType::StorageGb(1024), // 1 TB
                        ResourceType::Gpu("AMD_Radeon_Pro_VII".to_string()),
                        ResourceType::NetworkBandwidthMbps(10000),
                    ],
                    supported_features: vec!["high-performance-compute".to_string(), "gpu-enabled".to_string()],
                },
            ];

            println!("Discovered nodes (mock data):\n");
            for node in nodes {
                println!("🔹 Node ID: {}", node.node_id); // Assumes Did implements Display
                println!("   Available Resources:");
                if node.available_resources.is_empty() {
                    println!("     - None specified");
                } else {
                    for resource in &node.available_resources {
                        match resource {
                            ResourceType::CpuCores(val) => println!("     - CPU Cores: {}", val),
                            ResourceType::RamMb(val) => println!("     - RAM (MB): {}", val),
                            ResourceType::StorageGb(val) => println!("     - Storage (GB): {}", val),
                            ResourceType::Gpu(val) => println!("     - GPU: {}", val),
                            ResourceType::NetworkBandwidthMbps(val) => println!("     - Network (Mbps): {}", val),
                        }
                    }
                }
                println!("   Supported Features: {}", if node.supported_features.is_empty() { "None specified".to_string() } else { node.supported_features.join(", ") });
                println!(); // Newline for separation
            }
            Ok(())
        }
        MeshCommands::JobStatus { job_cid } => {
            // Attempt to parse the job_cid string into a Cid
            let external_cid: cid::CidGeneric<64> = job_cid.as_str().parse()
                .map_err(|e: cid::Error| CliError::InvalidCidFormat(format!("Invalid CID string '{}': {}", job_cid, e)))?;
            
            // Convert from cid::CidGeneric<64> to our Cid wrapper via bytes
            let parsed_cid = icn_core_types::Cid::from_bytes(&external_cid.to_bytes())
                .map_err(|e| CliError::InvalidCidFormat(format!("Failed to convert parsed CID to internal format: {}", e)))?;

            println!("Getting status for job: {}", parsed_cid); // Use parsed_cid for display

            // TODO: Query actual job status from scheduler/store based on parsed_cid
            // For now, return mock status. 
            let mock_status = match job_cid.as_str() { // Still use original job_cid string for mock matching for simplicity
                "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi" => JobStatus::Completed {
                    result_cid: Some(
                        icn_core_types::Cid::from_bytes(
                            &"bafybeihs7w44m7yf2dqsvfmu7kbmtrnn63wldh3pztdnffxjjmscgxkiqa".parse::<cid::CidGeneric<64>>().unwrap().to_bytes()
                        ).unwrap()
                    ),
                },
                "bafybeihwe6k7hxwfh6jbsz2pmyq5lhj2wvpsn5qjaddfobwhnlig4oakyq" => JobStatus::Running { progress_percent: 75 },
                "bafybeifg6ljzdrg55yq2nhqnqbyfkhlvnq4kgguoca3m7k47kbqtzjyoia" => JobStatus::Failed { error_message: "Task exceeded memory limits".to_string() },
                _ => JobStatus::Pending, // Default mock status
            };

            println!("\nJob Status Report:");
            println!("------------------");
            println!("Job ID (CID): {}", parsed_cid);
            match mock_status {
                JobStatus::Pending => println!("Status: ⏳ Pending"),
                JobStatus::Scheduled => println!("Status: 🗓️ Scheduled"),
                JobStatus::Running { progress_percent } => println!("Status: ⚙️ Running ({}% complete)", progress_percent),
                JobStatus::Completed { result_cid } => {
                    print!("Status: ✅ Completed");
                    if let Some(cid) = result_cid {
                        println!(" (Result CID: {})", cid);
                    } else {
                        println!();
                    }
                }
                JobStatus::Failed { error_message } => println!("Status: ❌ Failed ({})", error_message),
                JobStatus::NotFound => println!("Status: ❓ Not Found"),
            }
            println!("------------------");

            // The context parameter is unused for now in this arm.
            let _ = context;

            Ok(())
        }
        MeshCommands::GetBids { job_cid } => {
            // Attempt to parse the job_cid string into a Cid
            let external_cid_parsed: cid::CidGeneric<64> = job_cid.as_str().parse()
                .map_err(|e: cid::Error| CliError::InvalidCidFormat(format!("Invalid Job CID string '{}': {}", job_cid, e)))?;
            
            let parsed_job_cid = icn_core_types::Cid::from_bytes(&external_cid_parsed.to_bytes())
                .map_err(|e| CliError::InvalidCidFormat(format!("Failed to convert parsed Job CID to internal format: {}", e)))?;

            println!("Getting bids for job: {}\n", parsed_job_cid);

            // TODO: Query actual bids from scheduler/store based on parsed_job_cid
            // For now, returning a mock list of bids.

            // Mock CIDs and DIDs for bids
            let mock_job_manifest_cid_str = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
            let mock_job_manifest_cid_external = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".parse::<cid::CidGeneric<64>>().unwrap();
            let mock_job_manifest_cid = icn_core_types::Cid::from_bytes(&mock_job_manifest_cid_external.to_bytes()).unwrap();
            
            let bidder_alpha_did = Did::from_string("did:icn:node:bidder_alpha").expect("Mock DID alpha failed");
            let bidder_beta_did = Did::from_string("did:icn:node:bidder_beta").expect("Mock DID beta failed");

            let mock_bids: Vec<Bid> = vec![
                Bid {
                    job_manifest_cid: mock_job_manifest_cid.clone(), // Assuming the bids are for the parsed_job_cid or a known mock
                    bidder_node_id: bidder_alpha_did.clone(),
                    price: 150, // Example price units
                    confidence: 0.95,
                    offered_capabilities: vec![
                        ResourceType::CpuCores(4),
                        ResourceType::RamMb(16 * 1024),
                        ResourceType::Gpu("NVIDIA_RTX_A4000".to_string()),
                    ],
                    expires_at: Some(Utc::now() + chrono::Duration::hours(24)),
                },
                Bid {
                    job_manifest_cid: mock_job_manifest_cid.clone(),
                    bidder_node_id: bidder_beta_did.clone(),
                    price: 120,
                    confidence: 0.88,
                    offered_capabilities: vec![
                        ResourceType::CpuCores(2),
                        ResourceType::RamMb(8 * 1024),
                        ResourceType::StorageGb(500),
                    ],
                    expires_at: Some(Utc::now() + chrono::Duration::hours(48)),
                },
                // Add a bid for a different job CID to show filtering (if we were actually filtering)
                // For now, all mock bids will be for the same mock_job_manifest_cid
            ];
            
            // Simulate filtering bids for the requested job_cid (even though all mocks are for one job here)
            let bids_for_job: Vec<&Bid> = mock_bids.iter()
                                            .filter(|b| b.job_manifest_cid == parsed_job_cid || b.job_manifest_cid == mock_job_manifest_cid) // Simple mock filter
                                            .collect();

            if bids_for_job.is_empty() {
                println!("No bids found for job ID: {}", parsed_job_cid);
                if parsed_job_cid.to_string() != mock_job_manifest_cid_str {
                    println!("(Note: Mock data currently only has bids for job ID {})", mock_job_manifest_cid_str);
                }
            } else {
                println!("Bids Found:");
                println!("-----------");
                for (index, bid) in bids_for_job.iter().enumerate() {
                    println!("Bid #{}", index + 1);
                    println!("  Bidder Node ID:   {}", bid.bidder_node_id);
                    println!("  Price:            {}", bid.price);
                    println!("  Confidence:       {:.2}", bid.confidence);
                    println!("  Job Manifest CID: {}", bid.job_manifest_cid); // Usually same as parsed_job_cid
                    println!("  Offered Resources:");
                    if bid.offered_capabilities.is_empty() {
                        println!("    - None specified");
                    } else {
                        for resource in &bid.offered_capabilities {
                            match resource {
                                ResourceType::CpuCores(val) => println!("    - CPU Cores: {}", val),
                                ResourceType::RamMb(val) => println!("    - RAM (MB): {}", val),
                                ResourceType::StorageGb(val) => println!("    - Storage (GB): {}", val),
                                ResourceType::Gpu(val) => println!("    - GPU: {}", val),
                                ResourceType::NetworkBandwidthMbps(val) => println!("    - Network (Mbps): {}", val),
                            }
                        }
                    }
                    if let Some(expires) = bid.expires_at {
                        println!("  Expires At:       {}", expires.to_rfc3339());
                    }
                    println!("-----------");
                }
            }
            
            // The context parameter is unused for now in this arm.
            let _ = context;
            Ok(())
        }
    }
    // Ok(())
} 