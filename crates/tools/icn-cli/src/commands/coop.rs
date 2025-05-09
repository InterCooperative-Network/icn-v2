use clap::{Args, Subcommand, ValueHint};
use crate::context::{CliContext, get_cid};
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::fs;
use icn_types::dag::{DagNodeBuilder, DagPayload, NodeScope};
use icn_types::Did;
use serde_json::json;

#[derive(Debug, Subcommand)]
pub enum CoopCommands {
    /// Create a new cooperative in the federation
    #[command(name = "create")]
    Create {
        /// Cooperative ID
        #[arg(long)]
        coop_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Description of the cooperative
        #[arg(long)]
        description: Option<String>,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Create a new proposal in the cooperative's DAG
    #[command(name = "propose")]
    Propose {
        /// Cooperative ID
        #[arg(long)]
        coop_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Proposal title
        #[arg(long)]
        title: String,
        
        /// Proposal content file
        #[arg(long, value_hint = ValueHint::FilePath)]
        content: PathBuf,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Vote on a proposal in the cooperative's DAG
    #[command(name = "vote")]
    Vote {
        /// Cooperative ID
        #[arg(long)]
        coop_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Proposal CID
        #[arg(long)]
        proposal_cid: String,
        
        /// Vote (yes/no)
        #[arg(long)]
        vote: String,
        
        /// Optional comment
        #[arg(long)]
        comment: Option<String>,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Export a cooperative's DAG thread
    #[command(name = "export-thread")]
    ExportThread {
        /// Cooperative ID
        #[arg(long)]
        coop_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Output file
        #[arg(long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Join a cooperative to a federation
    #[command(name = "join-federation")]
    JoinFederation {
        /// Cooperative ID
        #[arg(long)]
        coop_id: String,
        
        /// Federation ID to join
        #[arg(long)]
        federation_id: String,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
}

pub async fn handle_coop_command(command: &CoopCommands, ctx: &mut CliContext) -> CliResult<()> {
    match command {
        CoopCommands::Create { coop_id, federation_id, key, description, dag_dir } => {
            let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Create a genesis node for the cooperative
            let desc = description.clone().unwrap_or_else(|| format!("Cooperative {}", coop_id));
            let payload = DagPayload::Json(json!({
                "type": "CooperativeGenesis",
                "name": coop_id,
                "federationId": federation_id,
                "description": desc,
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "founder": did.to_string(),
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Cooperative)
                .with_scope_id(coop_id.clone())
                .with_label("CooperativeGenesis".to_string())
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Created cooperative '{}' in federation '{}' with genesis node {}", 
                coop_id, federation_id, cid);
            
            Ok(())
        },
        
        CoopCommands::Propose { coop_id, federation_id, title, content, key, dag_dir } => {
            let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read proposal content from file
            let content_str = fs::read_to_string(content)
                .map_err(|e| CliError::IoError(format!("Failed to read content file: {}", e)))?;
            
            // Create a proposal node
            let payload = DagPayload::Json(json!({
                "type": "CooperativeProposal",
                "title": title,
                "content": content_str,
                "proposedAt": chrono::Utc::now().to_rfc3339(),
                "proposer": did.to_string(),
                "status": "Open",
            }));
            
            // Get the latest nodes from this cooperative to use as parents
            let coop_nodes = dag_store.get_nodes_by_payload_type("Cooperative").await?;
            let mut parent_cids = Vec::new();
            
            for mut node in coop_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == coop_id {
                        let cid = get_cid(&node)?;
                        parent_cids.push(cid);
                    }
                }
            }
            
            let mut builder = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Cooperative)
                .with_scope_id(coop_id.clone())
                .with_label("CooperativeProposal".to_string());
            
            // Add parents if available
            if !parent_cids.is_empty() {
                builder = builder.with_parents(parent_cids);
            }
            
            let node = builder.build()?;
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Created proposal '{}' in cooperative '{}' with CID {}", 
                title, coop_id, cid);
            
            Ok(())
        },
        
        CoopCommands::Vote { coop_id, federation_id, proposal_cid, vote, comment, key, dag_dir } => {
            let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the CID
            let proposal_cid_obj = cid_from_string(proposal_cid)?;
            
            // Get the proposal node
            let proposal_node = dag_store.get_node(&proposal_cid_obj).await?;
            
            // Create a vote node
            let payload = DagPayload::Json(json!({
                "type": "CooperativeVote",
                "vote": vote,
                "comment": comment,
                "votedAt": chrono::Utc::now().to_rfc3339(),
                "voter": did.to_string(),
                "proposalCid": proposal_cid,
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Cooperative)
                .with_scope_id(coop_id.clone())
                .with_label("CooperativeVote".to_string())
                .with_parent(proposal_cid_obj)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Recorded vote '{}' on proposal {} with CID {}", 
                vote, proposal_cid, cid);
            
            Ok(())
        },
        
        CoopCommands::ExportThread { coop_id, federation_id, output, dag_dir } => {
            let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            
            // Get all nodes for this cooperative
            let all_nodes = dag_store.get_ordered_nodes().await?;
            let mut coop_nodes = Vec::new();
            
            for node in all_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == coop_id && node.node.metadata.scope == NodeScope::Cooperative {
                        coop_nodes.push(node);
                    }
                }
            }
            
            // Export the nodes to a file
            let json = serde_json::to_string_pretty(&coop_nodes)
                .map_err(|e| CliError::SerializationError(e.to_string()))?;
            
            fs::write(output, json)
                .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
            
            println!("Exported {} nodes from cooperative '{}' to {}", 
                coop_nodes.len(), coop_id, output.display());
            
            Ok(())
        },
        
        CoopCommands::JoinFederation { coop_id, federation_id, key, dag_dir } => {
            let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Get the federation genesis node
            let mut federation_nodes = dag_store.get_nodes_by_payload_type("FederationGenesis").await?;
            if federation_nodes.is_empty() {
                return Err(CliError::SerializationError(
                    format!("Federation '{}' not found", federation_id)));
            }
            
            let federation_genesis = &mut federation_nodes[0];
            let federation_genesis_cid = get_cid(federation_genesis)?;
            
            // Get the cooperative genesis node
            let coop_nodes = dag_store.get_nodes_by_payload_type("CooperativeGenesis").await?;
            let mut coop_genesis = None;
            
            for node in coop_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == coop_id {
                        coop_genesis = Some(node);
                        break;
                    }
                }
            }
            
            let mut coop_genesis = match coop_genesis {
                Some(node) => node,
                None => {
                    return Err(CliError::SerializationError(
                        format!("Cooperative '{}' not found", coop_id)));
                },
            };
            
            let coop_genesis_cid = get_cid(&coop_genesis)?;
            
            // Create a join request node
            let payload = DagPayload::Json(json!({
                "type": "CooperativeJoinRequest",
                "cooperativeId": coop_id,
                "federationId": federation_id,
                "cooperativeGenesisCid": coop_genesis_cid.to_string(),
                "federationGenesisCid": federation_genesis_cid.to_string(),
                "requestedAt": chrono::Utc::now().to_rfc3339(),
                "requester": did.to_string(),
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("CooperativeJoinRequest".to_string())
                .with_parent(federation_genesis_cid)
                .with_parent(coop_genesis_cid)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Submitted join request for cooperative '{}' to federation '{}' with CID {}", 
                coop_id, federation_id, cid);
            
            Ok(())
        },
    }
}

// Helper function to parse a CID from a string
fn cid_from_string(cid_str: &str) -> CliResult<icn_types::Cid> {
    icn_types::Cid::from_bytes(cid_str.as_bytes())
        .map_err(|e| CliError::SerializationError(format!("Invalid CID: {}", e)))
} 