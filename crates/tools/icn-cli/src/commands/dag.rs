use clap::{Subcommand};
use crate::context::CliContext;
use crate::error::{CliError};
use std::path::PathBuf;
use std::collections::{HashSet, VecDeque};
use hex;

// Define the DagCommands enum here (or move it from main.rs)
#[derive(Subcommand, Debug)]
pub enum DagCommands {
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
        #[arg(short = 'd', long)]
        dag_dir: Option<PathBuf>,
    },

    /// Replay and verify a DAG branch
    #[command(name = "replay")]
    Replay {
        /// CID of the DAG node to start replay from
        #[arg(short, long)]
        cid: String,

        /// Path to DAG storage directory
        #[arg(short = 'd', long)]
        dag_dir: Option<PathBuf>,
    },

    /// Verify a TrustBundle
    #[command(name = "verify-bundle")]
    VerifyBundle {
        /// CID of the TrustBundle to verify
        #[arg(short, long)]
        cid: String,

        /// Path to DAG storage directory
        #[arg(short = 'd', long)]
        dag_dir: Option<PathBuf>,
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
        #[arg(short = 'd', long)]
        dag_dir: Option<PathBuf>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Synchronize with a federation peer (Deprecated? Use sync-p2p)
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
        #[arg(short = 'd', long)]
        dag_dir: Option<PathBuf>,
        
        /// Trust level for this peer (0-100)
        #[arg(short, long, default_value = "50")]
        trust: u8,
    },

    // Note: The original code had SyncP2P(DagSyncCommands) under Dag.
    // It might be cleaner to move SyncP2P to the top level `Commands` enum in main.rs.
    // For now, keeping it here but commenting out.
    // /// Advanced DAG sync commands with libp2p support
    // #[command(subcommand)]
    // SyncP2P(DagSyncCommands),

    /// Generate a visual representation of the DAG
    #[command(name = "visualize")]
    Visualize {
        /// Path to DAG storage directory
        #[arg(long)]
        dag_dir: Option<PathBuf>,
        
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

// Handler for all DAG commands
pub async fn handle_dag_command(
    context: &mut CliContext, // Pass mutable context
    cmd: &DagCommands
) -> Result<(), CliError> {
    // Basic logging based on verbosity flag in context
    // if context.verbose { println!("Handling DAG command: {:?}", cmd); }

    match cmd {
        DagCommands::SubmitAnchor { input, anchor_type, key, dag_dir } => {
            // Prefix unused variables
            let (_input, _anchor_type, _key, _dag_dir) = (input, anchor_type, key, dag_dir);
            println!("SubmitAnchor command invoked (not implemented)");
            // TODO: Implement anchor submission logic
            unimplemented!("SubmitAnchor handler")
        }
        DagCommands::Replay { cid, dag_dir } => {
            println!("Replaying DAG from: {}", cid);
            
            let dag_store = context.get_dag_store(dag_dir.as_ref().map(|v| &**v))?;
            
            // Parse the input CID string
            // Note: Using external cid crate here for parsing
            let external_cid_parsed: cid::CidGeneric<64> = cid.as_str().parse()
                .map_err(|e: cid::Error| CliError::InvalidCidFormat(format!("Invalid start CID string '{}': {}", cid, e)))?;
            let start_cid = icn_core_types::Cid::from_bytes(&external_cid_parsed.to_bytes())
                .map_err(|e| CliError::InvalidCidFormat(format!("Failed to convert start CID to internal format: {}", e)))?;

            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(start_cid);

            println!("\n--- DAG Replay Start ---");

            while let Some(current_cid) = queue.pop_front() {
                if !visited.insert(current_cid.clone()) {
                    continue; // Already processed
                }

                // Adjust match for Result<SignedDagNode, DagError>
                match dag_store.get_node(&current_cid).await {
                    Ok(signed_node) => { // Renamed variable for clarity
                        println!("\nNode: {}", current_cid);
                        println!("  Timestamp: {}", signed_node.node.metadata.timestamp); // Access via node.metadata
                        println!("  Author: {}", signed_node.node.author); // Access via node
                        println!("  Signature: {}", hex::encode(signed_node.signature.to_bytes())); // Access signature field
                        println!("  Parents: {:?}", signed_node.node.parents); // Access via node
                        
                        // Determine payload type and length
                        let (payload_type_str, payload_len) = match &signed_node.node.payload {
                            icn_types::dag::DagPayload::Raw(data) => ("Raw", data.len()),
                            icn_types::dag::DagPayload::Json(value) => ("Json", value.to_string().len()), // Approx length
                            icn_types::dag::DagPayload::Reference(cid) => ("Reference", cid.to_string().len()),
                            icn_types::dag::DagPayload::TrustBundle(cid) => ("TrustBundleRef", cid.to_string().len()),
                            icn_types::dag::DagPayload::ExecutionReceipt(cid) => ("ExecReceiptRef", cid.to_string().len()),
                        };
                        println!("  Payload Type: {}", payload_type_str);
                        println!("  Payload (size approx): {} bytes", payload_len);

                        // Iterate through parents
                        for parent_cid in &signed_node.node.parents { // Access via node, use reference
                            if !visited.contains(parent_cid) {
                                queue.push_back(parent_cid.clone());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to get node {} during replay: {}", current_cid, e);
                    }
                }
            }
            println!("\n--- DAG Replay End ---");
            Ok(())
        }
        DagCommands::VerifyBundle { cid, dag_dir } => {
             // Prefix unused variables
             let (_cid, _dag_dir) = (cid, dag_dir);
            println!("VerifyBundle command invoked (not implemented)");
            // TODO: Implement bundle verification
            unimplemented!("VerifyBundle handler")
        }
        DagCommands::ExportThread { from, to, dag_dir, output } => {
             // Prefix unused variables
             let (_from, _to, _dag_dir, _output) = (from, to, dag_dir, output);
            println!("ExportThread command invoked (not implemented)");
            // TODO: Implement thread export
            unimplemented!("ExportThread handler")
        }
        DagCommands::Sync { .. } => {
             println!("Executing dag sync... (Consider using sync-p2p)");
             // TODO: Implement logic (or remove if fully deprecated)
             unimplemented!("Sync handler")
        }
        // DagCommands::SyncP2P(sync_cmd) => {
        //     // Delegate to a sync_p2p handler if kept here
        //     // crate::commands::sync_p2p::handle_dag_sync_command(context, sync_cmd).await?
        // }
        DagCommands::Visualize { dag_dir, output, thread_did, max_nodes } => {
            // Prefix unused variables
            let (_dag_dir, _output, _thread_did, _max_nodes) = (dag_dir, output, thread_did, max_nodes);
            println!("Visualize command invoked (not implemented)");
            // TODO: Implement visualization
            unimplemented!("Visualize handler")
        }
    }
}