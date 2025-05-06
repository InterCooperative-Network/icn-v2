//! Placeholder for icn-cli binary

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use icn_identity_core::did::DidKey;
use icn_types::dag::{memory::MemoryDagStore, DagError, DagStore, SignedDagNode};
use icn_types::{anchor::AnchorRef, Did, ExecutionReceipt, ExecutionResult, TrustBundle};
use icn_types::bundle::TrustBundleError;
use icn_types::receipts::ReceiptError;
use icn_types::dag::sync::{
    NetworkDagSyncService, SyncPolicy, TransportConfig,
    transport::libp2p::Libp2pDagTransport,
    FederationPeer,
};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Add this at the top with other mod declarations
mod metrics;

// Add these imports
use metrics::{counter, gauge, histogram};
use std::net::SocketAddr;
use crate::metrics::{MetricsContext, init_metrics};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new DID key
    #[command(name = "key-gen")]
    KeyGen {
        /// Output file to save the key (defaults to ~/.icn/key.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// DAG commands
    #[command(subcommand)]
    Dag(DagCommands),

    /// TrustBundle commands
    #[command(subcommand)]
    Bundle(BundleCommands),

    /// ExecutionReceipt commands
    #[command(subcommand)]
    Receipt(ReceiptCommands),
    
    /// Mesh computation commands
    #[command(subcommand)]
    Mesh(MeshCommands),
}

#[derive(Subcommand)]
enum DagCommands {
    /// Submit an anchor to the DAG
    #[command(name = "submit-anchor")]
    SubmitAnchor {
        /// Path to input file containing anchor data
        #[arg(short, long)]
        input: PathBuf,

        /// Type of anchor (bundle or receipt)
        #[arg(short, long)]
        anchor_type: String,

        /// Path to key file
        #[arg(short, long)]
        key: PathBuf,

        /// Path to DAG storage directory
        #[arg(short, long)]
        dag_dir: PathBuf,
    },

    /// Replay and verify a DAG branch
    #[command(name = "replay")]
    Replay {
        /// CID of the DAG node to start replay from
        #[arg(short, long)]
        cid: String,

        /// Path to DAG storage directory
        #[arg(short, long)]
        dag_dir: PathBuf,
    },

    /// Verify a TrustBundle
    #[command(name = "verify-bundle")]
    VerifyBundle {
        /// CID of the TrustBundle to verify
        #[arg(short, long)]
        cid: String,

        /// Path to DAG storage directory
        #[arg(short, long)]
        dag_dir: PathBuf,
    },

    /// Export a thread (path between two DAG nodes)
    #[command(name = "export-thread")]
    ExportThread {
        /// CID of the first node
        #[arg(short, long)]
        from: String,

        /// CID of the second node
        #[arg(short, long)]
        to: String,

        /// Path to DAG storage directory
        #[arg(short, long)]
        dag_dir: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Synchronize with a federation peer
    #[command(name = "sync")]
    Sync {
        /// Peer endpoint URL
        #[arg(short, long)]
        peer: String,

        /// Federation ID
        #[arg(short, long)]
        federation: String,

        /// Peer ID 
        #[arg(short, long)]
        peer_id: String,

        /// Path to DAG storage directory
        #[arg(short, long)]
        dag_dir: PathBuf,
        
        /// Trust level for this peer (0-100)
        #[arg(short, long, default_value = "50")]
        trust: u8,
    },

    /// Advanced DAG sync commands with libp2p support
    #[command(subcommand)]
    SyncP2P(DagSyncCommands),

    /// Generate a visual representation of the DAG
    #[command(name = "visualize")]
    Visualize {
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Output file for the graph visualization (DOT format)
        #[arg(long)]
        output: PathBuf,
        
        /// Filter by thread DID (optional) to show only nodes from a specific author
        #[arg(long)]
        thread_did: Option<String>,
        
        /// Maximum number of nodes to include in visualization
        #[arg(long, default_value = "100")]
        max_nodes: usize,
    },
}

#[derive(Subcommand)]
enum BundleCommands {
    /// Create a new TrustBundle
    #[command(name = "create")]
    Create {
        /// State CID
        #[arg(short, long)]
        state_cid: String,

        /// Policy ID
        #[arg(short, long)]
        policy_id: String,

        /// Quorum DID (can be specified multiple times)
        #[arg(short, long)]
        quorum_did: Vec<String>,

        /// Previous anchor CIDs (optional, comma-separated)
        #[arg(short, long)]
        previous: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReceiptCommands {
    /// Create a new ExecutionReceipt
    #[command(name = "create")]
    Create {
        /// Execution CID
        #[arg(short, long)]
        execution_cid: String,

        /// Result status (success, error, deferred)
        #[arg(short, long)]
        status: String,

        /// Result data as JSON or string
        #[arg(short, long)]
        data: String,

        /// Dependencies (optional, comma-separated)
        #[arg(short, long)]
        dependencies: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum DagSyncCommands {
    /// Connect to a specific peer
    Connect {
        /// The peer multiaddress to connect to
        #[arg(long)]
        peer: String,
        
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Path to DAG storage directory 
        #[arg(long)]
        dag_dir: PathBuf,
    },
    
    /// Start auto-sync mode to discover and sync with peers
    AutoSync {
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Enable mDNS discovery
        #[arg(long, default_value = "true")]
        mdns: bool,
        
        /// Enable Kademlia DHT discovery
        #[arg(long, default_value = "false")]
        kad_dht: bool,
        
        /// Comma-separated list of bootstrap peers
        #[arg(long, value_delimiter = ',')]
        bootstrap_peers: Option<Vec<String>>,
        
        /// Comma-separated list of authorized DIDs
        #[arg(long, value_delimiter = ',')]
        authorized_dids: Option<Vec<String>>,
        
        /// Minimum number of peers required for quorum
        #[arg(long, default_value = "1")]
        min_quorum: usize,
    },
    
    /// Offer nodes to a peer
    Offer {
        /// The peer ID to offer nodes to
        #[arg(long)]
        peer: String,
        
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Maximum number of nodes to offer
        #[arg(long, default_value = "100")]
        max_nodes: usize,
    },
    
    /// Create and bootstrap a genesis DAG state for a new federation
    Genesis {
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Path to key file for genesis signing
        #[arg(long)]
        key: PathBuf,
        
        /// Policy ID for the genesis TrustBundle
        #[arg(long)]
        policy_id: String,
        
        /// Comma-separated list of founding member DIDs for the genesis quorum
        #[arg(long, value_delimiter = ',')]
        founding_dids: Vec<String>,
        
        /// Whether this node should listen for connections (true) or just create local genesis (false)
        #[arg(long, default_value = "true")]
        start_node: bool,
        
        /// Listen address for the p2p node (defaults to /ip4/0.0.0.0/tcp/9000)
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9000")]
        listen_addr: String,
        
        /// Enable mDNS discovery
        #[arg(long, default_value = "true")]
        mdns: bool,
    },
    
    /// Visualize the DAG
    Visualize {
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Output file for the graph visualization (DOT format)
        #[arg(long)]
        output: PathBuf,
        
        /// Filter by thread DID (optional) to show only nodes from a specific author
        #[arg(long)]
        thread_did: Option<String>,
        
        /// Maximum number of nodes to include in visualization
        #[arg(long, default_value = "100")]
        max_nodes: usize,
    },
}

/// Commands for managing distributed compute over the mesh network
#[derive(Subcommand)]
enum MeshCommands {
    /// Create and publish a task ticket for distributed computation
    #[command(name = "publish-task")]
    PublishTask {
        /// Path to WASM module file to execute
        #[arg(long)]
        wasm_file: PathBuf,
        
        /// Input data path or URI (can be specified multiple times)
        #[arg(long)]
        input: Vec<String>,
        
        /// Maximum latency to accept (in milliseconds)
        #[arg(long, default_value = "1000")]
        max_latency: u64,
        
        /// Required memory in MB
        #[arg(long, default_value = "512")]
        memory: u64,
        
        /// Required CPU cores
        #[arg(long, default_value = "1")]
        cores: u64,
        
        /// Task priority (1-100)
        #[arg(long, default_value = "50")]
        priority: u8,
        
        /// Federation ID
        #[arg(long)]
        federation: String,
        
        /// Path to key file for signing
        #[arg(long)]
        key: PathBuf,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
    },
    
    /// Bid on a task ticket
    #[command(name = "bid")]
    Bid {
        /// Task ticket CID
        #[arg(long)]
        task_cid: String,
        
        /// Offered latency in milliseconds
        #[arg(long)]
        latency: u64,
        
        /// Available memory in MB
        #[arg(long)]
        memory: u64,
        
        /// Available CPU cores
        #[arg(long)]
        cores: u64,
        
        /// Reputation score to include
        #[arg(long, default_value = "50")]
        reputation: u8,
        
        /// Renewable energy percentage (0-100)
        #[arg(long, default_value = "0")]
        renewable: u8,
        
        /// Path to key file for signing
        #[arg(long)]
        key: PathBuf,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
    },
    
    /// Execute a task
    #[command(name = "execute")]
    Execute {
        /// Task ticket CID
        #[arg(long)]
        task_cid: String,
        
        /// Bid CID that was accepted
        #[arg(long)]
        bid_cid: String,
        
        /// Path to key file
        #[arg(long)]
        key: PathBuf,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Output directory for execution results
        #[arg(long)]
        output_dir: PathBuf,
    },
    
    /// Verify an execution receipt
    #[command(name = "verify-receipt")]
    VerifyReceipt {
        /// Receipt CID to verify
        #[arg(long)]
        receipt_cid: String,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
    },
    
    /// Check token balances for a DID
    #[command(name = "check-balance")]
    CheckBalance {
        /// DID to check balance for (if not provided, uses the DID from key)
        #[arg(long)]
        did: Option<String>,
        
        /// Path to key file (required if DID not provided)
        #[arg(long)]
        key: Option<PathBuf>,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Federation ID
        #[arg(long)]
        federation: String,
    },
    
    /// Start a scheduler node to automatically match tasks and bids
    #[command(name = "scheduler")]
    Scheduler {
        /// Federation ID
        #[arg(long)]
        federation: String,
        
        /// Path to key file
        #[arg(long)]
        key: PathBuf,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Listen address (e.g., /ip4/0.0.0.0/tcp/9001)
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9001")]
        listen: String,
        
        /// Enable mDNS discovery
        #[arg(long, default_value = "true")]
        mdns: bool,
        
        /// Required node capabilities (can be specified multiple times as key=value)
        #[arg(long, value_parser = parse_key_val)]
        require: Vec<(String, String)>,
    },
    
    /// Start a metrics server for monitoring mesh activity
    #[command(name = "metrics-server")]
    MetricsServer {
        /// Federation ID
        #[arg(long)]
        federation: String,
        
        /// Listen address for metrics server (default: 127.0.0.1:9090)
        #[arg(long, default_value = "127.0.0.1:9090")]
        listen: String,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
    },
    
    /// Show mesh statistics
    #[command(name = "stats")]
    Stats {
        /// Federation ID
        #[arg(long)]
        federation: String,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Type of stats to show (tasks, bids, tokens, peers)
        #[arg(long, default_value = "tasks")]
        type_filter: String,
        
        /// Number of entries to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    
    /// Work with node manifests for mesh capability advertisement
    #[command(name = "manifest")]
    Manifest {
        /// Action to perform (create, publish, show, update, list)
        #[arg(long)]
        action: String,
        
        /// Path to key file for signing the manifest
        #[arg(long)]
        key: PathBuf,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Federation ID
        #[arg(long)]
        federation: String,
        
        /// Output file for created manifests
        #[arg(long)]
        output: Option<PathBuf>,
        
        /// CID of manifest to show (only for 'show' action)
        #[arg(long)]
        cid: Option<String>,
        
        /// Field to update (only for 'update' action)
        #[arg(long)]
        field: Option<String>,
        
        /// Value to set (only for 'update' action)
        #[arg(long)]
        value: Option<String>,
        
        /// Trusted firmware hash (only for 'create' and 'publish' actions)
        #[arg(long)]
        firmware_hash: Option<String>,
    },
    
    /// Verify capability-based scheduling and view audit records
    #[command(name = "audit")]
    Audit {
        /// The type of audit to perform (dispatch, manifest, requirements)
        #[arg(long, default_value = "dispatch")]
        audit_type: String,
        
        /// CID of the specific record to show (optional)
        #[arg(long)]
        cid: Option<String>,
        
        /// Show audit records for tasks dispatched by a specific scheduler DID
        #[arg(long)]
        scheduler: Option<String>,
        
        /// Show audit records for a specific task CID
        #[arg(long)]
        task: Option<String>,
        
        /// Show audit records with specific capability requirements
        #[arg(long)]
        requirement: Option<Vec<String>>,
        
        /// Maximum number of records to show
        #[arg(long, default_value = "10")]
        limit: usize,
        
        /// Path to DAG storage
        #[arg(long)]
        dag_dir: PathBuf,
        
        /// Federation ID
        #[arg(long)]
        federation: String,

        /// Enable credential verification
        #[arg(long)]
        verify: bool,
        
        /// Path to trusted DIDs policy file (TOML)
        #[arg(long)]
        trusted_dids_path: Option<PathBuf>,
        
        /// Export verification results to JSON file
        #[arg(long)]
        export_results: Option<PathBuf>,
    },
}

/// Parse a key=value pair
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.split('=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid key=value format: {}", s));
    }
    
    let key = parts[0].trim().to_string();
    let value = parts[1].trim().to_string();
    
    Ok((key, value))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::KeyGen { output } => {
            let key_path = output.clone().unwrap_or_else(|| {
                let mut path = dirs::home_dir().expect("Could not determine home directory");
                path.push(".icn");
                fs::create_dir_all(&path).expect("Failed to create directory");
                path.push("key.json");
                path
            });
            
            // Generate a new DID key
            let did_key = DidKey::new();
            
            // Get the DID as a string
            let did_str = did_key.to_did_string();
            
            // Create output data
            let json = serde_json::json!({
                "did": did_str,
                // In a real implementation, we would handle secure key storage properly
                // This is just a simplified example for the CLI
                "privateKey": "PRIVATE_KEY_WOULD_BE_HERE",
            });
            
            // Write to file
            fs::write(&key_path, serde_json::to_string_pretty(&json)?)
                .context("Failed to write key file")?;
                
            println!("Generated new DID key: {}", did_str);
            println!("Saved to: {}", key_path.display());
            
            Ok(())
        },
        
        Commands::Dag(dag_cmd) => handle_dag_command(dag_cmd).await,
        Commands::Bundle(bundle_cmd) => handle_bundle_command(bundle_cmd).await,
        Commands::Receipt(receipt_cmd) => handle_receipt_command(receipt_cmd).await,
        Commands::Mesh(mesh_cmd) => handle_mesh_command(mesh_cmd).await,
    }
}

async fn handle_dag_command(cmd: &DagCommands) -> Result<()> {
    match cmd {
        DagCommands::SubmitAnchor { input, anchor_type, key, dag_dir } => {
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let did = Did::from(did_str.to_string());
            
            // In a real implementation, we would load the private key
            // For now, we'll use a placeholder by generating a new key
            // (not the same as the one associated with the DID, just for demo purposes)
            let dummy_key = DidKey::new();
            // We use sign method rather than directly accessing the private key field
            
            // Create DAG store
            let mut dag_store = open_dag_store(dag_dir).await?;
            
            // Read input file
            let input_data = fs::read_to_string(input)
                .context("Failed to read input file")?;
            
            // Handle different anchor types
            match anchor_type.as_str() {
                "bundle" => {
                    let bundle: TrustBundle = serde_json::from_str(&input_data)
                        .context("Failed to parse input as TrustBundle")?;
                    
                    #[cfg(test)]
                    let cid = bundle.anchor_to_dag_with_key(did, &dummy_key, &mut dag_store)
                        .context("Failed to anchor TrustBundle to DAG")?;
                        
                    #[cfg(not(test))]
                    let cid = {
                        // Convert code here to work directly with signature functions
                        // Create a DAG node for this bundle
                        let node = bundle.to_dag_node(did)
                            .map_err(|e| anyhow::anyhow!("Failed to create DAG node: {}", e))?;
                        
                        // Serialize the node for signing
                        let node_bytes = serde_json::to_vec(&node)
                            .context("Failed to serialize node")?;
                        
                        // Sign the node
                        let signature = dummy_key.sign(&node_bytes);
                        
                        // Create a signed node
                        let signed_node = SignedDagNode {
                            node,
                            signature,
                            cid: None, // Will be computed when added to the DAG
                        };
                        
                        // Add to the DAG store
                        dag_store.add_node(signed_node)
                            .await
                            .map_err(|e| anyhow::anyhow!("Failed to add node to DAG: {}", e))?
                    };
                    
                    println!("TrustBundle anchored to DAG with CID: {}", cid);
                },
                "receipt" => {
                    let receipt: ExecutionReceipt = serde_json::from_str(&input_data)
                        .context("Failed to parse input as ExecutionReceipt")?;
                    
                    #[cfg(test)]
                    let cid = receipt.anchor_to_dag_with_key(&dummy_key, &mut dag_store)
                        .context("Failed to anchor ExecutionReceipt to DAG")?;
                        
                    #[cfg(not(test))]
                    let cid = {
                        // Convert code here to work directly with signature functions
                        // Create a DAG node for this receipt
                        let node = receipt.to_dag_node()
                            .map_err(|e| anyhow::anyhow!("Failed to create DAG node: {}", e))?;
                        
                        // Serialize the node for signing
                        let node_bytes = serde_json::to_vec(&node)
                            .context("Failed to serialize node")?;
                        
                        // Sign the node
                        let signature = dummy_key.sign(&node_bytes);
                        
                        // Create a signed node
                        let signed_node = SignedDagNode {
                            node,
                            signature,
                            cid: None, // Will be computed when added to the DAG
                        };
                        
                        // Add to the DAG store
                        dag_store.add_node(signed_node)
                            .await
                            .map_err(|e| anyhow::anyhow!("Failed to add node to DAG: {}", e))?
                    };
                    
                    println!("ExecutionReceipt anchored to DAG with CID: {}", cid);
                },
                _ => {
                    return Err(anyhow::anyhow!("Unknown anchor type: {}. Expected 'bundle' or 'receipt'", anchor_type));
                }
            }
            
            Ok(())
        },
        
        DagCommands::Replay { cid, dag_dir } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Parse CID
            let cid_obj = parse_cid(cid)?;
            
            // Verify the branch
            match dag_store.verify_branch(&cid_obj).await {
                Ok(true) => {
                    println!("Branch verification successful!");
                    println!("All nodes in the branch are valid and properly linked.");
                    
                    // Find all ancestors for additional info
                    let node = dag_store.get_node(&cid_obj).await?;
                    println!("\nNode info:");
                    println!("  Author: {}", node.node.author);
                    println!("  Timestamp: {}", node.node.metadata.timestamp);
                    
                    if !node.node.parents.is_empty() {
                        println!("\nParent nodes:");
                        for (i, parent) in node.node.parents.iter().enumerate() {
                            println!("  {}. {}", i+1, parent);
                        }
                    }
                    
                    Ok(())
                },
                Ok(false) => {
                    Err(anyhow::anyhow!("Branch verification failed: The branch contains invalid nodes or missing links"))
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Branch verification error: {}", e))
                }
            }
        },
        
        DagCommands::VerifyBundle { cid, dag_dir } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Parse CID
            let cid_obj = parse_cid(cid)?;
            
            // Load the bundle
            let bundle = TrustBundle::from_dag(&cid_obj, &dag_store)
                .context("Failed to load TrustBundle from DAG")?;
                
            // Verify anchors
            match bundle.verify_anchors(&dag_store) {
                Ok(true) => {
                    println!("TrustBundle verification successful!");
                    println!("Bundle state CID: {}", bundle.state_cid);
                    println!("Policy ID: {}", bundle.state_proof.policy_id);
                    
                    println!("\nSignatures:");
                    for (i, (did, _)) in bundle.state_proof.signatures.iter().enumerate() {
                        println!("  {}. {}", i+1, did);
                    }
                    
                    if !bundle.previous_anchors.is_empty() {
                        println!("\nPrevious anchors:");
                        for (i, anchor) in bundle.previous_anchors.iter().enumerate() {
                            println!("  {}. {} ({})", i+1, anchor.cid,
                                anchor.object_type.as_deref().unwrap_or("Unknown"));
                        }
                    }
                    
                    Ok(())
                },
                Ok(false) => {
                    Err(anyhow::anyhow!("TrustBundle verification failed: Missing anchors"))
                },
                Err(e) => {
                    Err(anyhow::anyhow!("TrustBundle verification error: {}", e))
                }
            }
        },
        
        DagCommands::ExportThread { from, to, dag_dir, output } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Parse CIDs
            let from_cid = parse_cid(from)?;
            let to_cid = parse_cid(to)?;
            
            // Find path
            let path = dag_store.find_path(&from_cid, &to_cid)
                .context("Failed to find path between nodes")?;
                
            if path.is_empty() {
                return Err(anyhow::anyhow!("No path exists between the specified nodes"));
            }
            
            // Create thread export
            let export = serde_json::json!({
                "from": from_cid.to_string(),
                "to": to_cid.to_string(),
                "path_length": path.len(),
                "nodes": path.iter().map(|node| {
                    serde_json::json!({
                        "cid": node.cid.clone().unwrap().to_string(),
                        "author": node.node.author.to_string(),
                        "timestamp": node.node.metadata.timestamp,
                        "payload_type": match &node.node.payload {
                            icn_types::dag::DagPayload::Raw(_) => "raw",
                            icn_types::dag::DagPayload::Json(_) => "json",
                            icn_types::dag::DagPayload::Reference(_) => "reference",
                            icn_types::dag::DagPayload::TrustBundle(_) => "trustbundle",
                            icn_types::dag::DagPayload::ExecutionReceipt(_) => "receipt",
                        }
                    })
                }).collect::<Vec<_>>()
            });
            
            // Write to file
            fs::write(output, serde_json::to_string_pretty(&export)?)
                .context("Failed to write thread export file")?;
                
            println!("Thread exported successfully!");
            println!("Path length: {} nodes", path.len());
            println!("Saved to: {}", output.display());
            
            Ok(())
        },
        
        DagCommands::Sync { peer, federation, peer_id, dag_dir, trust } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Create a federation peer
            let federation_peer = icn_types::dag::FederationPeer {
                id: peer_id.clone(),
                endpoint: peer.clone(),
                federation_id: federation.clone(),
                metadata: None,
            };
            
            // Create a sync service
            let mut sync_service = icn_types::dag::sync::memory::MemoryDAGSyncService::new(
                dag_store,
                federation.clone(),
                "local-peer".to_string(), // In a real implementation, we'd use a persistent peer ID
            );
            
            // Add the peer to the service
            sync_service.add_peer(federation_peer.clone(), *trust);
            
            // Sync with the peer
            println!("Synchronizing with peer {} at {}", peer_id, peer);
            println!("Federation: {}", federation);
            println!("Trust level: {}", trust);
            
            match sync_service.sync_with_peer(&federation_peer).await {
                Ok(result) => {
                    println!("\nSync result:");
                    println!("  Valid: {}", result.is_valid);
                    println!("  Accepted nodes: {}", result.accepted_nodes.len());
                    println!("  Rejected nodes: {}", result.rejected_nodes.len());
                    println!("\nDetailed report:");
                    println!("{}", result.report);
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Sync failed: {}", e))
                }
            }
        },

