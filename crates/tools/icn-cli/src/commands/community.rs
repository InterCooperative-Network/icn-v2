use clap::{Args, Subcommand, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::fs;
use icn_types::dag::{DagNodeBuilder, DagPayload, NodeScope};
use icn_types::Did;
use serde_json::json;

#[derive(Debug, Subcommand)]
pub enum CommunityCommands {
    /// Create a new community in the federation
    #[command(name = "create")]
    Create {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Description of the community
        #[arg(long)]
        description: Option<String>,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Create a new proposal in the community's DAG
    #[command(name = "propose")]
    Propose {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
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
    
    /// Vote on a proposal in the community's DAG
    #[command(name = "vote")]
    Vote {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
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
    
    /// Create a charter for the community
    #[command(name = "create-charter")]
    CreateCharter {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Charter title
        #[arg(long)]
        title: String,
        
        /// Charter content file
        #[arg(long, value_hint = ValueHint::FilePath)]
        content: PathBuf,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Export a community's DAG thread
    #[command(name = "export-thread")]
    ExportThread {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
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
    
    /// Join a community to a federation
    #[command(name = "join-federation")]
    JoinFederation {
        /// Community ID
        #[arg(long)]
        community_id: String,
        
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

pub async fn handle_community_command(command: &CommunityCommands, ctx: &mut CliContext) -> CliResult<()> {
    match command {
        CommunityCommands::Create { community_id, federation_id, key, description, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Create a genesis node for the community
            let desc = description.clone().unwrap_or_else(|| format!("Community {}", community_id));
            let payload = DagPayload::Json(json!({
                "type": "CommunityGenesis",
                "name": community_id,
                "federationId": federation_id,
                "description": desc,
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "founder": did.to_string(),
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Community)
                .with_scope_id(community_id.clone())
                .with_label("CommunityGenesis".to_string())
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Created community '{}' in federation '{}' with genesis node {}", 
                community_id, federation_id, cid);
            
            Ok(())
        },
        
        CommunityCommands::Propose { community_id, federation_id, title, content, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read proposal content from file
            let content_str = fs::read_to_string(content)
                .map_err(|e| CliError::IoError(format!("Failed to read content file: {}", e)))?;
            
            // Create a proposal node
            let payload = DagPayload::Json(json!({
                "type": "CommunityProposal",
                "title": title,
                "content": content_str,
                "proposedAt": chrono::Utc::now().to_rfc3339(),
                "proposer": did.to_string(),
                "status": "Open",
            }));
            
            // Get the latest nodes from this community to use as parents
            let community_nodes = dag_store.get_nodes_by_payload_type("Community").await?;
            let mut parent_cids = Vec::new();
            
            for node in community_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == community_id {
                        let cid = node.ensure_cid()?;
                        parent_cids.push(cid);
                    }
                }
            }
            
            let mut builder = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Community)
                .with_scope_id(community_id.clone())
                .with_label("CommunityProposal".to_string());
            
            // Add parents if available
            if !parent_cids.is_empty() {
                builder = builder.with_parents(parent_cids);
            }
            
            let node = builder.build()?;
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Created proposal '{}' in community '{}' with CID {}", 
                title, community_id, cid);
            
            Ok(())
        },
        
        CommunityCommands::Vote { community_id, federation_id, proposal_cid, vote, comment, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the CID
            let proposal_cid_obj = cid_from_string(proposal_cid)?;
            
            // Get the proposal node
            let proposal_node = dag_store.get_node(&proposal_cid_obj).await?;
            
            // Create a vote node
            let payload = DagPayload::Json(json!({
                "type": "CommunityVote",
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
                .with_scope(NodeScope::Community)
                .with_scope_id(community_id.clone())
                .with_label("CommunityVote".to_string())
                .with_parent(proposal_cid_obj)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Recorded vote '{}' on proposal {} with CID {}", 
                vote, proposal_cid, cid);
            
            Ok(())
        },
        
        CommunityCommands::CreateCharter { community_id, federation_id, title, content, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read charter content from file
            let content_str = fs::read_to_string(content)
                .map_err(|e| CliError::IoError(format!("Failed to read content file: {}", e)))?;
            
            // Create a charter node
            let payload = DagPayload::Json(json!({
                "type": "CommunityCharter",
                "title": title,
                "content": content_str,
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "author": did.to_string(),
                "status": "Active",
            }));
            
            // Get community genesis node as parent
            let community_nodes = dag_store.get_nodes_by_payload_type("CommunityGenesis").await?;
            let mut parent_cid = None;
            
            for node in community_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == community_id {
                        let cid = node.ensure_cid()?;
                        parent_cid = Some(cid);
                        break;
                    }
                }
            }
            
            let mut builder = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Community)
                .with_scope_id(community_id.clone())
                .with_label("CommunityCharter".to_string());
            
            // Add parent if available
            if let Some(cid) = parent_cid {
                builder = builder.with_parent(cid);
            }
            
            let node = builder.build()?;
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Created charter '{}' for community '{}' with CID {}", 
                title, community_id, cid);
            
            Ok(())
        },
        
        CommunityCommands::ExportThread { community_id, federation_id, output, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            
            // Get all nodes for this community
            let all_nodes = dag_store.get_ordered_nodes().await?;
            let mut community_nodes = Vec::new();
            
            for node in all_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == community_id && node.node.metadata.scope == NodeScope::Community {
                        community_nodes.push(node);
                    }
                }
            }
            
            // Export the nodes to a file
            let json = serde_json::to_string_pretty(&community_nodes)
                .map_err(|e| CliError::SerializationError(e.to_string()))?;
            
            fs::write(output, json)
                .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
            
            println!("Exported {} nodes from community '{}' to {}", 
                community_nodes.len(), community_id, output.display());
            
            Ok(())
        },
        
        CommunityCommands::JoinFederation { community_id, federation_id, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Get the federation genesis node
            let federation_nodes = dag_store.get_nodes_by_payload_type("FederationGenesis").await?;
            if federation_nodes.is_empty() {
                return Err(CliError::ValidationError(
                    format!("Federation '{}' not found", federation_id)));
            }
            
            let federation_genesis = &federation_nodes[0];
            let federation_genesis_cid = federation_genesis.ensure_cid()?;
            
            // Get the community genesis node
            let community_nodes = dag_store.get_nodes_by_payload_type("CommunityGenesis").await?;
            let mut community_genesis = None;
            
            for node in community_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == community_id {
                        community_genesis = Some(node);
                        break;
                    }
                }
            }
            
            let community_genesis = match community_genesis {
                Some(node) => node,
                None => return Err(CliError::ValidationError(
                    format!("Community '{}' not found", community_id))),
            };
            
            let community_genesis_cid = community_genesis.ensure_cid()?;
            
            // Create a join request node
            let payload = DagPayload::Json(json!({
                "type": "CommunityJoinRequest",
                "communityId": community_id,
                "federationId": federation_id,
                "communityGenesisCid": community_genesis_cid.to_string(),
                "federationGenesisCid": federation_genesis_cid.to_string(),
                "requestedAt": chrono::Utc::now().to_rfc3339(),
                "requester": did.to_string(),
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("CommunityJoinRequest".to_string())
                .with_parent(federation_genesis_cid)
                .with_parent(community_genesis_cid)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Submitted join request for community '{}' to federation '{}' with CID {}", 
                community_id, federation_id, cid);
            
            Ok(())
        },
    }
}

// Helper function to parse a CID from a string
fn cid_from_string(cid_str: &str) -> CliResult<icn_types::Cid> {
    icn_types::Cid::from_bytes(cid_str.as_bytes())
        .map_err(|e| CliError::ValidationError(format!("Invalid CID: {}", e)))
} 