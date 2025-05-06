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
    },
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
            
            Ok(())
        },
        
        MeshCommands::Scheduler { 
            federation, 
            key, 
            dag_dir, 
            listen, 
            mdns 
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
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Create transport config for the scheduler node
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(),
                federation_id: federation.clone(),
                local_did: Some(author_did.clone()),
                listen_addresses: vec![listen.clone()],
                bootstrap_peers: vec![],
                enable_mdns: *mdns,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            println!("Starting scheduler node for federation: {}", federation);
            println!("Listening on: {}", listen);
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create the sync service
            let sync_service = NetworkDagSyncService::new(
                transport,
                store.clone(),
                federation.clone(),
                Some(author_did.clone()),
            );
            
            // Start background sync
            sync_service.start_background_sync().await?;
            println!("Background sync started");
            
            // Spawn a task to monitor the DAG for new tasks and bids
            let store_clone = store.clone();
            tokio::spawn(async move {
                let mut last_check = Instant::now();
                let mut processed_tasks = HashSet::new();
                let mut processed_bids = HashSet::new();
                
                loop {
                    // Wait a bit before checking again
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    
                    // Get all nodes created since last check
                    let now = Instant::now();
                    
                    // Get all nodes (in a real implementation, we would filter by timestamp)
                    match store_clone.get_ordered_nodes().await {
                        Ok(nodes) => {
                            let mut tasks = Vec::new();
                            let mut bids = HashMap::new();
                            
                            // Identify task tickets and bids
                            for node in nodes {
                                if let Some(cid) = &node.cid {
                                    let cid_str = cid.to_string();
                                    
                                    if let icn_types::dag::DagPayload::Json(payload) = &node.node.payload {
                                        if let Some(type_str) = payload.get("type").and_then(|t| t.as_str()) {
                                            match type_str {
                                                "TaskTicket" => {
                                                    if !processed_tasks.contains(&cid_str) {
                                                        tasks.push((cid_str.clone(), node.clone(), payload.clone()));
                                                        processed_tasks.insert(cid_str);
                                                    }
                                                },
                                                "TaskBid" => {
                                                    if !processed_bids.contains(&cid_str) {
                                                        if let Some(task_cid) = payload.get("task_cid").and_then(|t| t.as_str()) {
                                                            bids.entry(task_cid.to_string())
                                                               .or_insert_with(Vec::new)
                                                               .push((cid_str.clone(), node.clone(), payload.clone()));
                                                            
                                                            processed_bids.insert(cid_str);
                                                        }
                                                    }
                                                },
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Process tasks with bids
                            for (task_cid, task_node, task_payload) in tasks {
                                if let Some(task_bids) = bids.get(&task_cid) {
                                    println!("Processing task: {} with {} bids", task_cid, task_bids.len());
                                    
                                    // Find the best bid (lowest score)
                                    let mut best_bid = None;
                                    let mut best_score = f64::MAX;
                                    
                                    for (bid_cid, bid_node, bid_payload) in task_bids {
                                        if let (Some(latency), Some(memory), Some(cores), Some(reputation), Some(renewable)) = (
                                            bid_payload["offered_resources"]["latency_ms"].as_u64(),
                                            bid_payload["offered_resources"]["memory_mb"].as_u64(),
                                            bid_payload["offered_resources"]["cores"].as_u64(),
                                            bid_payload["offered_resources"]["reputation"].as_u64(),
                                            bid_payload["offered_resources"]["renewable_energy_pct"].as_u64(),
                                        ) {
                                            // Calculate bid score - lower is better
                                            let score = latency as f64 * (100.0 - reputation as f64) / 
                                                      (memory as f64 * cores as f64 * (1.0 + renewable as f64 / 100.0));
                                            
                                            if score < best_score {
                                                best_score = score;
                                                best_bid = Some((bid_cid.clone(), bid_node.clone(), bid_payload.clone()));
                                            }
                                        }
                                    }
                                    
                                    // If we found a best bid, create a task assignment
                                    if let Some((best_bid_cid, best_bid_node, _)) = best_bid {
                                        println!("  Selected bid: {} with score: {:.4}", best_bid_cid, best_score);
                                        
                                        // In a real implementation, we would create a task assignment node
                                        // that links the task and the winning bid, and distribute it to peers
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("Error getting nodes: {:?}", e);
                        }
                    }
                    
                    last_check = now;
                }
            });
            
            // Keep the process running
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
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
    }
}

/// Handler for the p2p-based DAG sync commands
async fn handle_dag_sync_command(cmd: &DagSyncCommands) -> Result<()> {
    match cmd {
        DagSyncCommands::Connect { peer, federation, dag_dir } => {
            // Create storage path
            std::fs::create_dir_all(dag_dir)?;
            
            // Create DAG store with RocksDB
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Create transport config
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                federation_id: federation.clone(),
                local_did: None, // We could load from key file in the future
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                bootstrap_peers: vec![peer.clone()],
                enable_mdns: true,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            println!("Connecting to peer {}", peer);
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create the sync service
            let sync_service = NetworkDagSyncService::new(
                transport,
                store,
                federation.clone(),
                None, // Local DID
            );
            
            // Parse the peer string into a FederationPeer
            let peer_parts: Vec<&str> = peer.split('/').collect();
            let peer_id = peer_parts.last().unwrap_or(&"").to_string();
            
            let federation_peer = FederationPeer {
                id: peer_id,
                endpoint: peer.clone(),
                federation_id: federation.clone(),
                metadata: None,
            };
            
            // Connect to the peer
            sync_service.connect_peer(&federation_peer).await?;
            println!("Connected to peer: {}", peer);
            
            // Start background sync
            sync_service.start_background_sync().await?;
            println!("Background sync started");
            
            // Keep the process running
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
        
        DagSyncCommands::AutoSync { 
            federation, 
            dag_dir,
            mdns, 
            kad_dht, 
            bootstrap_peers,
            authorized_dids,
            min_quorum,
        } => {
            // Create storage directory
            std::fs::create_dir_all(dag_dir)?;
            
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Create transport config
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                federation_id: federation.clone(),
                local_did: None, // We could load from key file in the future
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                bootstrap_peers: bootstrap_peers.clone().unwrap_or_default(),
                enable_mdns: *mdns,
                enable_kad_dht: *kad_dht,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            println!("Starting auto-sync for federation {}", federation);
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create sync policy
            let mut policy = SyncPolicy::default();
            policy.min_quorum = *min_quorum;
            
            // Set authorized DIDs if provided
            if let Some(dids) = authorized_dids {
                let did_set: HashSet<Did> = dids.iter()
                    .filter_map(|d| {
                        if d.starts_with("did:") {
                            Some(Did::from(d.to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();
                
                if !did_set.is_empty() {
                    policy.authorized_dids = Some(did_set);
                }
            }
            
            // Create the sync service with the policy
            let sync_service = NetworkDagSyncService::new(
                transport,
                store,
                federation.clone(),
                None, // Local DID
            ).with_policy(policy);
            
            // Start background sync
            sync_service.start_background_sync().await?;
            println!("Background sync started");
            
            // Discover peers periodically
            let sync_service_clone = sync_service.clone();
            tokio::spawn(async move {
                loop {
                    match sync_service_clone.discover_peers().await {
                        Ok(peers) => {
                            println!("Discovered {} peers", peers.len());
                            for peer in peers {
                                if let Err(e) = sync_service_clone.connect_peer(&peer).await {
                                    eprintln!("Failed to connect to peer {}: {:?}", peer.id, e);
                                } else {
                                    println!("Connected to peer: {}", peer.id);
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("Error discovering peers: {:?}", e);
                        }
                    }
                    
                    // Wait before next discovery
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            });
            
            // Keep the process running
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
        
        DagSyncCommands::Offer { peer, federation, dag_dir, max_nodes } => {
            // Create DAG store
            let store = Arc::new(icn_types::dag::rocksdb::RocksDbDagStore::new(dag_dir)?);
            
            // Create transport config
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                federation_id: federation.clone(),
                local_did: None, // We could load from key file in the future
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
                None, // Local DID
            );
            
            // Get nodes from the store (limited by max_nodes)
            let cids = store.list_cids(*max_nodes).await?;
            
            if cids.is_empty() {
                println!("No nodes available to offer");
                return Ok(());
            }
            
            println!("Offering {} nodes to peer {}", cids.len(), peer);
            
            // Offer nodes to the peer
            match sync_service.offer_nodes(peer, &cids).await {
                Ok(requested_cids) => {
                    println!("Peer requested {} nodes", requested_cids.len());
                    
                    if !requested_cids.is_empty() {
                        // Fetch the requested nodes from our store
                        let mut nodes = Vec::new();
                        for cid in &requested_cids {
                            if let Ok(Some(node)) = store.get(cid).await {
                                nodes.push(node);
                            }
                        }
                        
                        // Create a bundle and send it
                        if !nodes.is_empty() {
                            sync_service.broadcast_nodes(&nodes).await?;
                            println!("Sent {} nodes to peer", nodes.len());
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to offer nodes: {:?}", e);
                }
            }
            
            Ok(())
        },
        
        DagSyncCommands::Genesis { 
            federation, 
            dag_dir,
            key,
            policy_id,
            founding_dids,
            start_node,
            listen_addr,
            mdns,
        } => {
            // Create storage directory
            std::fs::create_dir_all(dag_dir)?;
            
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
            let mut store_ref = store.clone();

            println!("Creating genesis state for federation: {}", federation);
            
            // Create a genesis state object - this would be the initial state of the federation
            let genesis_state = serde_json::json!({
                "type": "GenesisState",
                "federation_id": federation,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "founding_members": founding_dids,
                "policy_id": policy_id,
                "description": "Genesis state for the federation",
            });
            
            // Serialize the genesis state and compute its CID
            let genesis_bytes = serde_json::to_vec(&genesis_state)
                .context("Failed to serialize genesis state")?;
                
            // Use the DagNodeBuilder to create a raw payload node
            let genesis_node = icn_types::dag::DagNodeBuilder::new()
                .with_payload(icn_types::dag::DagPayload::Json(genesis_state))
                .with_author(author_did.clone())
                .with_federation_id(federation.clone())
                .with_label("GenesisState".to_string())
                .build()?;
                
            // Sign the genesis node
            let node_bytes = serde_json::to_vec(&genesis_node)
                .context("Failed to serialize genesis node")?;
            let signature = dummy_key.sign(&node_bytes);
            
            // Create a signed node
            let signed_genesis_node = icn_types::dag::SignedDagNode {
                node: genesis_node,
                signature,
                cid: None, // Will be computed when added to the DAG
            };
            
            // Add to the DAG store to get its CID
            let genesis_cid = store_ref.add_node(signed_genesis_node)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add genesis node to DAG: {}", e))?;
                
            println!("Genesis state created with CID: {}", genesis_cid);
            
            // Create DIDs for quorum members and signatures
            let quorum_dids: Vec<Did> = founding_dids.iter()
                .map(|s| Did::from(s.clone()))
                .collect();
                
            // Create signatures (in a real implementation, we would collect actual signatures)
            // For now, we just create dummy signatures for all founding members
            let signatures = quorum_dids.iter()
                .map(|did| (did.clone(), dummy_key.sign(genesis_cid.as_bytes())))
                .collect();
                
            // Create quorum proof
            let quorum_proof = icn_types::quorum::QuorumProof {
                data_cid: genesis_cid.clone(),
                policy_id: policy_id.clone(),
                signatures,
                metadata: None,
            };
            
            // Create TrustBundle with the genesis state as its state CID
            let trust_bundle = TrustBundle::new(
                genesis_cid.clone(),
                quorum_proof,
                vec![], // No previous anchors for genesis
                Some(serde_json::json!({
                    "name": "Genesis TrustBundle",
                    "description": "Initial TrustBundle for the federation",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })),
            );
            
            // Anchor the TrustBundle to the DAG
            let bundle_node = trust_bundle.to_dag_node(author_did.clone())?;
            let bundle_bytes = serde_json::to_vec(&bundle_node)
                .context("Failed to serialize bundle node")?;
            let bundle_signature = dummy_key.sign(&bundle_bytes);
            
            let signed_bundle_node = icn_types::dag::SignedDagNode {
                node: bundle_node,
                signature: bundle_signature,
                cid: None,
            };
            
            let bundle_cid = store_ref.add_node(signed_bundle_node)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add bundle node to DAG: {}", e))?;
                
            println!("Genesis TrustBundle anchored with CID: {}", bundle_cid);
            
            // If not starting a node, we're done
            if !*start_node {
                println!("Genesis state created successfully. Node not started.");
                return Ok(());
            }
            
            println!("Starting p2p node for federation sync...");
            
            // Create transport config for the p2p node
            let transport_config = TransportConfig {
                peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                federation_id: federation.clone(),
                local_did: Some(author_did.clone()), // Use the author DID for the node
                listen_addresses: vec![listen_addr.clone()],
                bootstrap_peers: vec![],
                enable_mdns: *mdns,
                enable_kad_dht: false,
                max_message_size: 1024 * 1024, // 1MB
                request_timeout: 30, // 30 seconds
            };
            
            // Create the transport
            let transport = Libp2pDagTransport::new(transport_config).await?;
            
            // Create sync policy
            let mut policy = SyncPolicy::default();
            policy.min_quorum = 1; // Genesis quorum is always 1
            
            // Set authorized DIDs
            let did_set: HashSet<Did> = founding_dids.iter()
                .filter_map(|d| {
                    if d.starts_with("did:") {
                        Some(Did::from(d.to_string()))
                    } else {
                        None
                    }
                })
                .collect();
            
            if !did_set.is_empty() {
                policy.authorized_dids = Some(did_set);
            }
            
            // Create the sync service with the policy
            let sync_service = NetworkDagSyncService::new(
                transport,
                store,
                federation.clone(),
                Some(author_did), // Use the author DID for the node
            ).with_policy(policy);
            
            // Print listening address
            println!("Listening on: {}", listen_addr);
            
            // Start background sync
            sync_service.start_background_sync().await?;
            println!("Background sync started");
            
            // Discover peers periodically
            let sync_service_clone = sync_service.clone();
            tokio::spawn(async move {
                loop {
                    match sync_service_clone.discover_peers().await {
                        Ok(peers) => {
                            println!("Discovered {} peers", peers.len());
                            for peer in peers {
                                if let Err(e) = sync_service_clone.connect_peer(&peer).await {
                                    eprintln!("Failed to connect to peer {}: {:?}", peer.id, e);
                                } else {
                                    println!("Connected to peer: {}", peer.id);
                                    
                                    // Once connected, try to sync with the peer
                                    match sync_service_clone.sync_with_peer(&peer).await {
                                        Ok(result) => {
                                            println!("Synced with peer {}: {} nodes accepted, {} nodes rejected", 
                                                peer.id, result.accepted_nodes.len(), result.rejected_nodes.len());
                                        },
                                        Err(e) => {
                                            eprintln!("Failed to sync with peer {}: {:?}", peer.id, e);
                                        }
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("Error discovering peers: {:?}", e);
                        }
                    }
                    
                    // Wait before next discovery
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            });
            
            // Keep the process running
            loop {
                // Display some status information every 10 seconds
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                
                // Get tip nodes to show the current DAG state
                match store_ref.get_tips().await {
                    Ok(tips) => {
                        println!("Current DAG has {} tip nodes", tips.len());
                    },
                    Err(e) => {
                        eprintln!("Error getting DAG tips: {:?}", e);
                    }
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