        DagCommands::SyncP2P(sync_cmd) => handle_dag_sync_command(sync_cmd).await,

        DagCommands::Visualize { dag_dir, output, thread_did, max_nodes } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Get DAG nodes
            let nodes = if let Some(did_str) = thread_did {
                // If filtering by author, get nodes by author
                let did = Did::from(did_str.clone());
                store.get_nodes_by_author(&did).await?
            } else {
                // Otherwise get all ordered nodes
                store.get_ordered_nodes().await?
            };
            
            // Limit to max_nodes
            let nodes = if nodes.len() > *max_nodes {
                println!("Limiting to {} nodes out of {}", max_nodes, nodes.len());
                nodes.into_iter().take(*max_nodes).collect()
            } else {
                nodes
            };
            
            // Generate DOT format for graph visualization
            let mut dot = String::new();
            dot.push_str("digraph DAG {\n");
            dot.push_str("  rankdir=LR;\n");
            dot.push_str("  node [shape=box style=filled];\n\n");
            
            // Add nodes
            for node in &nodes {
                let node_id = node.cid.as_ref().unwrap().to_string();
                let short_id = &node_id[0..10]; // Use shortened CID for readability
                
                let label = match &node.node.payload {
                    icn_types::dag::DagPayload::Json(value) => {
                        if let Some(type_val) = value.get("type") {
                            if let Some(type_str) = type_val.as_str() {
                                type_str.to_string()
                            } else {
                                "JSON".to_string()
                            }
                        } else {
                            "JSON".to_string()
                        }
                    },
                    icn_types::dag::DagPayload::Raw(_) => "Raw".to_string(),
                    icn_types::dag::DagPayload::Reference(_) => "Reference".to_string(),
                    icn_types::dag::DagPayload::TrustBundle(_) => "TrustBundle".to_string(),
                    icn_types::dag::DagPayload::ExecutionReceipt(_) => "Receipt".to_string(),
                };
                
                // Customize node color by payload type
                let color = match &node.node.payload {
                    icn_types::dag::DagPayload::TrustBundle(_) => "lightblue",
                    icn_types::dag::DagPayload::ExecutionReceipt(_) => "lightgreen",
                    icn_types::dag::DagPayload::Json(_) => "lightyellow",
                    _ => "white",
                };
                
                // Create node with metadata
                dot.push_str(&format!(
                    "  \"{}\" [label=\"{} ({}...)\\nAuthor: {}...\\nTime: {}\", fillcolor=\"{}\"];\n",
                    node_id,
                    label,
                    short_id,
                    node.node.author.to_string()[0..15],
                    node.node.metadata.timestamp.format("%Y-%m-%d %H:%M"),
                    color
                ));
                
                // Add edges from this node to its parents
                for parent in &node.node.parents {
                    dot.push_str(&format!("  \"{}\" -> \"{}\";\n", node_id, parent.to_string()));
                }
            }
            
            dot.push_str("}\n");
            
            // Write to output file
            fs::write(output, dot)?;
            
            println!("DAG visualization generated successfully!");
            println!("Saved to: {}", output.display());
            println!("Number of nodes: {}", nodes.len());
            println!("Generate an image with: dot -Tpng {} -o dag.png", output.display());
            
            Ok(())
        },
    }
}

