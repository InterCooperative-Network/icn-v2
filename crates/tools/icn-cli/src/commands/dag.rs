use clap::{Args, Subcommand, ValueHint, Parser};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use crate::config;
use std::path::PathBuf;
use std::collections::{HashSet, VecDeque};
use hex;
use icn_core_types::Did;
use icn_types::dag::NodeScope;
use icn_runtime::dag_indexing::{SledDagIndex, DagIndex, IndexError};
use std::str::FromStr;
use anyhow::Result;
use icn_ccl_compiler::compile;
use chrono::Utc;
use serde_json::json;

// Define the DagCommands enum here (or move it from main.rs)
#[derive(Subcommand, Debug, Clone)]
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

    /// Query the DAG index for nodes by DID or scope.
    #[command(name = "query")]
    Query(QueryArgs),

    /// Submit a DAG payload.
    #[command(name = "submit")]
    Submit {
        /// File containing the payload.
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Payload content.
        #[arg(short, long)]
        payload: Option<String>,
    },

    /// Propose a CCL.
    #[command(name = "propose-ccl")]
    ProposeCcl {
        /// File containing the CCL source.
        #[arg(short, long)]
        file: PathBuf,
        /// Scope of the CCL.
        #[arg(short, long)]
        scope: String,
        /// Title of the CCL.
        #[arg(short, long)]
        title: String,
        /// Output file for the proposal JSON.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        out: Option<PathBuf>,
    },
}

#[derive(Args, Debug, Clone)]
pub struct QueryArgs {
    /// Query by author DID.
    #[arg(long, group = "query_type")]
    did: Option<String>,

    /// Query by node scope (e.g., 'Federation', 'Community', 'Cooperative').
    #[arg(long, group = "query_type")]
    scope: Option<String>,

