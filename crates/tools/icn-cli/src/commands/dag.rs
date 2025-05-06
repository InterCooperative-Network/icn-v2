use clap::{Args, Subcommand, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::collections::{HashSet, VecDeque};
use hex;

// Define the DagCommands enum here (or move it from main.rs)
#[derive(Subcommand, Debug)]
pub enum DagCommands {
    /// Replay and verify a DAG branch.
    #[command(name = "replay")]
    Replay {
        /// CID of the DAG node to start replay from.
        #[arg(short, long)]
        cid: String,
        /// Optional path to DAG storage directory.
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },

    /// Export a thread (path between two DAG nodes).
    #[command(name = "export-thread")]
    ExportThread {
        /// CID of the first node.
        #[arg(short, long)]
        from: String,
        /// CID of the second node.
        #[arg(short, long)]
        to: String,
        /// Optional path to DAG storage directory.
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        /// Output file for the exported thread data.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
    },

    /// Generate a visual representation of the DAG.
    #[command(name = "visualize")]
    Visualize {
        /// Optional path to DAG storage directory.
        #[arg(long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        /// Output file for the graph visualization (DOT format).
        #[arg(long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
        /// Filter by thread DID (optional) to show only nodes from a specific author.
        #[arg(long)]
        thread_did: Option<String>,
        /// Maximum number of nodes to include in visualization.
        #[arg(long, default_value = "100")]
        max_nodes: usize,
    },

    /// Fetch and display a raw SignedDagNode by its CID.
    #[command(name = "get-node")]
    GetNode(GetNodeArgs),

    /// Fetch a DAG node and attempt to display its payload.
    #[command(name = "get-payload")]
    GetPayload(GetPayloadArgs),
}

#[derive(Args, Debug, Clone)]
pub struct GetNodeArgs {
    /// CID of the DAG node to fetch.
    #[arg(long)]
    pub cid: String,
    /// Optional path to DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct GetPayloadArgs {
    /// CID of the DAG node whose payload is to be fetched.
    #[arg(long)]
    pub cid: String,
    /// Optional path to DAG storage directory.
    #[arg(long, short = 'd', value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
    /// Optional output file to save the payload.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}

// Handler for all DAG commands
pub async fn handle_dag_command(
    context: &mut CliContext,
    cmd: &DagCommands,
) -> CliResult {
    if context.verbose {
        println!("Handling DAG command: {:?}", cmd);
    }
    match cmd {
        DagCommands::Replay { cid, dag_dir } => {
            println!("Replaying DAG from: {}", cid);
            let dag_store = context.get_dag_store(dag_dir.as_ref().map(|v| &**v))?;
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
                    continue;
                }
                match dag_store.get_node(&current_cid).await {
                    Ok(signed_node) => {
                        println!("\nNode: {}", current_cid);
                        println!("  Timestamp: {}", signed_node.node.metadata.timestamp);
                        println!("  Author: {}", signed_node.node.author);
                        println!("  Signature: {}", hex::encode(signed_node.signature.to_bytes()));
                        println!("  Parents: {:?}", signed_node.node.parents);
                        let (payload_type_str, payload_len) = match &signed_node.node.payload {
                            icn_types::dag::DagPayload::Raw(data) => ("Raw", data.len()),
                            icn_types::dag::DagPayload::Json(value) => ("Json", value.to_string().len()),
                            icn_types::dag::DagPayload::Reference(cid) => ("Reference", cid.to_string().len()),
                            icn_types::dag::DagPayload::TrustBundle(cid) => ("TrustBundleRef", cid.to_string().len()),
                            icn_types::dag::DagPayload::ExecutionReceipt(cid) => ("ExecReceiptRef", cid.to_string().len()),
                        };
                        println!("  Payload Type: {}", payload_type_str);
                        println!("  Payload (size approx): {} bytes", payload_len);
                        for parent_cid in &signed_node.node.parents {
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
        DagCommands::ExportThread { from, to, dag_dir, output } => {
             let (_from, _to, _dag_dir, _output) = (from, to, dag_dir, output); // Avoid unused var warnings for now
            println!("ExportThread command invoked (not implemented)");
            Err(CliError::Unimplemented("dag export-thread".to_string()))
        }
        DagCommands::Visualize { dag_dir, output, thread_did, max_nodes } => {
            let (_dag_dir, _output, _thread_did, _max_nodes) = (dag_dir, output, thread_did, max_nodes); // Avoid unused
            println!("Visualize command invoked (not implemented)");
            Err(CliError::Unimplemented("dag visualize".to_string()))
        }
        DagCommands::GetNode(args) => handle_get_node(context, args).await,
        DagCommands::GetPayload(args) => handle_get_payload(context, args).await,
    }
}

// Placeholder handlers for new commands
async fn handle_get_node(_context: &mut CliContext, args: &GetNodeArgs) -> CliResult {
    println!("Executing dag get-node with args: {:?}", args);
    // TODO: Implement logic to get DAG store, fetch SignedDagNode, and print its details.
    Err(CliError::Unimplemented("dag get-node".to_string()))
}

async fn handle_get_payload(_context: &mut CliContext, args: &GetPayloadArgs) -> CliResult {
    println!("Executing dag get-payload with args: {:?}", args);
    // TODO: Implement logic to get DAG store, fetch SignedDagNode, extract payload, attempt to pretty-print/save.
    Err(CliError::Unimplemented("dag get-payload".to_string()))
}