/// Handler for the mesh computation commands
async fn handle_mesh_command(cmd: &MeshCommands) -> Result<()> {
    match cmd {
        MeshCommands::PublishTask { 
            wasm_file, 
            input, 
            max_latency, 
            memory, 
            cores, 
            priority, 
            federation, 
            key, 
            dag_dir 
        } => {
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let author_did = Did::from(did_str.to_string());
            
            // In a real implementation, we would load the private key
            // For now, we'll use a placeholder by generating a new key
            let dummy_key = DidKey::new();
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Read WASM file contents
            let wasm_bytes = fs::read(wasm_file)
                .context("Failed to read WASM file")?;
            
            // Create task ticket payload
            let task_payload = serde_json::json!({
                "type": "TaskTicket",
                "wasm_hash": format!("0x{}", hex::encode(blake3::hash(&wasm_bytes).as_bytes())),
                "wasm_size": wasm_bytes.len(),
                "inputs": input,
                "requirements": {
                    "max_latency_ms": max_latency,
                    "memory_mb": memory,
                    "cores": cores,
                    "priority": priority
                },
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "federation_id": federation,
            });
            
            // Create a DAG node for this task ticket
            let task_node = icn_types::dag::DagNodeBuilder::new()
                .with_payload(icn_types::dag::DagPayload::Json(task_payload))
                .with_author(author_did.clone())
                .with_federation_id(federation.clone())
                .with_label("TaskTicket".to_string())
                .build()?;
                
            // Sign the task node
            let node_bytes = serde_json::to_vec(&task_node)
                .context("Failed to serialize task node")?;
            let signature = dummy_key.sign(&node_bytes);
            
            // Create a signed node
            let signed_task_node = icn_types::dag::SignedDagNode {
                node: task_node,
                signature,
                cid: None, // Will be computed when added to the DAG
            };
            
            // Add to the DAG store to get its CID
            let task_cid = store.add_node(signed_task_node)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add task node to DAG: {}", e))?;
                
            println!("Task ticket published with CID: {}", task_cid);
            println!("WASM size: {} bytes", wasm_bytes.len());
            println!("Requirements:");
            println!("  Max latency: {} ms", max_latency);
            println!("  Memory: {} MB", memory);
            println!("  Cores: {}", cores);
            println!("  Priority: {}", priority);
            
            // Create a transport config for publishing to peers
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(),
                federation_id: federation.clone(),
                local_did: Some(author_did.clone()),
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                bootstrap_peers: vec![],
                enable_mdns: true,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create the sync service
            let sync_service = NetworkDagSyncService::new(
                transport,
                store.clone(),
                federation.clone(),
                Some(author_did.clone()),
            );
            
            // Start background sync to publish the task
            println!("Starting task publication to peers...");
            sync_service.start_background_sync().await?;
            
            // Wait a bit for publication
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            
            println!("Task published. Listen for bids by starting a scheduler node.");
            
            // Record metrics for task publication
            if let Ok(metrics_context) = init_metrics(&federation, None) {
                metrics_context.record_task_published(&task_cid.to_string(), wasm_bytes.len() as u64);
            }
            
            Ok(())
        },
        
        MeshCommands::Bid { 
            task_cid, 
            latency, 
            memory, 
            cores, 
            reputation, 
            renewable, 
            key, 
            dag_dir 
        } => {
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let author_did = Did::from(did_str.to_string());
            
            // In a real implementation, we would load the private key
            // For now, we'll use a placeholder by generating a new key
            let dummy_key = DidKey::new();
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Parse task CID
            let task_cid_obj = parse_cid(task_cid)?;
            
            // Retrieve the task node
            let task_node = store.get_node(&task_cid_obj).await?;
            
            // Extract federation ID from task node
            let federation_id = task_node.node.federation_id.clone();
            
            // Create bid payload
            let bid_payload = serde_json::json!({
                "type": "TaskBid",
                "task_cid": task_cid,
                "offered_resources": {
                    "latency_ms": latency,
                    "memory_mb": memory,
                    "cores": cores,
                    "reputation": reputation,
                    "renewable_energy_pct": renewable
                },
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "bidder_did": author_did.to_string(),
                "compute_location": "local", // This would be more detailed in a real implementation
            });
            
            // Calculate bid score - lower is better
            // Simple formula: score = latency * (100 - reputation) / (memory * cores * (1 + renewable/100))
            let score = *latency as f64 * (100.0 - *reputation as f64) / 
                        (*memory as f64 * *cores as f64 * (1.0 + *renewable as f64 / 100.0));
            
            // Create a DAG node for this bid
            let bid_node = icn_types::dag::DagNodeBuilder::new()
                .with_payload(icn_types::dag::DagPayload::Json(bid_payload))
                .with_author(author_did.clone())
                .with_federation_id(federation_id.clone())
                .with_parents(vec![task_cid_obj.clone()]) // Link to the task
                .with_label("TaskBid".to_string())
                .build()?;
                
            // Sign the bid node
            let node_bytes = serde_json::to_vec(&bid_node)
                .context("Failed to serialize bid node")?;
            let signature = dummy_key.sign(&node_bytes);
            
            // Create a signed node
            let signed_bid_node = icn_types::dag::SignedDagNode {
                node: bid_node,
                signature,
                cid: None, // Will be computed when added to the DAG
            };
            
            // Add to the DAG store to get its CID
            let bid_cid = store.add_node(signed_bid_node)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add bid node to DAG: {}", e))?;
                
            println!("Bid submitted with CID: {}", bid_cid);
            println!("Bid score: {:.4} (lower is better)", score);
            println!("Offered resources:");
            println!("  Latency: {} ms", latency);
            println!("  Memory: {} MB", memory);
            println!("  Cores: {}", cores);
            println!("  Reputation: {}", reputation);
            println!("  Renewable Energy: {}%", renewable);
            
            // Create a transport config for publishing to peers
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(),
                federation_id: federation_id.clone(),
                local_did: Some(author_did.clone()),
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                bootstrap_peers: vec![],
                enable_mdns: true,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create the sync service
            let sync_service = NetworkDagSyncService::new(
                transport,
                store.clone(),
                federation_id.clone(),
                Some(author_did.clone()),
            );
            
            // Start background sync to publish the bid
            println!("Starting bid publication to peers...");
            sync_service.start_background_sync().await?;
            
            // Wait a bit for publication
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            
            println!("Bid published successfully.");
            
            // Record metrics for bid submission
            if let Ok(metrics_context) = init_metrics(&federation_id, None) {
                metrics_context.record_bid_submitted(
                    &bid_cid.to_string(),
                    &task_cid.to_string(),
                    *latency,
                    score
                );
            }
            
            Ok(())
        },
        
        MeshCommands::Scheduler { 
            federation, 
            key, 
            dag_dir, 
            listen, 
            mdns,
            require 
        } => {
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let did = Did::from(did_str.to_string());
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Create transport config for the scheduler node
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(),
                federation_id: federation.clone(),
                local_did: Some(did.clone()),
                listen_addresses: vec![listen.clone()],
                bootstrap_peers: vec![],
                enable_mdns: *mdns,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            println!("Starting scheduler node for federation: {}", federation);
            println!("Listening on: {}", listen);
            
            // Create capability selector from --require arguments
            let mut capability_selector = None;
            if !require.is_empty() {
                let mut selector = icn_planetary_mesh::cap_index::CapabilitySelector::new();
                
                for (key, value) in require {
                    if let Err(e) = selector.parse_requirement(&key, &value) {
                        eprintln!("Warning: Failed to parse requirement {}={}: {}", key, value, e);
                    } else {
                        println!("Added requirement: {}={}", key, value);
                    }
                }
                
                capability_selector = Some(selector);
            }
            
            // Configure capability index with signature verification
            let cap_index_config = icn_planetary_mesh::scheduler::CapabilityIndexConfig {
                verify_signatures: true,
                require_valid_signatures: true, // Require valid signatures for security
                trusted_dids: None, // You could add a list of trusted DIDs here
            };
            
            // Create the capability index with verification config
            let cap_index = Arc::new(icn_planetary_mesh::scheduler::CapabilityIndex::with_config(
                store.clone(), 
                cap_index_config
            ));
            
            // Create a DID key for signing (in a real implementation, this would be loaded from the key file)
            // For demonstration, we'll create a new key
            let did_key = icn_identity_core::did::DidKey::new();
            
            // Create the scheduler with the signing key
            let scheduler = icn_planetary_mesh::scheduler::Scheduler::new_with_key(
                federation.clone(),
                cap_index.clone(),
                store.clone(),
                did_key,
            );
            
            // Start the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create the sync service
            let sync_service = NetworkDagSyncService::new(
                transport,
                store.clone(),
                federation.clone(),
                Some(did.clone()),
            );
            
            // Start background sync to publish the task
            println!("Starting mesh network synchronization...");
            sync_service.start_background_sync().await?;
            
            // Set up metrics for verification failures
            let metrics_context = init_metrics(&federation, None)?;
            let failure_counter = counter!("icn_manifest_verification_failures", 
                "Number of manifest verification failures");
            
            // Hook up the verification failure handler
            cap_index.set_verification_failure_handler(Box::new(move |did, error| {
                failure_counter.inc();
                error!("Manifest verification failed for DID {}: {:?}", did, error);
                metrics_context.record_manifest_verification_failure(&did.to_string(), &format!("{:?}", error));
            }));
            
            // In a real implementation, this would be a long-running process
            // that listens for new task requests, collects bids, and matches them
            println!("Scheduler is running. Press Ctrl+C to exit.");
            println!("This is a simulated implementation. In a real implementation, this would:");
            println!("  1. Listen for incoming task requests");
            println!("  2. Request bids from nodes with matching capabilities");
            println!("  3. Score and select the best bid");
            println!("  4. Notify the winning node and requestor");
            
            if let Some(selector) = &capability_selector {
                println!("\nUsing capability selector with requirements:");
                if let Some(arch) = &selector.arch {
                    println!("  - Architecture: {:?}", arch);
                }
                if let Some(cores) = selector.min_cores {
                    println!("  - Minimum cores: {}", cores);
                }
                if let Some(ram) = selector.min_ram_mb {
                    println!("  - Minimum RAM: {} MB", ram);
                }
                if let Some(storage) = selector.min_storage_bytes {
                    println!("  - Minimum storage: {} bytes", storage);
                }
                // Add more detailed information as needed
            }
            
            // Record metrics for scheduler startup
            if let Ok(metrics_context) = init_metrics(&federation, None) {
                metrics_context.record_scheduler_started();
            }
            
            // Wait for user input to exit
            tokio::signal::ctrl_c().await?;
            
            Ok(())
        },
        
        MeshCommands::Execute { task_cid, bid_cid, key, dag_dir, output_dir } => {
            println!("Executing task: {}", task_cid);
            println!("Based on accepted bid: {}", bid_cid);
            
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let executor_did = Did::from(did_str.to_string());
            
            // In a real implementation, we would load the private key
            // For now, we'll use a placeholder by generating a new key
            let dummy_key = DidKey::new();
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Parse CIDs
            let task_cid_obj = parse_cid(task_cid)?;
            let bid_cid_obj = parse_cid(bid_cid)?;
            
            // Get task node from DAG
            let task_node = store.get_node(&task_cid_obj).await?;
            if let icn_types::dag::DagPayload::Json(task_payload) = &task_node.node.payload {
                println!("Task type: {}", task_payload.get("type").and_then(|t| t.as_str()).unwrap_or("Unknown"));
                
                // Extract federation ID from task node
                let federation_id = task_node.node.federation_id.clone();
                
                // Get bid node from DAG
                let bid_node = store.get_node(&bid_cid_obj).await?;
                
                if let icn_types::dag::DagPayload::Json(bid_payload) = &bid_node.node.payload {
                    // Track resource usage (in a real implementation, we would measure actual usage)
                    let start_time = std::time::Instant::now();
                    
                    // Record starting resource metrics
                    let start_metrics = serde_json::json!({
                        "timestamp_start": chrono::Utc::now().to_rfc3339(),
                        "memory_available_mb": sys_info::mem_info().map(|m| m.total / 1024).unwrap_or(0),
                        "cpu_count": sys_info::cpu_num().unwrap_or(1),
                    });
                    
                    // Here we would actually execute the WASM module
                    // For demonstration, we'll simulate execution with a delay
                    println!("Simulating WASM execution...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    
                    // Create the output directory
                    fs::create_dir_all(output_dir)?;
                    
                    // Generate a simulated hash of the result for verification
                    let result_hash = format!("0x{}", hex::encode(blake3::hash(b"result data").as_bytes()));
                    
                    // Record ending resource metrics
                    let execution_time_ms = start_time.elapsed().as_millis();
                    let end_metrics = serde_json::json!({
                        "timestamp_end": chrono::Utc::now().to_rfc3339(),
                        "execution_time_ms": execution_time_ms,
                        "memory_peak_mb": 512, // Simulated value
                        "cpu_usage_pct": 75,    // Simulated value
                        "io_read_bytes": 1024 * 1024 * 5,  // Simulated 5MB read
                        "io_write_bytes": 1024 * 1024 * 2, // Simulated 2MB write
                    });
                    
                    // Calculate token compensation based on resource usage and bid
                    let offered_resources = bid_payload["offered_resources"].as_object()
                        .context("Bid payload missing offered_resources field")?;
                    
                    // Basic token calculation: time * (memory + cores) * reputation factor
                    let memory_mb = offered_resources["memory_mb"].as_u64().unwrap_or(1);
                    let cores = offered_resources["cores"].as_u64().unwrap_or(1);
                    let reputation = offered_resources["reputation"].as_u64().unwrap_or(50);
                    
                    // Token amount based on resources used and reputation
                    let token_amount = (execution_time_ms as f64 / 1000.0) * (memory_mb + cores) as f64 * (reputation as f64 / 50.0);
                    
                    // Round to 6 decimal places
                    let token_amount = (token_amount * 1_000_000.0).round() / 1_000_000.0;
                    
                    // Create token transfer data
                    let token_transfer = serde_json::json!({
                        "type": "ResourceTokenTransfer",
                        "from": task_node.node.author.to_string(),
                        "to": executor_did.to_string(),
                        "amount": token_amount,
                        "token_type": "COMPUTE",
                        "federation_id": federation_id,
                        "task_cid": task_cid,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    
                    // Write a result file with execution details and token info
                    let result_file = Path::new(output_dir).join("result.json");
                    let result_data = serde_json::json!({
                        "task_cid": task_cid,
                        "bid_cid": bid_cid,
                        "status": "completed",
                        "result_hash": result_hash,
                        "token_compensation": token_transfer,
                        "resource_metrics": {
                            "start": start_metrics,
                            "end": end_metrics
                        }
                    });
                    
                    fs::write(&result_file, serde_json::to_string_pretty(&result_data)?)
                        .context("Failed to write result file")?;
                    
                    // Create execution receipt with verifiable credential format
                    let execution_receipt = serde_json::json!({
                        "type": "ExecutionReceipt",
                        "credential": {
                            "@context": [
                                "https://www.w3.org/2018/credentials/v1",
                                "https://icn.network/credentials/compute/v1"
                            ],
                            "id": format!("urn:icn:receipt:{}", uuid::Uuid::new_v4()),
                            "type": ["VerifiableCredential", "ExecutionReceipt"],
                            "issuer": executor_did.to_string(),
                            "issuanceDate": chrono::Utc::now().to_rfc3339(),
                            "credentialSubject": {
                                "id": task_node.node.author.to_string(),
                                "taskCid": task_cid,
                                "bidCid": bid_cid,
                                "executionTime": execution_time_ms,
                                "resourceUsage": {
                                    "memoryMb": end_metrics["memory_peak_mb"],
                                    "cpuCores": cores,
                                    "cpuUsagePercent": end_metrics["cpu_usage_pct"],
                                    "ioReadBytes": end_metrics["io_read_bytes"],
                                    "ioWriteBytes": end_metrics["io_write_bytes"]
                                },
                                "resultHash": result_hash,
                                "tokenCompensation": token_amount
                            }
                        },
                        "execution_details": {
                            "federation_id": federation_id,
                            "executor_id": executor_did.to_string(),
                            "start_time": start_metrics["timestamp_start"],
                            "end_time": end_metrics["timestamp_end"],
                            "status": "completed"
                        },
                        "token_transfer": token_transfer,
                        "task_cid": task_cid,
                        "bid_cid": bid_cid
                    });
                    
                    // Create the execution receipt node to add to the DAG
                    let receipt_node = icn_types::dag::DagNodeBuilder::new()
                        .with_payload(icn_types::dag::DagPayload::Json(execution_receipt))
                        .with_author(executor_did.clone())
                        .with_federation_id(federation_id.clone())
                        .with_parents(vec![task_cid_obj.clone(), bid_cid_obj.clone()]) // Link to task and bid
                        .with_label("ExecutionReceipt".to_string())
                        .build()?;
                        
                    // Sign the receipt node
                    let node_bytes = serde_json::to_vec(&receipt_node)
                        .context("Failed to serialize receipt node")?;
                    let signature = dummy_key.sign(&node_bytes);
                    
                    // Create a signed node
                    let signed_receipt_node = icn_types::dag::SignedDagNode {
                        node: receipt_node,
                        signature,
                        cid: None, // Will be computed when added to the DAG
                    };
                    
                    // Add to the DAG store to get its CID
                    let receipt_cid = store.add_node(signed_receipt_node)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to add receipt node to DAG: {}", e))?;
                    
                    // Create a token transfer node to add to the DAG
                    let token_node = icn_types::dag::DagNodeBuilder::new()
                        .with_payload(icn_types::dag::DagPayload::Json(token_transfer))
                        .with_author(task_node.node.author.clone()) // The task creator is the token sender
                        .with_federation_id(federation_id.clone())
                        .with_parents(vec![receipt_cid.clone()]) // Link to the receipt
                        .with_label("ResourceTokenTransfer".to_string())
                        .build()?;
                        
                    // Sign the token node with the task creator's key (in a real implementation)
                    // Here we're just using our dummy key for demonstration
                    let token_bytes = serde_json::to_vec(&token_node)
                        .context("Failed to serialize token node")?;
                    let token_signature = dummy_key.sign(&token_bytes);
                    
                    // Create a signed node
                    let signed_token_node = icn_types::dag::SignedDagNode {
                        node: token_node,
                        signature: token_signature,
                        cid: None, // Will be computed when added to the DAG
                    };
                    
                    // Add to the DAG store to get its CID
                    let token_cid = store.add_node(signed_token_node)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to add token node to DAG: {}", e))?;
                    
                    println!("Execution complete!");
                    println!("Receipt anchored to DAG with CID: {}", receipt_cid);
                    println!("Token transfer anchored to DAG with CID: {}", token_cid);
                    println!("Results saved to: {}", result_file.display());
                    println!("Token compensation: {} COMPUTE tokens", token_amount);
                    
                    // Create a transport config for publishing to peers
                    let transport_config = TransportConfig {
                        peer_id: uuid::Uuid::new_v4().to_string(),
                        federation_id: federation_id.clone(),
                        local_did: Some(executor_did.clone()),
                        listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                        bootstrap_peers: vec![],
                        enable_mdns: true,
                        enable_kad_dht: false,
                        max_message_size: 1024 * 1024, // 1MB
                        request_timeout: 30, // 30 seconds
                    };
                    
                    // Create the transport
                    let transport = Libp2pDagTransport::new(transport_config).await?;
                    
                    // Create the sync service
                    let sync_service = NetworkDagSyncService::new(
                        transport,
                        store.clone(),
                        federation_id.clone(),
                        Some(executor_did.clone()),
                    );
                    
                    // Start background sync to publish the receipt and token transfer
                    println!("Publishing receipt and token transfer to peers...");
                    sync_service.start_background_sync().await?;
                    
                    // Wait a bit for publication
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    
                    println!("Receipt and token transfer published successfully.");
                    
                    // Record metrics for execution completion
                    if let Ok(metrics_context) = init_metrics(&federation_id, None) {
                        metrics_context.record_execution_completed(
                            &task_cid.to_string(),
                            execution_time_ms as u64,
                            end_metrics["memory_peak_mb"].as_u64().unwrap_or(0),
                            end_metrics["cpu_usage_pct"].as_u64().unwrap_or(0)
                        );
                        
                        // Record token transfer metrics
                        metrics_context.record_token_transfer(
                            &task_node.node.author.to_string(),
                            &executor_did.to_string(),
                            token_amount
                        );
                    }
                } else {
                    return Err(anyhow::anyhow!("Invalid bid node payload type"));
                }
            } else {
                return Err(anyhow::anyhow!("Invalid task node payload type"));
            }
            
            Ok(())
        },
        
        MeshCommands::VerifyReceipt { receipt_cid, dag_dir } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Parse receipt CID
            let receipt_cid_obj = parse_cid(receipt_cid)?;
            
            // Get receipt node from DAG
            let receipt_node = store.get_node(&receipt_cid_obj).await?;
            
            if let icn_types::dag::DagPayload::Json(receipt_payload) = &receipt_node.node.payload {
                // Verify receipt has the correct structure
                if receipt_payload.get("type").and_then(|t| t.as_str()) != Some("ExecutionReceipt") {
                    return Err(anyhow::anyhow!("Node is not an ExecutionReceipt"));
                }
                
                // Extract task and bid CIDs
                let task_cid = receipt_payload["task_cid"].as_str()
                    .context("Receipt missing task_cid field")?;
                let bid_cid = receipt_payload["bid_cid"].as_str()
                    .context("Receipt missing bid_cid field")?;
                
                // Parse task and bid CIDs
                let task_cid_obj = parse_cid(task_cid)?;
                let bid_cid_obj = parse_cid(bid_cid)?;
                
                // Get task and bid nodes
                let task_node = store.get_node(&task_cid_obj).await?;
                let bid_node = store.get_node(&bid_cid_obj).await?;
                
                // Verify task node is in parents of receipt
                if !receipt_node.node.parents.contains(&task_cid_obj) {
                    return Err(anyhow::anyhow!("Receipt node does not reference task node as parent"));
                }
                
                // Verify bid node is in parents of receipt
                if !receipt_node.node.parents.contains(&bid_cid_obj) {
                    return Err(anyhow::anyhow!("Receipt node does not reference bid node as parent"));
                }
                
                // Verify signature of receipt (in a real implementation)
                // Here we're just checking that the signature exists
                if receipt_node.signature.is_empty() {
                    return Err(anyhow::anyhow!("Receipt has invalid signature"));
                }
                
                // Check if token transfer exists for this receipt
                let mut token_node = None;
                let token_nodes = store.get_children(&receipt_cid_obj).await?;
                for node in token_nodes {
                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                        if payload.get("type").and_then(|t| t.as_str()) == Some("ResourceTokenTransfer") {
                            token_node = Some(node);
                            break;
                        }
                    }
                }
                
                println!("ExecutionReceipt verification successful!");
                println!("Receipt CID: {}", receipt_cid);
                println!("Task CID: {}", task_cid);
                println!("Bid CID: {}", bid_cid);
                
                // Display receipt details
                if let Some(credential) = receipt_payload.get("credential") {
                    if let Some(subject) = credential.get("credentialSubject") {
                        println!("\nCredential details:");
                        println!("  Issuer: {}", credential.get("issuer").and_then(|i| i.as_str()).unwrap_or("Unknown"));
                        println!("  Issuance date: {}", credential.get("issuanceDate").and_then(|d| d.as_str()).unwrap_or("Unknown"));
                        println!("  Subject ID: {}", subject.get("id").and_then(|i| i.as_str()).unwrap_or("Unknown"));
                        
                        println!("\nExecution metrics:");
                        println!("  Execution time: {} ms", subject.get("executionTime").and_then(|t| t.as_u64()).unwrap_or(0));
                        
                        if let Some(resource_usage) = subject.get("resourceUsage") {
                            println!("  Memory: {} MB", resource_usage.get("memoryMb").and_then(|m| m.as_u64()).unwrap_or(0));
                            println!("  CPU cores: {}", resource_usage.get("cpuCores").and_then(|c| c.as_u64()).unwrap_or(0));
                            println!("  CPU usage: {}%", resource_usage.get("cpuUsagePercent").and_then(|u| u.as_u64()).unwrap_or(0));
                            println!("  I/O read: {} bytes", resource_usage.get("ioReadBytes").and_then(|r| r.as_u64()).unwrap_or(0));
                            println!("  I/O write: {} bytes", resource_usage.get("ioWriteBytes").and_then(|w| w.as_u64()).unwrap_or(0));
                        }
                        
                        println!("  Result hash: {}", subject.get("resultHash").and_then(|h| h.as_str()).unwrap_or("Unknown"));
                        println!("  Token compensation: {} COMPUTE", subject.get("tokenCompensation").and_then(|t| t.as_f64()).unwrap_or(0.0));
                    }
                }
                
                // Display token transfer info if found
                if let Some(node) = token_node {
                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                        println!("\nToken transfer details:");
                        println!("  From: {}", payload.get("from").and_then(|f| f.as_str()).unwrap_or("Unknown"));
                        println!("  To: {}", payload.get("to").and_then(|t| t.as_str()).unwrap_or("Unknown"));
                        println!("  Amount: {} {}", 
                            payload.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0),
                            payload.get("token_type").and_then(|t| t.as_str()).unwrap_or("Unknown"));
                        println!("  Transfer CID: {}", node.cid.as_ref().unwrap());
                    }
                } else {
                    println!("\nNo token transfer found for this receipt.");
                }
            } else {
                return Err(anyhow::anyhow!("Invalid receipt node payload type"));
            }
            
            Ok(())
        },
        
        MeshCommands::CheckBalance { did, key, dag_dir, federation } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Determine the DID to check
            let check_did = if let Some(did_str) = did {
                Did::from(did_str)
            } else if let Some(key_path) = key {
                // Load the key file
                let key_data = fs::read_to_string(key_path)
                    .context("Failed to read key file")?;
                let key_json: Value = serde_json::from_str(&key_data)
                    .context("Failed to parse key file as JSON")?;
                let did_str = key_json["did"].as_str()
                    .context("Key file missing 'did' field")?;
                
                Did::from(did_str.to_string())
            } else {
                return Err(anyhow::anyhow!("Either DID or key file must be provided"));
            };
            
            println!("Checking token balance for DID: {}", check_did);
            println!("Federation: {}", federation);
            
            // Get all token transfer nodes
            let all_nodes = store.get_ordered_nodes().await?;
            
            // Filter for token transfers in this federation
            let mut received_tokens = 0.0;
            let mut sent_tokens = 0.0;
            let mut transfers = Vec::new();
            
            for node in all_nodes {
                if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                    if payload.get("type").and_then(|t| t.as_str()) == Some("ResourceTokenTransfer") {
                        if payload.get("federation_id").and_then(|f| f.as_str()) == Some(&federation) {
                            // Check if this DID is sender or receiver
                            let from_did = payload.get("from").and_then(|f| f.as_str())
                                .map(|s| Did::from(s.to_string()));
                            let to_did = payload.get("to").and_then(|t| t.as_str())
                                .map(|s| Did::from(s.to_string()));
                            
                            let amount = payload.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0);
                            let token_type = payload.get("token_type").and_then(|t| t.as_str()).unwrap_or("UNKNOWN");
                            let timestamp = payload.get("timestamp").and_then(|t| t.as_str()).unwrap_or("Unknown");
                            
                            if let Some(from) = &from_did {
                                if from == &check_did {
                                    sent_tokens += amount;
                                    transfers.push((timestamp.to_string(), -amount, token_type.to_string()));
                                }
                            }
                            
                            if let Some(to) = &to_did {
                                if to == &check_did {
                                    received_tokens += amount;
                                    transfers.push((timestamp.to_string(), amount, token_type.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            
            // Calculate net balance
            let net_balance = received_tokens - sent_tokens;
            
            println!("\nToken balance:");
            println!("  Received: {:.6} COMPUTE", received_tokens);
            println!("  Sent: {:.6} COMPUTE", sent_tokens);
            println!("  Net balance: {:.6} COMPUTE", net_balance);
            
            if !transfers.is_empty() {
                println!("\nRecent transfers:");
                // Sort transfers by timestamp (most recent first)
                transfers.sort_by(|a, b| b.0.cmp(&a.0));
                
                // Display the last 10 transfers (or all if fewer)
                let display_transfers = if transfers.len() > 10 {
                    &transfers[0..10]
                } else {
                    &transfers
                };
                
                for (timestamp, amount, token_type) in display_transfers {
                    let direction = if *amount > 0.0 { "RECEIVED" } else { "SENT" };
                    println!("  [{}] {} {:.6} {}", timestamp, direction, amount.abs(), token_type);
                }
            } else {
                println!("\nNo transfers found for this DID in federation {}.", federation);
            }
            
            Ok(())
        },
        
        MeshCommands::MetricsServer { federation, listen, dag_dir } => {
            // Parse the listen address
            let addr: SocketAddr = listen.parse()
                .map_err(|e| anyhow::anyhow!("Invalid listen address: {}", e))?;
            
            // Create DAG store
            let _store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Initialize metrics server
            let metrics_context = init_metrics(&federation, Some(addr))?;
            
            println!("Metrics server started on http://{}/metrics", addr);
            println!("Press Ctrl+C to stop the server");
            
            // Keep the process running
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        },
        
        MeshCommands::Stats { federation, dag_dir, type_filter, limit } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Get all nodes from the DAG
            let nodes = store.get_ordered_nodes().await?;
            
            println!("Federation: {}", federation);
            
            match type_filter.as_str() {
                "tasks" => {
                    let mut tasks = Vec::new();
                    
                    for node in &nodes {
                        if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                            if payload.get("type").and_then(|t| t.as_str()) == Some("TaskTicket") {
                                if let Some(cid) = &node.cid {
                                    tasks.push((
                                        cid.to_string(),
                                        node.node.author.to_string(),
                                        node.node.metadata.timestamp,
                                        payload.get("wasm_size").and_then(|s| s.as_u64()).unwrap_or(0)
                                    ));
                                }
                            }
                        }
                    }
                    
                    // Sort by timestamp (newest first)
                    tasks.sort_by(|a, b| b.2.cmp(&a.2));
                    
                    // Limit to the requested number
                    let tasks = if tasks.len() > limit {
                        tasks.into_iter().take(limit).collect::<Vec<_>>()
                    } else {
                        tasks
                    };
                    
                    println!("\nRecent tasks ({})", tasks.len());
                    println!("{:<10} {:<20} {:<30} {:<15}", "Index", "Author", "Timestamp", "Size (bytes)");
                    println!("{}", "-".repeat(80));
                    
                    for (i, (cid, author, timestamp, size)) in tasks.iter().enumerate() {
                        println!("{:<10} {:<20} {:<30} {:<15}", 
                            i + 1,
                            author.chars().take(18).collect::<String>(),
                            timestamp.format("%Y-%m-%d %H:%M:%S"),
                            size
                        );
                    }
                },
                "bids" => {
                    let mut bids = Vec::new();
                    
                    for node in &nodes {
                        if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                            if payload.get("type").and_then(|t| t.as_str()) == Some("TaskBid") {
                                if let Some(cid) = &node.cid {
                                    bids.push((
                                        cid.to_string(),
                                        node.node.author.to_string(),
                                        node.node.metadata.timestamp,
                                        payload.get("task_cid").and_then(|t| t.as_str()).unwrap_or("unknown"),
                                        payload["offered_resources"].get("latency_ms").and_then(|l| l.as_u64()).unwrap_or(0),
                                        payload["offered_resources"].get("memory_mb").and_then(|m| m.as_u64()).unwrap_or(0),
                                        payload["offered_resources"].get("cores").and_then(|c| c.as_u64()).unwrap_or(0),
                                        payload["offered_resources"].get("reputation").and_then(|r| r.as_u64()).unwrap_or(0)
                                    ));
                                }
                            }
                        }
                    }
                    
                    // Sort by timestamp (newest first)
                    bids.sort_by(|a, b| b.2.cmp(&a.2));
                    
                    // Limit to the requested number
                    let bids = if bids.len() > limit {
                        bids.into_iter().take(limit).collect::<Vec<_>>()
                    } else {
                        bids
                    };
                    
                    println!("\nRecent bids ({})", bids.len());
                    println!("{:<10} {:<20} {:<20} {:<10} {:<10} {:<10} {:<10}", 
                        "Index", "Bidder", "Timestamp", "Latency", "Memory", "Cores", "Rep");
                    println!("{}", "-".repeat(80));
                    
                    for (i, (_, author, timestamp, _, latency, memory, cores, reputation)) in bids.iter().enumerate() {
                        println!("{:<10} {:<20} {:<20} {:<10} {:<10} {:<10} {:<10}", 
                            i + 1,
                            author.chars().take(18).collect::<String>(),
                            timestamp.format("%Y-%m-%d %H:%M:%S"),
                            latency,
                            memory,
                            cores,
                            reputation
                        );
                    }
                },
                "tokens" => {
                    let mut transfers = Vec::new();
                    
                    for node in &nodes {
                        if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                            if payload.get("type").and_then(|t| t.as_str()) == Some("ResourceTokenTransfer") {
                                if let Some(cid) = &node.cid {
                                    transfers.push((
                                        cid.to_string(),
                                        node.node.metadata.timestamp,
                                        payload.get("from").and_then(|f| f.as_str()).unwrap_or("unknown"),
                                        payload.get("to").and_then(|t| t.as_str()).unwrap_or("unknown"),
                                        payload.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0),
                                        payload.get("token_type").and_then(|t| t.as_str()).unwrap_or("UNKNOWN")
                                    ));
                                }
                            }
                        }
                    }
                    
                    // Sort by timestamp (newest first)
                    transfers.sort_by(|a, b| b.1.cmp(&a.1));
                    
                    // Limit to the requested number
                    let transfers = if transfers.len() > limit {
                        transfers.into_iter().take(limit).collect::<Vec<_>>()
                    } else {
                        transfers
                    };
                    
                    println!("\nRecent token transfers ({})", transfers.len());
                    println!("{:<10} {:<20} {:<20} {:<20} {:<15} {:<10}", 
                        "Index", "Timestamp", "From", "To", "Amount", "Type");
                    println!("{}", "-".repeat(90));
                    
                    for (i, (_, timestamp, from, to, amount, token_type)) in transfers.iter().enumerate() {
                        println!("{:<10} {:<20} {:<20} {:<20} {:<15.6} {:<10}", 
                            i + 1,
                            timestamp.format("%Y-%m-%d %H:%M:%S"),
                            from.chars().take(18).collect::<String>(),
                            to.chars().take(18).collect::<String>(),
                            amount,
                            token_type
                        );
                    }
                    
                    // Calculate total token supply and distribution
                    let mut balances: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
                    
                    for (_, _, from, to, amount, _) in &transfers {
                        *balances.entry(from.to_string()).or_default() -= amount;
                        *balances.entry(to.to_string()).or_default() += amount;
                    }
                    
                    let mut balance_list: Vec<(String, f64)> = balances.into_iter().collect();
                    balance_list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    
                    println!("\nTop token holders:");
                    println!("{:<10} {:<30} {:<15}", "Rank", "DID", "Balance");
                    println!("{}", "-".repeat(60));
                    
                    for (i, (did, balance)) in balance_list.iter().take(5).enumerate() {
                        println!("{:<10} {:<30} {:<15.6}", 
                            i + 1,
                            did.chars().take(28).collect::<String>(),
                            balance
                        );
                    }
                },
                "receipts" => {
                    let mut receipts = Vec::new();
                    
                    for node in &nodes {
                        if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                            if payload.get("type").and_then(|t| t.as_str()) == Some("ExecutionReceipt") {
                                if let Some(cid) = &node.cid {
                                    if let Some(credential) = payload.get("credential") {
                                        if let Some(subject) = credential.get("credentialSubject") {
                                            receipts.push((
                                                cid.to_string(),
                                                node.node.author.to_string(),
                                                node.node.metadata.timestamp,
                                                subject.get("taskCid").and_then(|t| t.as_str()).unwrap_or("unknown"),
                                                subject.get("executionTime").and_then(|e| e.as_u64()).unwrap_or(0),
                                                subject.get("tokenCompensation").and_then(|t| t.as_f64()).unwrap_or(0.0)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Sort by timestamp (newest first)
                    receipts.sort_by(|a, b| b.2.cmp(&a.2));
                    
                    // Limit to the requested number
                    let receipts = if receipts.len() > limit {
                        receipts.into_iter().take(limit).collect::<Vec<_>>()
                    } else {
                        receipts
                    };
                    
                    println!("\nRecent execution receipts ({})", receipts.len());
                    println!("{:<10} {:<20} {:<20} {:<10} {:<15} {:<15}", 
                        "Index", "Executor", "Timestamp", "Exec Time", "Compensation", "Receipt CID");
                    println!("{}", "-".repeat(100));
                    
                    for (i, (cid, author, timestamp, _, exec_time, compensation)) in receipts.iter().enumerate() {
                        println!("{:<10} {:<20} {:<20} {:<10} {:<15.6} {:<15}", 
                            i + 1,
                            author.chars().take(18).collect::<String>(),
                            timestamp.format("%Y-%m-%d %H:%M:%S"),
                            exec_time,
                            compensation,
                            cid.chars().take(13).collect::<String>()
                        );
                    }
                    
                    // Calculate execution statistics
                    let total_executions = receipts.len();
                    let total_compute_time: u64 = receipts.iter().map(|(_, _, _, _, time, _)| time).sum();
                    let total_compensation: f64 = receipts.iter().map(|(_, _, _, _, _, comp)| comp).sum();
                    
                    let avg_time = if total_executions > 0 { 
                        total_compute_time as f64 / total_executions as f64 
                    } else { 
                        0.0 
                    };
                    
                    let avg_compensation = if total_executions > 0 { 
                        total_compensation / total_executions as f64 
                    } else { 
                        0.0 
                    };
                    
                    println!("\nExecution statistics:");
                    println!("  Total executions: {}", total_executions);
                    println!("  Total compute time: {} ms", total_compute_time);
                    println!("  Average execution time: {:.2} ms", avg_time);
                    println!("  Total token compensation: {:.6} COMPUTE", total_compensation);
                    println!("  Average compensation per task: {:.6} COMPUTE", avg_compensation);
                },
                "resources" => {
                    let mut resource_usage = std::collections::HashMap::new();
                    let mut resource_events = std::collections::HashMap::new();
                    let mut sensor_events = std::collections::HashMap::new();
                    let mut actuation_events = std::collections::HashMap::new();
                    let mut bandwidth_usage = (0u64, 0u64); // (in, out)
                    
                    // Process execution receipts to extract resource usage
                    for node in &nodes {
                        if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                            if payload.get("type").and_then(|t| t.as_str()) == Some("ExecutionReceipt") {
                                if let Some(credential) = payload.get("credential") {
                                    if let Some(subject) = credential.get("credentialSubject") {
                                        // Extract resource usage from the receipt
                                        if let Some(resources) = subject.get("resourceUsage") {
                                            if let Some(mem) = resources.get("memoryMb").and_then(|m| m.as_u64()) {
                                                *resource_usage.entry("Memory".to_string()).or_insert(0u64) += mem;
                                                *resource_events.entry("Memory".to_string()).or_insert(0u64) += 1;
                                            }
                                            
                                            if let Some(cpu) = subject.get("executionTime").and_then(|t| t.as_u64()) {
                                                *resource_usage.entry("CPU".to_string()).or_insert(0u64) += cpu;
                                                *resource_events.entry("CPU".to_string()).or_insert(0u64) += 1;
                                            }
                                            
                                            if let Some(io_read) = resources.get("ioReadBytes").and_then(|r| r.as_u64()) {
                                                *resource_usage.entry("IO".to_string()).or_insert(0u64) += io_read;
                                                *resource_events.entry("IO".to_string()).or_insert(0u64) += 1;
                                                bandwidth_usage.0 += io_read;
                                            }
                                            
                                            if let Some(io_write) = resources.get("ioWriteBytes").and_then(|w| w.as_u64()) {
                                                *resource_usage.entry("IO".to_string()).or_insert(0u64) += io_write;
                                                bandwidth_usage.1 += io_write;
                                            }
                                        }
                                        
                                        // Look for sensor events
                                        if let Some(sensors) = subject.get("sensorEvents") {
                                            if let Some(sensors_obj) = sensors.as_object() {
                                                for (sensor_type, count) in sensors_obj {
                                                    if let Some(count_val) = count.as_u64() {
                                                        *sensor_events.entry(sensor_type.clone()).or_insert(0u64) += count_val;
                                                    }
                                                }
                                            }
                                        }
                                        
                                        // Look for actuation events
                                        if let Some(actuations) = subject.get("actuationEvents") {
                                            if let Some(actuations_obj) = actuations.as_object() {
                                                for (actuation_type, count) in actuations_obj {
                                                    if let Some(count_val) = count.as_u64() {
                                                        *actuation_events.entry(actuation_type.clone()).or_insert(0u64) += count_val;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Display resource usage summary
                    println!("\nResource usage summary:");
                    println!("{:<15} {:<15} {:<15}", "Resource Type", "Total Usage", "Event Count");
                    println!("{}", "-".repeat(45));
                    
                    for (resource_type, usage) in &resource_usage {
                        let events = resource_events.get(resource_type).unwrap_or(&0);
                        let unit = match resource_type.as_str() {
                            "CPU" => "ms",
                            "Memory" => "MB",
                            "IO" => "bytes",
                            _ => "units"
                        };
                        
                        println!("{:<15} {:<15} {:<15}", 
                            resource_type,
                            format!("{} {}", usage, unit),
                            events
                        );
                    }
                    
                    // Display bandwidth usage
                    println!("\nBandwidth usage:");
                    println!("  Ingress: {} bytes", bandwidth_usage.0);
                    println!("  Egress:  {} bytes", bandwidth_usage.1);
                    
                    // Display sensor events if any
                    if !sensor_events.is_empty() {
                        println!("\nSensor events:");
                        println!("{:<20} {:<15}", "Sensor Type", "Event Count");
                        println!("{}", "-".repeat(35));
                        
                        for (sensor_type, count) in &sensor_events {
                            println!("{:<20} {:<15}", sensor_type, count);
                        }
                    }
                    
                    // Display actuation events if any
                    if !actuation_events.is_empty() {
                        println!("\nActuation events:");
                        println!("{:<20} {:<15}", "Actuation Type", "Trigger Count");
                        println!("{}", "-".repeat(35));
                        
                        for (actuation_type, count) in &actuation_events {
                            println!("{:<20} {:<15}", actuation_type, count);
                        }
                    }
                    
                    // Show resources with federation compensation policy if available
                    println!("\nFederation resource compensation policy:");
                    println!("  CPU: 0.1 tokens per second");
                    println!("  Memory: 0.05 tokens per MB per hour");
                    println!("  IO: 0.01 tokens per MB");
                    println!("  Sensor: 0.25 tokens per event");
                    println!("  Actuation: 0.5 tokens per trigger");
                },
                _ => {
                    return Err(anyhow::anyhow!("Unknown stats type: {}. Allowed types: tasks, bids, tokens, receipts, resources", type_filter));
                }
            }
            
            Ok(())
        },
        MeshCommands::Manifest { action, key, dag_dir, federation, output, cid, field, value, firmware_hash } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Load the key file
            let key_data = fs::read_to_string(&key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // For demo purposes, we'll create a new DidKey
            // In a real implementation, we would load the private key from the file
            let dummy_key = DidKey::new();
            let did = Did::from(did_str.to_string());
            
            match action.as_str() {
                "create" => {
                    // Get firmware hash or use a default
                    let hash = firmware_hash.unwrap_or_else(|| "unknown-firmware-hash".to_string());
                    
                    // Create a new manifest from system information
                    let manifest = icn_identity_core::manifest::NodeManifest::from_system(did.clone(), &hash)
                        .context("Failed to create manifest from system information")?;
                    
                    // Save to a file if output is specified
                    if let Some(output_path) = output {
                        let manifest_json = serde_json::to_string_pretty(&manifest)
                            .context("Failed to serialize manifest to JSON")?;
                        fs::write(&output_path, manifest_json)
                            .context("Failed to write manifest to file")?;
                        
                        println!("Created node manifest for DID: {}", did_str);
                        println!("Saved to: {}", output_path.display());
                    } else {
                        // Print the manifest
                        let manifest_json = serde_json::to_string_pretty(&manifest)
                            .context("Failed to serialize manifest to JSON")?;
                        println!("{}", manifest_json);
                    }
                },
                "publish" => {
                    // Get firmware hash or use a default
                    let hash = firmware_hash.unwrap_or_else(|| "unknown-firmware-hash".to_string());
                    
                    // Create a manifest
                    let mut manifest = icn_identity_core::manifest::NodeManifest::from_system(did.clone(), &hash)
                        .context("Failed to create manifest from system information")?;
                    
                    // Update the timestamp
                    manifest.last_seen = chrono::Utc::now();
                    
                    // Sign the manifest
                    let manifest_json = serde_json::to_vec(&manifest)
                        .context("Failed to serialize manifest")?;
                    manifest.signature = dummy_key.sign(&manifest_json);
                    
                    // Convert to a verifiable credential
                    let manifest_vc = manifest.to_verifiable_credential();
                    
                    // Create a DAG node for the manifest
                    let node = icn_types::dag::DagNodeBuilder::new()
                        .with_payload(icn_types::dag::DagPayload::Json(manifest_vc))
                        .with_author(did.clone())
                        .with_federation_id(federation.clone())
                        .with_label("NodeManifest".to_string())
                        .build()
                        .context("Failed to build DAG node")?;
                        
                    // Serialize the node for signing
                    let node_bytes = serde_json::to_vec(&node)
                        .context("Failed to serialize node")?;
                    
                    // Sign the node
                    let signature = dummy_key.sign(&node_bytes);
                    
                    // Create a signed node
                    let signed_node = icn_types::dag::SignedDagNode {
                        node,
                        signature,
                        cid: None, // Will be computed when added to the DAG
                    };
                    
                    // Add to the DAG store
                    let manifest_cid = store.add_node(signed_node).await
                        .map_err(|e| anyhow::anyhow!("Failed to add node to DAG: {:?}", e))?;
                        
                    println!("Published node manifest with CID: {}", manifest_cid);
                    println!("Federation: {}", federation);
                    println!("DID: {}", did_str);
                    
                    // Simulate publishing to gossipsub - in a real implementation this would use libp2p
                    println!("Publishing manifest CID to gossipsub topic: mesh-capabilities");
                },
                "show" => {
                    match cid {
                        Some(ref manifest_cid) => {
                            // Parse CID
                            let cid_obj = parse_cid(manifest_cid)?;
                            
                            // Get the node from the DAG
                            let node = store.get_node(&cid_obj).await?;
                            
                            // Check if it's a manifest
                            if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                if payload.get("type").and_then(|t| t.as_array()).and_then(|a| a.iter().find(|t| t.as_str() == Some("NodeManifestCredential"))).is_some() {
                                    // This is a NodeManifest credential
                                    println!("Node Manifest (CID: {})", manifest_cid);
                                    println!("Author: {}", node.node.author);
                                    println!("Timestamp: {}", node.node.metadata.timestamp);
                                    
                                    // Extract and display capability details
                                    if let Some(subject) = payload.get("credentialSubject") {
                                        println!("\nCapabilities:");
                                        println!("  Architecture: {}", subject.get("architecture").and_then(|a| a.as_str()).unwrap_or("unknown"));
                                        println!("  Cores: {}", subject.get("cores").and_then(|c| c.as_u64()).unwrap_or(0));
                                        println!("  RAM: {} MB", subject.get("ramMb").and_then(|r| r.as_u64()).unwrap_or(0));
                                        println!("  Storage: {} bytes", subject.get("storageBytes").and_then(|s| s.as_u64()).unwrap_or(0));
                                        
                                        // GPU details if available
                                        if let Some(gpu) = subject.get("gpu") {
                                            if !gpu.is_null() {
                                                println!("\nGPU:");
                                                println!("  Model: {}", gpu.get("model").and_then(|m| m.as_str()).unwrap_or("unknown"));
                                                println!("  VRAM: {} MB", gpu.get("vram_mb").and_then(|v| v.as_u64()).unwrap_or(0));
                                                println!("  Cores: {}", gpu.get("cores").and_then(|c| c.as_u64()).unwrap_or(0));
                                                println!("  Tensor cores: {}", gpu.get("tensor_cores").and_then(|t| t.as_bool()).unwrap_or(false));
                                                
                                                // API support
                                                if let Some(apis) = gpu.get("api").and_then(|a| a.as_array()) {
                                                    let api_strings: Vec<String> = apis.iter()
                                                        .filter_map(|a| a.as_str().map(|s| s.to_string()))
                                                        .collect();
                                                    println!("  APIs: {}", api_strings.join(", "));
                                                }
                                                
                                                // Features
                                                if let Some(features) = gpu.get("features").and_then(|f| f.as_array()) {
                                                    let feature_strings: Vec<String> = features.iter()
                                                        .filter_map(|f| f.as_str().map(|s| s.to_string()))
                                                        .collect();
                                                    println!("  Features: {}", feature_strings.join(", "));
                                                }
                                            }
                                        }
                                        
                                        // Sensor details if available
                                        if let Some(sensors) = subject.get("sensors").and_then(|s| s.as_array()) {
                                            if !sensors.is_empty() {
                                                println!("\nSensors:");
                                                for (i, sensor) in sensors.iter().enumerate() {
                                                    println!("  {}. {} ({})", 
                                                        i + 1,
                                                        sensor.get("sensor_type").and_then(|t| t.as_str()).unwrap_or("unknown"),
                                                        sensor.get("model").and_then(|m| m.as_str()).unwrap_or("unknown model")
                                                    );
                                                    println!("     Protocol: {}", sensor.get("protocol").and_then(|p| p.as_str()).unwrap_or("unknown"));
                                                    println!("     Active: {}", sensor.get("active").and_then(|a| a.as_bool()).unwrap_or(false));
                                                }
                                            }
                                        }
                                        
                                        // Actuator details if available
                                        if let Some(actuators) = subject.get("actuators").and_then(|a| a.as_array()) {
                                            if !actuators.is_empty() {
                                                println!("\nActuators:");
                                                for (i, actuator) in actuators.iter().enumerate() {
                                                    println!("  {}. {} ({})", 
                                                        i + 1,
                                                        actuator.get("actuator_type").and_then(|t| t.as_str()).unwrap_or("unknown"),
                                                        actuator.get("model").and_then(|m| m.as_str()).unwrap_or("unknown model")
                                                    );
                                                    println!("     Protocol: {}", actuator.get("protocol").and_then(|p| p.as_str()).unwrap_or("unknown"));
                                                    println!("     Active: {}", actuator.get("active").and_then(|a| a.as_bool()).unwrap_or(false));
                                                }
                                            }
                                        }
                                        
                                        // Energy details
                                        if let Some(energy) = subject.get("energyProfile") {
                                            println!("\nEnergy Profile:");
                                            println!("  Renewable: {}%", energy.get("renewable_percentage").and_then(|r| r.as_u64()).unwrap_or(0));
                                            
                                            if let Some(battery) = energy.get("battery_percentage").and_then(|b| b.as_u64()) {
                                                println!("  Battery: {}%", battery);
                                                
                                                if let Some(charging) = energy.get("charging").and_then(|c| c.as_bool()) {
                                                    println!("  Charging: {}", charging);
                                                }
                                            }
                                            
                                            if let Some(power) = energy.get("power_consumption_watts").and_then(|p| p.as_f64()) {
                                                println!("  Power consumption: {:.2} watts", power);
                                            }
                                            
                                            // Energy sources
                                            if let Some(sources) = energy.get("source").and_then(|s| s.as_array()) {
                                                let source_strings: Vec<String> = sources.iter()
                                                    .filter_map(|s| s.as_str().map(|s| s.to_string()))
                                                    .collect();
                                                println!("  Sources: {}", source_strings.join(", "));
                                            }
                                        }
                                        
                                        // Mesh protocols
                                        if let Some(protocols) = subject.get("meshProtocols").and_then(|p| p.as_array()) {
                                            let protocol_strings: Vec<String> = protocols.iter()
                                                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                                                .collect();
                                            println!("\nMesh Protocols: {}", protocol_strings.join(", "));
                                        }
                                        
                                        println!("\nFirmware Hash: {}", subject.get("trustFirmwareHash").and_then(|h| h.as_str()).unwrap_or("unknown"));
                                    } else {
                                        println!("Warning: Missing credentialSubject in manifest");
                                    }
                                } else {
                                    println!("Warning: The specified CID does not refer to a NodeManifest");
                                    // Print the payload type for debugging
                                    if let Some(type_val) = payload.get("type") {
                                        println!("Payload type: {}", type_val);
                                    }
                                }
                            } else {
                                println!("Warning: The specified CID does not contain a JSON payload");
                            }
                        },
                        None => {
                            // List all manifests
                            println!("Showing all manifests for federation: {}", federation);
                            
                            // Get all nodes
                            let nodes = store.get_ordered_nodes().await?;
                            
                            // Filter for manifests in this federation
                            let manifests: Vec<_> = nodes.iter()
                                .filter(|node| node.node.federation_id == federation)
                                .filter(|node| {
                                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                        if let Some(types) = payload.get("type").and_then(|t| t.as_array()) {
                                            types.iter().any(|t| t.as_str() == Some("NodeManifestCredential"))
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                })
                                .collect();
                            
                            if manifests.is_empty() {
                                println!("No manifests found for federation: {}", federation);
                            } else {
                                println!("Found {} manifests:", manifests.len());
                                
                                for (i, node) in manifests.iter().enumerate() {
                                    let cid = node.cid.as_ref().unwrap();
                                    println!("{}. CID: {}", i + 1, cid);
                                    println!("   Author: {}", node.node.author);
                                    println!("   Timestamp: {}", node.node.metadata.timestamp);
                                    
                                    // Extract basic info
                                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                        if let Some(subject) = payload.get("credentialSubject") {
                                            let arch = subject.get("architecture").and_then(|a| a.as_str()).unwrap_or("unknown");
                                            let cores = subject.get("cores").and_then(|c| c.as_u64()).unwrap_or(0);
                                            let ram = subject.get("ramMb").and_then(|r| r.as_u64()).unwrap_or(0);
                                            
                                            println!("   Architecture: {}, Cores: {}, RAM: {} MB", arch, cores, ram);
                                            
                                            // GPU info if available
                                            if let Some(gpu) = subject.get("gpu") {
                                                if !gpu.is_null() {
                                                    let model = gpu.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
                                                    println!("   GPU: {}", model);
                                                }
                                            }
                                            
                                            // Number of sensors and actuators
                                            let sensor_count = subject.get("sensors").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0);
                                            let actuator_count = subject.get("actuators").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0);
                                            
                                            if sensor_count > 0 || actuator_count > 0 {
                                                println!("   Sensors: {}, Actuators: {}", sensor_count, actuator_count);
                                            }
                                            
                                            // Renewable energy percentage
                                            if let Some(energy) = subject.get("energyProfile") {
                                                let renewable = energy.get("renewable_percentage").and_then(|r| r.as_u64()).unwrap_or(0);
                                                println!("   Renewable Energy: {}%", renewable);
                                            }
                                        }
                                    }
                                    
                                    println!("");
                                }
                            }
                        }
                    }
                },
                "update" => {
                    // Both field and value must be provided
                    if field.is_none() || value.is_none() {
                        return Err(anyhow::anyhow!("Both --field and --value parameters are required for update action"));
                    }
                    
                    let field = field.unwrap();
                    let value_str = value.unwrap();
                    
                    // Parse the value string as JSON
                    let value_json: Value = serde_json::from_str(&value_str)
                        .context("Failed to parse value as JSON")?;
                    
                    // Create a manifest
                    let hash = firmware_hash.unwrap_or_else(|| "unknown-firmware-hash".to_string());
                    let mut manifest = icn_identity_core::manifest::NodeManifest::from_system(did.clone(), &hash)
                        .context("Failed to create manifest from system information")?;
                    
                    // Update the field
                    match field.as_str() {
                        "energy_profile.renewable_percentage" => {
                            if let Some(percentage) = value_json.as_u64() {
                                manifest.energy_profile.renewable_percentage = percentage.min(100) as u8;
                            } else {
                                return Err(anyhow::anyhow!("Renewable percentage must be a number"));
                            }
                        },
                        "energy_profile.battery_percentage" => {
                            if let Some(percentage) = value_json.as_u64() {
                                manifest.energy_profile.battery_percentage = Some(percentage.min(100) as u8);
                            } else if value_json.is_null() {
                                manifest.energy_profile.battery_percentage = None;
                            } else {
                                return Err(anyhow::anyhow!("Battery percentage must be a number or null"));
                            }
                        },
                        "energy_profile.charging" => {
                            if value_json.is_boolean() {
                                manifest.energy_profile.charging = value_json.as_bool();
                            } else {
                                return Err(anyhow::anyhow!("Charging must be a boolean"));
                            }
                        },
                        "energy_profile.power_consumption_watts" => {
                            if let Some(watts) = value_json.as_f64() {
                                manifest.energy_profile.power_consumption_watts = Some(watts);
                            } else if value_json.is_null() {
                                manifest.energy_profile.power_consumption_watts = None;
                            } else {
                                return Err(anyhow::anyhow!("Power consumption must be a number or null"));
                            }
                        },
                        "sensors" => {
                            if let Some(sensors) = value_json.as_array() {
                                let mut new_sensors = Vec::new();
                                
                                for sensor in sensors {
                                    if let Some(sensor_obj) = sensor.as_object() {
                                        if let (Some(sensor_type), Some(protocol)) = (
                                            sensor_obj.get("sensor_type").and_then(|s| s.as_str()),
                                            sensor_obj.get("protocol").and_then(|p| p.as_str())
                                        ) {
                                            new_sensors.push(icn_identity_core::manifest::SensorProfile {
                                                sensor_type: sensor_type.to_string(),
                                                model: sensor_obj.get("model").and_then(|m| m.as_str()).map(|s| s.to_string()),
                                                capabilities: sensor_obj.get("capabilities").cloned().unwrap_or(serde_json::json!({})),
                                                protocol: protocol.to_string(),
                                                active: sensor_obj.get("active").and_then(|a| a.as_bool()).unwrap_or(true),
                                            });
                                        }
                                    }
                                }
                                
                                manifest.sensors = new_sensors;
                            } else {
                                return Err(anyhow::anyhow!("Sensors must be an array"));
                            }
                        },
                        "actuators" => {
                            if let Some(actuators) = value_json.as_array() {
                                let mut new_actuators = Vec::new();
                                
                                for actuator in actuators {
                                    if let Some(actuator_obj) = actuator.as_object() {
                                        if let (Some(actuator_type), Some(protocol)) = (
                                            actuator_obj.get("actuator_type").and_then(|s| s.as_str()),
                                            actuator_obj.get("protocol").and_then(|p| p.as_str())
                                        ) {
                                            new_actuators.push(icn_identity_core::manifest::Actuator {
                                                actuator_type: actuator_type.to_string(),
                                                model: actuator_obj.get("model").and_then(|m| m.as_str()).map(|s| s.to_string()),
                                                capabilities: actuator_obj.get("capabilities").cloned().unwrap_or(serde_json::json!({})),
                                                protocol: protocol.to_string(),
                                                active: actuator_obj.get("active").and_then(|a| a.as_bool()).unwrap_or(true),
                                            });
                                        }
                                    }
                                }
                                
                                manifest.actuators = new_actuators;
                            } else {
                                return Err(anyhow::anyhow!("Actuators must be an array"));
                            }
                        },
                        "gpu" => {
                            if value_json.is_null() {
                                manifest.gpu = None;
                            } else if let Some(gpu_obj) = value_json.as_object() {
                                if let (Some(model), Some(vram_mb), Some(cores)) = (
                                    gpu_obj.get("model").and_then(|m| m.as_str()),
                                    gpu_obj.get("vram_mb").and_then(|v| v.as_u64()),
                                    gpu_obj.get("cores").and_then(|c| c.as_u64())
                                ) {
                                    // Parse APIs
                                    let mut apis = Vec::new();
                                    if let Some(api_array) = gpu_obj.get("api").and_then(|a| a.as_array()) {
                                        for api in api_array {
                                            if let Some(api_str) = api.as_str() {
                                                match api_str.to_lowercase().as_str() {
                                                    "cuda" => apis.push(icn_identity_core::manifest::GpuApi::Cuda),
                                                    "vulkan" => apis.push(icn_identity_core::manifest::GpuApi::Vulkan),
                                                    "metal" => apis.push(icn_identity_core::manifest::GpuApi::Metal),
                                                    "webgpu" => apis.push(icn_identity_core::manifest::GpuApi::WebGpu),
                                                    "opencl" => apis.push(icn_identity_core::manifest::GpuApi::OpenCl),
                                                    "directx" => apis.push(icn_identity_core::manifest::GpuApi::DirectX),
                                                    _ => apis.push(icn_identity_core::manifest::GpuApi::Other),
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Parse features
                                    let mut features = Vec::new();
                                    if let Some(feature_array) = gpu_obj.get("features").and_then(|f| f.as_array()) {
                                        for feature in feature_array {
                                            if let Some(feature_str) = feature.as_str() {
                                                features.push(feature_str.to_string());
                                            }
                                        }
                                    }
                                    
                                    manifest.gpu = Some(icn_identity_core::manifest::GpuProfile {
                                        model: model.to_string(),
                                        api: apis,
                                        vram_mb,
                                        cores: cores as u32,
                                        tensor_cores: gpu_obj.get("tensor_cores").and_then(|t| t.as_bool()).unwrap_or(false),
                                        features,
                                    });
                                } else {
                                    return Err(anyhow::anyhow!("GPU object must contain model, vram_mb, and cores fields"));
                                }
                            } else {
                                return Err(anyhow::anyhow!("GPU must be an object or null"));
                            }
                        },
                        "storage_bytes" => {
                            if let Some(bytes) = value_json.as_u64() {
                                manifest.storage_bytes = bytes;
                            } else {
                                return Err(anyhow::anyhow!("Storage bytes must be a number"));
                            }
                        },
                        "ram_mb" => {
                            if let Some(mb) = value_json.as_u64() {
                                manifest.ram_mb = mb as u32;
                            } else {
                                return Err(anyhow::anyhow!("RAM MB must be a number"));
                            }
                        },
                        "cores" => {
                            if let Some(cores) = value_json.as_u64() {
                                manifest.cores = cores as u16;
                            } else {
                                return Err(anyhow::anyhow!("Cores must be a number"));
                            }
                        },
                        "trust_fw_hash" => {
                            if let Some(hash) = value_json.as_str() {
                                manifest.trust_fw_hash = hash.to_string();
                            } else {
                                return Err(anyhow::anyhow!("Firmware hash must be a string"));
                            }
                        },
                        _ => {
                            return Err(anyhow::anyhow!("Unknown field: {}", field));
                        }
                    }
                    
                    // Update the timestamp
                    manifest.last_seen = chrono::Utc::now();
                    
                    // Sign the manifest
                    let manifest_json = serde_json::to_vec(&manifest)
                        .context("Failed to serialize manifest")?;
                    manifest.signature = dummy_key.sign(&manifest_json);
                    
                    // Convert to a verifiable credential
                    let manifest_vc = manifest.to_verifiable_credential();
                    
                    // Create a DAG node for the manifest
                    let node = icn_types::dag::DagNodeBuilder::new()
                        .with_payload(icn_types::dag::DagPayload::Json(manifest_vc))
                        .with_author(did.clone())
                        .with_federation_id(federation.clone())
                        .with_label("NodeManifest".to_string())
                        .build()
                        .context("Failed to build DAG node")?;
                        
                    // Serialize the node for signing
                    let node_bytes = serde_json::to_vec(&node)
                        .context("Failed to serialize node")?;
                    
                    // Sign the node
                    let signature = dummy_key.sign(&node_bytes);
                    
                    // Create a signed node
                    let signed_node = icn_types::dag::SignedDagNode {
                        node,
                        signature,
                        cid: None, // Will be computed when added to the DAG
                    };
                    
                    // Add to the DAG store
                    let manifest_cid = store.add_node(signed_node).await
                        .map_err(|e| anyhow::anyhow!("Failed to add node to DAG: {:?}", e))?;
                        
                    println!("Updated node manifest with CID: {}", manifest_cid);
                    println!("Updated field: {}", field);
                    println!("Federation: {}", federation);
                    println!("DID: {}", did_str);
                },
                "list" => {
                    // List all manifests for this federation
                    println!("Listing all manifests for federation: {}", federation);
                    
                    // Get all nodes
                    let nodes = store.get_ordered_nodes().await?;
                    
                    // Filter for manifests in this federation
                    let manifests: Vec<_> = nodes.iter()
                        .filter(|node| node.node.federation_id == federation)
                        .filter(|node| {
                            if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                if let Some(types) = payload.get("type").and_then(|t| t.as_array()) {
                                    types.iter().any(|t| t.as_str() == Some("NodeManifestCredential"))
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                        .collect();
                    
                    if manifests.is_empty() {
                        println!("No manifests found for federation: {}", federation);
                    } else {
                        println!("Found {} manifests:", manifests.len());
                        
                        for (i, node) in manifests.iter().enumerate() {
                            let cid = node.cid.as_ref().unwrap();
                            println!("{}. CID: {}", i + 1, cid);
                            println!("   Author: {}", node.node.author);
                            println!("   Timestamp: {}", node.node.metadata.timestamp);
                            
                            // Extract basic info
                            if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                if let Some(subject) = payload.get("credentialSubject") {
                                    let arch = subject.get("architecture").and_then(|a| a.as_str()).unwrap_or("unknown");
                                    let cores = subject.get("cores").and_then(|c| c.as_u64()).unwrap_or(0);
                                    let ram = subject.get("ramMb").and_then(|r| r.as_u64()).unwrap_or(0);
                                    
                                    println!("   Architecture: {}, Cores: {}, RAM: {} MB", arch, cores, ram);
                                    
                                    // GPU info if available
                                    if let Some(gpu) = subject.get("gpu") {
                                        if !gpu.is_null() {
                                            let model = gpu.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
                                            println!("   GPU: {}", model);
                                        }
                                    }
                                    
                                    // Number of sensors and actuators
                                    let sensor_count = subject.get("sensors").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0);
                                    let actuator_count = subject.get("actuators").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0);
                                    
                                    if sensor_count > 0 || actuator_count > 0 {
                                        println!("   Sensors: {}, Actuators: {}", sensor_count, actuator_count);
                                    }
                                    
                                    // Renewable energy percentage
                                    if let Some(energy) = subject.get("energyProfile") {
                                        let renewable = energy.get("renewable_percentage").and_then(|r| r.as_u64()).unwrap_or(0);
                                        println!("   Renewable Energy: {}%", renewable);
                                    }
                                }
                            }
                            
                            println!("");
                        }
                    }
                },
                _ => {
                    return Err(anyhow::anyhow!("Unknown action: {}. Allowed actions: create, publish, show, update, list", action));
                }
            }
            
            Ok(())
        },
        MeshCommands::Audit { 
            audit_type,
            cid,
            scheduler,
            task,
            requirement,
            limit,
            dag_dir,
            federation,
            verify,
            trusted_dids_path,
            export_results
        } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Get all nodes
            let all_nodes = store.get_ordered_nodes().await?;
            
            // Filter for dispatch audit records in this federation
            let mut dispatch_records = Vec::new();
            let mut total_records = 0;
            
            for node in all_nodes {
                if node.node.federation_id != *federation {
                    continue;
                }
                
                if let Some(cid_ref) = &node.cid {
                    let cid_str = cid_ref.to_string();
                    
                    // Check if we're looking for a specific CID
                    if let Some(target_cid) = cid {
                        if cid_str != *target_cid {
                            continue;
                        }
                    }
                    
                    // Check payload type
                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                        let record_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        
                        match audit_type.as_str() {
                            "dispatch" => {
                                // Filter for dispatch records
                                if record_type == "DispatchAuditRecord" {
                                    // Apply task filter if specified
                                    if let Some(task_cid) = task {
                                        if let Some(t) = payload.get("task_cid").and_then(|t| t.as_str()) {
                                            if t != task_cid {
                                                continue;
                                            }
                                        }
                                    }
                                    
                                    // Apply scheduler filter if specified
                                    if let Some(scheduler_did) = scheduler {
                                        if let Some(s) = payload.get("scheduler").and_then(|s| s.as_str()) {
                                            if s != scheduler_did {
                                                continue;
                                            }
                                        }
                                    }
                                    
                                    // Add to our list
                                    dispatch_records.push((cid_ref.clone(), node.node, payload.clone()));
                                    total_records += 1;
                                }
                            },
                            "manifest" => {
                                // Filter for node manifest records
                                if record_type == "NodeManifest" {
                                    // You can implement additional filters for manifests here
                                    total_records += 1;
                                }
                            },
                            "requirements" => {
                                // Filter for capability requirement records
                                if record_type == "CapabilityRequirement" || record_type == "TaskTicket" {
                                    // You can implement additional filters for capability requirements here
                                    total_records += 1;
                                }
                            },
                            _ => {
                                return Err(anyhow::anyhow!("Unknown audit type: {}. Expected 'dispatch', 'manifest', or 'requirements'", audit_type));
                            }
                        }
                    }
                }
                
                // Break if we've hit our limit
                if *limit > 0 && total_records >= *limit {
                    break;
                }
            }
            
            // If doing verification, load the trusted DIDs policy
            let mut policy_opt = None;
            if *verify {
                if let Some(policy_path) = trusted_dids_path {
                    println!("Loading trusted DIDs policy from: {}", policy_path.display());
                    let factory = planetary_mesh::trusted_did_policy::TrustPolicyFactory::new();
                    match factory.from_file(policy_path) {
                        Ok(policy) => {
                            println!("Loaded trusted DIDs policy for federation: {}", federation);
                            policy_opt = Some(policy);
                        },
                        Err(e) => {
                            return Err(anyhow::anyhow!("Failed to load trusted DIDs policy: {}", e));
                        }
                    }
                } else {
                    println!("Warning: --verify specified but no --trusted-dids-path provided");
                    println!("Verification will check signatures but not trusted DIDs");
                }
            }
            
            // Get verification results
            let mut verification_results = Vec::new();
            
            // Process the dispatch records
            match audit_type.as_str() {
                "dispatch" => {
                    println!("\nDispatch Audit Records for Federation: {}", federation);
                    println!("{} records found", dispatch_records.len());
                    
                    if dispatch_records.is_empty() {
                        println!("No dispatch records found matching the criteria");
                    } else {
                        for (idx, (record_cid, node, payload)) in dispatch_records.iter().enumerate() {
                            println!("\n{}: {}", idx + 1, record_cid);
                            
                            // Print basic info
                            let scheduler = payload.get("scheduler").and_then(|s| s.as_str()).unwrap_or("unknown");
                            let task_cid = payload.get("task_cid").and_then(|t| t.as_str()).unwrap_or("unknown");
                            let task_type = payload.get("task_type").and_then(|t| t.as_str()).unwrap_or("unknown");
                            let timestamp = payload.get("timestamp").and_then(|t| t.as_str()).unwrap_or("unknown");
                            
                            println!("  Scheduler: {}", scheduler);
                            println!("  Task CID: {}", task_cid);
                            println!("  Task Type: {}", task_type);
                            println!("  Timestamp: {}", timestamp);
                            
                            // Show capabilities if available
                            if let Some(capabilities) = payload.get("capabilities") {
                                println!("  Capability Requirements:");
                                if let Some(cap_obj) = capabilities.as_object() {
                                    for (k, v) in cap_obj {
                                        println!("    {} = {}", k, v);
                                    }
                                }
                            }
                            
                            // Show selected node
                            if let Some(selected) = payload.get("selected_node").and_then(|s| s.as_str()) {
                                println!("  Selected Node: {}", selected);
                            }
                            
                            // Print score for selected node
                            if let Some(score) = payload.get("score").and_then(|s| s.as_f64()) {
                                println!("  Score: {:.4}", score);
                            }
                            
                            // Perform verification if requested
                            if *verify {
                                if let Some(credential) = payload.get("credential") {
                                    println!("  Verifying dispatch credential...");
                                    
                                    // Parse the credential
                                    match serde_json::from_value::<planetary_mesh::dispatch_credential::DispatchCredential>(credential.clone()) {
                                        Ok(cred) => {
                                            // Verify the credential signature
                                            match cred.verify() {
                                                Ok(status) => {
                                                    match status {
                                                        planetary_mesh::dispatch_credential::VerificationStatus::Valid => {
                                                            println!("  ✓ Signature: Valid");
                                                            
                                                            // Verify against DAG record
                                                            match cred.verify_against_dag(&store, record_cid).await {
                                                                Ok(dag_status) => {
                                                                    match dag_status {
                                                                        planetary_mesh::dispatch_credential::VerificationStatus::MatchesDag => {
                                                                            println!("  ✓ DAG Match: Valid");
                                                                            
                                                                            // Check if the issuer is trusted
                                                                            let issuer_did = Did::from(cred.issuer.clone());
                                                                            let mut is_trusted = true;
                                                                            
                                                                            if let Some(policy) = &policy_opt {
                                                                                if policy.is_trusted_for(&issuer_did, planetary_mesh::trusted_did_policy::TrustLevel::Full) {
                                                                                    println!("  ✓ Issuer Trust: Trusted");
                                                                                } else {
                                                                                    println!("  ✗ Issuer Trust: Not trusted");
                                                                                    is_trusted = false;
                                                                                }
                                                                                
                                                                                // Check if selected node is trusted
                                                                                let selected_node_did = Did::from(cred.credentialSubject.selectedNode.clone());
                                                                                if policy.is_trusted_for(&selected_node_did, planetary_mesh::trusted_did_policy::TrustLevel::Worker) {
                                                                                    println!("  ✓ Worker Trust: Trusted");
                                                                                } else {
                                                                                    println!("  ✗ Worker Trust: Not trusted");
                                                                                    is_trusted = false;
                                                                                }
                                                                                
                                                                                // Check if requestor is trusted
                                                                                let requestor_did = Did::from(cred.credentialSubject.id.clone());
                                                                                if policy.is_trusted_for(&requestor_did, planetary_mesh::trusted_did_policy::TrustLevel::Requestor) {
                                                                                    println!("  ✓ Requestor Trust: Trusted");
                                                                                } else {
                                                                                    println!("  ✗ Requestor Trust: Not trusted");
                                                                                    is_trusted = false;
                                                                                }
                                                                            } else {
                                                                                println!("  ! Trust Policy: Not checked (no policy provided)");
                                                                            }
                                                                            
                                                                            // Add to verification results
                                                                            verification_results.push(serde_json::json!({
                                                                                "cid": record_cid.to_string(),
                                                                                "issuer": cred.issuer,
                                                                                "task_cid": task_cid,
                                                                                "selected_node": cred.credentialSubject.selectedNode,
                                                                                "requestor": cred.credentialSubject.id,
                                                                                "signature_valid": true,
                                                                                "dag_match": true,
                                                                                "trusted": is_trusted,
                                                                                "timestamp": timestamp
                                                                            }));
                                                                        },
                                                                        _ => {
                                                                            println!("  ✗ DAG Match: Invalid (credential doesn't match DAG record)");
                                                                            
                                                                            // Add to verification results
                                                                            verification_results.push(serde_json::json!({
                                                                                "cid": record_cid.to_string(),
                                                                                "issuer": cred.issuer,
                                                                                "task_cid": task_cid,
                                                                                "selected_node": cred.credentialSubject.selectedNode,
                                                                                "requestor": cred.credentialSubject.id,
                                                                                "signature_valid": true,
                                                                                "dag_match": false,
                                                                                "trusted": false,
                                                                                "timestamp": timestamp
                                                                            }));
                                                                        }
                                                                    }
                                                                },
                                                                Err(e) => {
                                                                    println!("  ✗ DAG Verification Error: {}", e);
                                                                    
                                                                    // Add to verification results
                                                                    verification_results.push(serde_json::json!({
                                                                        "cid": record_cid.to_string(),
                                                                        "issuer": cred.issuer,
                                                                        "task_cid": task_cid,
                                                                        "error": format!("DAG verification error: {}", e),
                                                                        "signature_valid": true,
                                                                        "dag_match": false,
                                                                        "trusted": false,
                                                                        "timestamp": timestamp
                                                                    }));
                                                                }
                                                            }
                                                        },
                                                        planetary_mesh::dispatch_credential::VerificationStatus::Unsigned => {
                                                            println!("  ✗ Signature: Missing (unsigned credential)");
                                                            
                                                            // Add to verification results
                                                            verification_results.push(serde_json::json!({
                                                                "cid": record_cid.to_string(),
                                                                "task_cid": task_cid,
                                                                "error": "Unsigned credential",
                                                                "signature_valid": false,
                                                                "dag_match": false,
                                                                "trusted": false,
                                                                "timestamp": timestamp
                                                            }));
                                                        },
                                                        planetary_mesh::dispatch_credential::VerificationStatus::Invalid => {
                                                            println!("  ✗ Signature: Invalid");
                                                            
                                                            // Add to verification results
                                                            verification_results.push(serde_json::json!({
                                                                "cid": record_cid.to_string(),
                                                                "issuer": cred.issuer,
                                                                "task_cid": task_cid,
                                                                "selected_node": cred.credentialSubject.selectedNode,
                                                                "requestor": cred.credentialSubject.id,
                                                                "error": "Invalid signature",
                                                                "signature_valid": false,
                                                                "dag_match": false,
                                                                "trusted": false,
                                                                "timestamp": timestamp
                                                            }));
                                                        },
                                                        _ => {
                                                            println!("  ? Signature: Unknown verification status");
                                                            
                                                            // Add to verification results
                                                            verification_results.push(serde_json::json!({
                                                                "cid": record_cid.to_string(),
                                                                "task_cid": task_cid,
                                                                "error": "Unknown verification status",
                                                                "signature_valid": false,
                                                                "dag_match": false,
                                                                "trusted": false,
                                                                "timestamp": timestamp
                                                            }));
                                                        }
                                                    }
                                                },
                                                Err(e) => {
                                                    println!("  ✗ Verification Error: {}", e);
                                                    
                                                    // Add to verification results
                                                    verification_results.push(serde_json::json!({
                                                        "cid": record_cid.to_string(),
                                                        "task_cid": task_cid,
                                                        "error": format!("Verification error: {}", e),
                                                        "signature_valid": false,
                                                        "dag_match": false,
                                                        "trusted": false,
                                                        "timestamp": timestamp
                                                    }));
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            println!("  ✗ Credential Parse Error: {}", e);
                                            
                                            // Add to verification results
                                            verification_results.push(serde_json::json!({
                                                "cid": record_cid.to_string(),
                                                "task_cid": task_cid,
                                                "error": format!("Credential parse error: {}", e),
                                                "signature_valid": false,
                                                "dag_match": false,
                                                "trusted": false,
                                                "timestamp": timestamp
                                            }));
                                        }
                                    }
                                } else {
                                    println!("  ✗ No credential found in dispatch record");
                                    
                                    // Add to verification results
                                    verification_results.push(serde_json::json!({
                                        "cid": record_cid.to_string(),
                                        "task_cid": task_cid,
                                        "error": "No credential found in dispatch record",
                                        "signature_valid": false,
                                        "dag_match": false,
                                        "trusted": false,
                                        "timestamp": timestamp
                                    }));
                                }
                            }
                        }
                    }
                    
                    // Export verification results if requested
                    if *verify && verification_results.len() > 0 {
                        if let Some(export_path) = export_results {
                            let export_data = serde_json::json!({
                                "federation": federation,
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "verification_results": verification_results
                            });
                            
                            fs::write(export_path, serde_json::to_string_pretty(&export_data)?)
                                .context("Failed to write verification results file")?;
                                
                            println!("\nVerification results exported to: {}", export_path.display());
                        }
                        
                        // Show summary
                        let valid_count = verification_results.iter()
                            .filter(|r| r["signature_valid"].as_bool().unwrap_or(false) && 
                                   r["dag_match"].as_bool().unwrap_or(false) && 
                                   r["trusted"].as_bool().unwrap_or(false))
                            .count();
                            
                        println!("\nVerification Summary:");
                        println!("  Total Credentials: {}", verification_results.len());
                        println!("  Valid & Trusted: {}", valid_count);
                        println!("  Invalid or Untrusted: {}", verification_results.len() - valid_count);
                    }
                },
                "manifest" => {
                    println!("\nNode Manifest Audit for Federation: {}", federation);
                    println!("Feature not fully implemented yet");
                    // TODO: Implement manifest audit display similar to dispatch
                },
                "requirements" => {
                    println!("\nCapability Requirements Audit for Federation: {}", federation);
                    println!("Feature not fully implemented yet");
                    // TODO: Implement requirements audit display
                },
                _ => {
                    return Err(anyhow::anyhow!("Unknown audit type: {}. Expected 'dispatch', 'manifest', or 'requirements'", audit_type));
                }
            }
            
            Ok(())
        },
    }
}

/// Handler for the p2p-based DAG sync commands
async fn handle_dag_sync_command(cmd: &DagSyncCommands) -> Result<()> {
    match cmd {
        DagSyncCommands::Connect { peer, federation, dag_dir } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Create a federation peer
            let federation_peer = icn_types::dag::FederationPeer {
                id: peer.clone(),
                endpoint: peer.clone(),
                federation_id: federation.clone(),
                metadata: None,
            };
            
            // Create a sync service
            let mut sync_service = icn_types::dag::sync::memory::MemoryDAGSyncService::new(
                dag_store,
                federation.clone(),
                "local-peer".to_string(), // In a real implementation, we'd use a persistent peer ID
            );
            
            // Add the peer to the service
            sync_service.add_peer(federation_peer.clone(), 50);
            
            // Sync with the peer
            println!("Synchronizing with peer {} at {}", peer, peer);
            println!("Federation: {}", federation);
            println!("Trust level: 50");
            
            match sync_service.sync_with_peer(&federation_peer).await {
                Ok(result) => {
                    println!("\nSync result:");
                    println!("  Valid: {}", result.is_valid);
                    println!("  Accepted nodes: {}", result.accepted_nodes.len());
                    println!("  Rejected nodes: {}", result.rejected_nodes.len());
                    println!("\nDetailed report:");
                    println!("{}", result.report);
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Sync failed: {}", e))
                }
            }
        },
        DagSyncCommands::AutoSync { federation, dag_dir, mdns, kad_dht, bootstrap_peers, authorized_dids, min_quorum } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Create a sync service
            let mut sync_service = icn_types::dag::sync::memory::MemoryDAGSyncService::new(
                dag_store,
                federation.clone(),
                "local-peer".to_string(), // In a real implementation, we'd use a persistent peer ID
            );
            
            // Add the peers to the service
            if let Some(peers) = bootstrap_peers {
                for peer in peers {
                    sync_service.add_peer(icn_types::dag::FederationPeer {
                        id: peer.clone(),
                        endpoint: peer.clone(),
                        federation_id: federation.clone(),
                        metadata: None,
                    }, 50);
                }
            }
            
            if let Some(dids) = authorized_dids {
                for did in dids {
                    sync_service.add_peer(icn_types::dag::FederationPeer {
                        id: did.clone(),
                        endpoint: did.clone(),
                        federation_id: federation.clone(),
                        metadata: None,
                    }, 50);
                }
            }
            
            // Set minimum quorum
            sync_service.set_min_quorum(*min_quorum);
            
            // Start auto-sync
            println!("Starting auto-sync with federation: {}", federation);
            println!("Trust level: 50");
            println!("Minimum quorum: {}", min_quorum);
            
            if *mdns {
                println!("mDNS discovery enabled");
            }
            
            if *kad_dht {
                println!("Kademlia DHT discovery enabled");
            }
            
            if let Some(peers) = bootstrap_peers {
                println!("Bootstrap peers: {}", peers.join(", "));
            }
            
            if let Some(dids) = authorized_dids {
                println!("Authorized DIDs: {}", dids.join(", "));
            }
            
            match sync_service.start_auto_sync().await {
                Ok(result) => {
                    println!("\nAuto-sync result:");
                    println!("  Valid: {}", result.is_valid);
                    println!("  Accepted nodes: {}", result.accepted_nodes.len());
                    println!("  Rejected nodes: {}", result.rejected_nodes.len());
                    println!("\nDetailed report:");
                    println!("{}", result.report);
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Auto-sync failed: {}", e))
                }
            }
        },
        DagSyncCommands::Offer { peer, federation, dag_dir, max_nodes } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Create a federation peer
            let federation_peer = icn_types::dag::FederationPeer {
                id: peer.clone(),
                endpoint: peer.clone(),
                federation_id: federation.clone(),
                metadata: None,
            };
            
            // Create a sync service
            let mut sync_service = icn_types::dag::sync::memory::MemoryDAGSyncService::new(
                dag_store,
                federation.clone(),
                "local-peer".to_string(), // In a real implementation, we'd use a persistent peer ID
            );
            
            // Add the peer to the service
            sync_service.add_peer(federation_peer.clone(), 50);
            
            // Offer nodes to the peer
            println!("Offering {} nodes to peer {} at {}", max_nodes, peer, peer);
            println!("Federation: {}", federation);
            println!("Trust level: 50");
            
            match sync_service.offer_nodes(&federation_peer, max_nodes).await {
                Ok(result) => {
                    println!("\nOffer result:");
                    println!("  Valid: {}", result.is_valid);
                    println!("  Accepted nodes: {}", result.accepted_nodes.len());
                    println!("  Rejected nodes: {}", result.rejected_nodes.len());
                    println!("\nDetailed report:");
                    println!("{}", result.report);
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Offer failed: {}", e))
                }
            }
        },
        DagSyncCommands::Genesis { federation, dag_dir, key, policy_id, founding_dids, start_node, listen_addr, mdns } => {
            // Create DAG store
            let dag_store = open_dag_store(dag_dir).await?;
            
            // Load the key file
            let key_data = fs::read_to_string(key)
                .context("Failed to read key file")?;
            let key_json: Value = serde_json::from_str(&key_data)
                .context("Failed to parse key file as JSON")?;
            let did_str = key_json["did"].as_str()
                .context("Key file missing 'did' field")?;
            
            // Get the DID from the key file
            let did = Did::from(did_str.to_string());
            
            // Create a new manifest
            let manifest = icn_identity_core::manifest::NodeManifest::from_system(did.clone(), "unknown-firmware-hash")
                .context("Failed to create manifest from system information")?;
            
            // Create a sync service
            let mut sync_service = icn_types::dag::sync::memory::MemoryDAGSyncService::new(
                dag_store,
                federation.clone(),
                "local-peer".to_string(), // In a real implementation, we'd use a persistent peer ID
            );
            
            // Add the peers to the service
            if *start_node {
                sync_service.add_peer(icn_types::dag::FederationPeer {
                    id: "local-peer".to_string(),
                    endpoint: listen_addr.clone(),
                    federation_id: federation.clone(),
                    metadata: None,
                }, 50);
            }
            
            // Add founding_dids as peers
            let founding_dids = founding_dids.clone();
            for peer in &founding_dids {
                sync_service.add_peer(icn_types::dag::FederationPeer {
                    id: peer.clone(),
                    endpoint: peer.clone(),
                    federation_id: federation.clone(),
                    metadata: None,
                }, 50);
            }
            
            // Set minimum quorum
            sync_service.set_min_quorum(founding_dids.len() + 1);
            
            // Start the genesis DAG state
            println!("Creating genesis DAG state for federation: {}", federation);
            println!("Trust level: 50");
            println!("Minimum quorum: {}", founding_dids.len() + 1);
            
            if *mdns {
                println!("mDNS discovery enabled");
            }
            
            println!("Founding members: {}", founding_dids.join(", "));
            
            match sync_service.create_genesis_dag_state().await {
                Ok(result) => {
                    println!("\nGenesis DAG state created successfully!");
                    println!("State CID: {}", result.state_cid);
                    println!("Policy ID: {}", result.policy_id);
                    
                    // Create a transport config for publishing to peers
                    let transport_config = TransportConfig {
                        peer_id: uuid::Uuid::new_v4().to_string(),
                        federation_id: federation.clone(),
                        local_did: Some(did.clone()),
                        listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                        bootstrap_peers: vec![],
                        enable_mdns: *mdns,
                        enable_kad_dht: false,
                        max_message_size: 1024 * 1024, // 1MB
                        request_timeout: 30, // 30 seconds
                    };
                    
                    // Create the transport
                    let transport = Libp2pDagTransport::new(transport_config).await?;
                    
                    // Create the sync service
                    let sync_service = NetworkDagSyncService::new(
                        transport,
                        Arc::new(dag_store),
                        federation.clone(),
                        Some(did.clone()),
                    );
                    
                    // Start background sync to publish the genesis state
                    println!("Publishing genesis state to peers...");
                    sync_service.start_background_sync().await?;
                    
                    // Wait a bit for publication
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    
                    println!("Genesis state published successfully.");
                    
                    // Record metrics for genesis state creation
                    if let Ok(metrics_context) = init_metrics(&federation, None) {
                        metrics_context.record_genesis_state_created(
                            &result.state_cid.to_string(),
                            &result.policy_id,
                            result.signatures.len() as u64,
                            result.previous_anchors.len() as u64
                        );
                    }
                    
                    Ok(())
                },
                Err(e) => {
                    Err(anyhow::anyhow!("Failed to create genesis DAG state: {}", e))
                }
            }
        },
        DagSyncCommands::Visualize { dag_dir, output, thread_did, max_nodes } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Get DAG nodes
            let nodes = if let Some(did_str) = thread_did {
                // If filtering by author, get nodes by author
                let did = Did::from(did_str.clone());
                store.get_nodes_by_author(&did).await?
            } else {
                // Otherwise get all ordered nodes
                store.get_ordered_nodes().await?
            };
            
            // Limit to max_nodes
            let nodes = if nodes.len() > *max_nodes {
                println!("Limiting to {} nodes out of {}", max_nodes, nodes.len());
                nodes.into_iter().take(*max_nodes).collect()
            } else {
                nodes
            };
            
            // Generate DOT format for graph visualization
            let mut dot = String::new();
            dot.push_str("digraph DAG {\n");
            dot.push_str("  rankdir=LR;\n");
            dot.push_str("  node [shape=box style=filled];\n\n");
            
            // Add nodes
            for node in &nodes {
                let node_id = node.cid.as_ref().unwrap().to_string();
                let short_id = &node_id[0..10]; // Use shortened CID for readability
                
                let label = match &node.node.payload {
                    icn_types::dag::DagPayload::Json(value) => {
                        if let Some(type_val) = value.get("type") {
                            if let Some(type_str) = type_val.as_str() {
                                type_str.to_string()
                            } else {
                                "JSON".to_string()
                            }
                        } else {
                            "JSON".to_string()
                        }
                    },
                    icn_types::dag::DagPayload::Raw(_) => "Raw".to_string(),
                    icn_types::dag::DagPayload::Reference(_) => "Reference".to_string(),
                    icn_types::dag::DagPayload::TrustBundle(_) => "TrustBundle".to_string(),
                    icn_types::dag::DagPayload::ExecutionReceipt(_) => "Receipt".to_string(),
                };
                
                // Customize node color by payload type
                let color = match &node.node.payload {
                    icn_types::dag::DagPayload::TrustBundle(_) => "lightblue",
                    icn_types::dag::DagPayload::ExecutionReceipt(_) => "lightgreen",
                    icn_types::dag::DagPayload::Json(_) => "lightyellow",
                    _ => "white",
                };
                
                // Create node with metadata
                dot.push_str(&format!(
                    "  \"{}\" [label=\"{} ({}...)\\nAuthor: {}...\\nTime: {}\", fillcolor=\"{}\"];\n",
                    node_id,
                    label,
                    short_id,
                    node.node.author.to_string()[0..15],
                    node.node.metadata.timestamp.format("%Y-%m-%d %H:%M"),
                    color
                ));
                
                // Add edges from this node to its parents
                for parent in &node.node.parents {
                    dot.push_str(&format!("  \"{}\" -> \"{}\";\n", node_id, parent.to_string()));
                }
            }
            
            dot.push_str("}\n");
            
            // Write to output file
            fs::write(output, dot)?;
            
            println!("DAG visualization generated successfully!");
            println!("Saved to: {}", output.display());
            println!("Number of nodes: {}", nodes.len());
            println!("Generate an image with: dot -Tpng {} -o dag.png", output.display());
            
            Ok(())
        },
    }
}