    /// Optional path to the DAG index directory.
    /// Defaults to the path configured in the runtime (e.g., runtime_data/dag_index).
    #[arg(long, short = 'i', value_hint = ValueHint::DirPath)]
    index_dir: Option<PathBuf>,
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
        DagCommands::Query(args) => handle_query(context, args).await,
        DagCommands::Submit { file, payload } => handle_submit(file.as_ref().cloned(), payload.as_ref().cloned()).await,
        DagCommands::ProposeCcl { file, scope, title, out } => handle_propose_ccl(file.clone(), scope.clone(), title.clone(), out.as_ref().cloned()).await,
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

// New handler for the query command
async fn handle_query(context: &mut CliContext, args: &QueryArgs) -> CliResult {
    // Determine index path: use provided path or default from config/context
    // Assuming CliContext can provide the default path (needs implementation in CliContext)
    let index_path = args.index_dir.clone().unwrap_or_else(|| context.get_default_dag_index_path());

    if context.verbose {
        println!("Querying DAG index at: {:?}", index_path);
        println!("Query args: {:?}", args);
    }

    if !index_path.exists() {
        return Err(CliError::InvalidPath(index_path.to_path_buf()));
    }

    let index_path_str = index_path.to_str()
        .ok_or_else(|| CliError::InvalidPath(index_path.to_path_buf()))?;
    let index = SledDagIndex::new(index_path_str)?;

    let cids = match (&args.did, &args.scope) {
        (Some(did_str), None) => {
            let did = Did::from_str(did_str).map_err(|_| CliError::InvalidDidFormat(did_str.clone()))?;
            println!("Querying index for DID: {}", did);
            index.nodes_by_did(&did)
                 .map_err(CliError::from)?
        }
        (None, Some(scope_str)) => {
            // Parse scope string - needs robust parsing
            let scope = parse_node_scope(scope_str)?;
            println!("Querying index for Scope: {:?}", scope);
            index.nodes_by_scope(&scope)
                 .map_err(CliError::from)?
        }
        _ => {
            // This case should be prevented by clap group validation
            return Err(CliError::InvalidInput("Exactly one of --did or --scope must be provided".to_string()));
        }
    };

    if cids.is_empty() {
        println!("No matching nodes found in the index.");
    } else {
        println!("Found {} matching node(s):", cids.len());
        for cid in cids {
            println!("  {}", cid);
        }
    }

    Ok(())
}

// Helper function to parse NodeScope from string
// This needs to be adapted based on the exact string format expected/used
fn parse_node_scope(scope_str: &str) -> Result<NodeScope, CliError> {
    // Simple parsing based on Debug format used in SledDagIndex for now
    // A more robust parser might be needed (case-insensitive, specific keywords)
    match scope_str.to_lowercase().as_str() {
        "federation" => Ok(NodeScope::Federation),
        "community" => Ok(NodeScope::Community),
        "cooperative" => Ok(NodeScope::Cooperative),
        // Add parsing for scope with ID if the index stores it that way
        // e.g., if the key is "Community(\"solar\")", more complex parsing is needed.
        // Current SledDagIndex uses Debug format like `Community` (without ID).
        _ => Err(CliError::InvalidInput(format!("Invalid scope string: '{}'. Use Federation, Community, or Cooperative.", scope_str)))
    }
}

async fn handle_submit(file: Option<PathBuf>, payload: Option<String>) -> CliResult {
    if let Some(f) = file {
        println!("Submitting DAG payload from file: {}", f.display());
    } else if let Some(p) = payload {
        println!("Submitting DAG payload: {}", p);
    }
    // ... (actual submit logic) ...
    Ok(())
}

async fn handle_propose_ccl(file: PathBuf, scope: String, title: String, out: Option<PathBuf>) -> CliResult {
    // 1. read file
    let source = std::fs::read_to_string(&file)
        .map_err(|e| anyhow::anyhow!("Failed to read CCL file {}: {}", file.display(), e))?;

    // 2. compile CCL ➜ Wasm + hash
    let art = compile(&source, &scope)
        .map_err(|e| anyhow::anyhow!("CCL compilation error: {}", e))?;

    // 3. persist Wasm bytes under $ICN_DATA/wasm/<hash>.wasm
    // Assuming config::data_dir() exists and returns Result<PathBuf, Error>
    let data_dir = config::data_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get ICN data directory: {}", e))?;
    let wasm_dir = data_dir.join("wasm");
    std::fs::create_dir_all(&wasm_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create wasm directory {}: {}", wasm_dir.display(), e))?;
    let wasm_path = wasm_dir.join(format!("{}.wasm", art.hash_hex));
    std::fs::write(&wasm_path, &art.wasm)
        .map_err(|e| anyhow::anyhow!("Failed to write Wasm file {}: {}", wasm_path.display(), e))?;

    // 4. build JSON payload
    let payload = json!({
        "kind":       "CCLProposal",
        "title":      title,
        "wasm_hash":  art.hash_hex,
        "wasm_size":  art.wasm.len(),
        "scope":      scope,
        "timestamp":  Utc::now().timestamp(),
    });

    // 5. output
    if let Some(out_path) = out {
        let pretty_json = serde_json::to_vec_pretty(&payload)
            .map_err(|e| anyhow::anyhow!("Failed to serialize payload to JSON: {}", e))?;
        std::fs::write(&out_path, pretty_json)
            .map_err(|e| anyhow::anyhow!("Failed to write proposal JSON to {}: {}", out_path.display(), e))?;
        println!("📄 Proposal JSON written to {}", out_path.display());
    } else {
        let pretty_json = serde_json::to_string_pretty(&payload)
            .map_err(|e| anyhow::anyhow!("Failed to serialize payload to JSON string: {}", e))?;
        println!("{}", pretty_json);
    }

    println!("✅ Wasm stored at {}", wasm_path.display());
    Ok(())
}

// Placeholder for config module if it doesn't exist
// mod config {
//     use std::path::PathBuf;
//     use anyhow::Result;
//     pub fn data_dir() -> Result<PathBuf> {
//         // Dummy implementation, replace with actual logic
//         Ok(PathBuf::from(".").join(".icn_data"))
//     }
// }