use clap::{Args, Subcommand, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::fs;
use icn_types::dag::{DagNodeBuilder, DagPayload, NodeScope};
use icn_types::Did;
use serde_json::json;

/// Enumeration of supported scope types
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeType {
    Cooperative,
    Community,
}

impl ScopeType {
    pub fn to_node_scope(&self) -> NodeScope {
        match self {
            ScopeType::Cooperative => NodeScope::Cooperative,
            ScopeType::Community => NodeScope::Community,
        }
    }
    
    pub fn from_str(s: &str) -> Result<Self, CliError> {
        match s.to_lowercase().as_str() {
            "cooperative" | "coop" => Ok(ScopeType::Cooperative),
            "community" => Ok(ScopeType::Community),
            _ => Err(CliError::ValidationError(format!("Invalid scope type: {}", s))),
        }
    }
}

#[derive(Debug, Args)]
pub struct ScopeOptions {
    /// The scope type (cooperative or community)
    #[arg(long)]
    pub scope_type: String,
    
    /// Scope ID (cooperative ID or community ID)
    #[arg(long)]
    pub scope_id: String,
    
    /// Federation ID
    #[arg(long)]
    pub federation_id: String,
    
    /// Path to the signing key file
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub key: PathBuf,
    
    /// Optional path to DAG storage directory
    #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum ScopeCommands {
    /// Create a new scope (cooperative or community) in the federation
    #[command(name = "create")]
    Create {
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Description of the scope
        #[arg(long)]
        description: Option<String>,
    },
    
    /// Create a new proposal in the scope's DAG
    #[command(name = "propose")]
    Propose {
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Proposal title
        #[arg(long)]
        title: String,
        
        /// Proposal content file
        #[arg(long, value_hint = ValueHint::FilePath)]
        content: PathBuf,
    },
    
    /// Vote on a proposal in the scope's DAG
    #[command(name = "vote")]
    Vote {
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Proposal CID
        #[arg(long)]
        proposal_cid: String,
        
        /// Vote (yes/no)
        #[arg(long)]
        vote: String,
        
        /// Optional comment
        #[arg(long)]
        comment: Option<String>,
    },
    
    /// Create a charter (for communities) or bylaws (for cooperatives)
    #[command(name = "create-charter")]
    CreateCharter {
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Charter title
        #[arg(long)]
        title: String,
        
        /// Charter content file
        #[arg(long, value_hint = ValueHint::FilePath)]
        content: PathBuf,
    },
    
    /// Export a scope's DAG thread
    #[command(name = "export-thread")]
    ExportThread {
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Output file
        #[arg(long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
    },
    
    /// Join a scope to a federation
    #[command(name = "join-federation")]
    JoinFederation {
        #[command(flatten)]
        options: ScopeOptions,
    },
}

pub async fn handle_scope_command(command: &ScopeCommands, ctx: &mut CliContext) -> CliResult<()> {
    match command {
        ScopeCommands::Create { options, description } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Create a genesis node for the scope
            let desc = description.clone().unwrap_or_else(|| {
                match scope_type {
                    ScopeType::Cooperative => format!("Cooperative {}", options.scope_id),
                    ScopeType::Community => format!("Community {}", options.scope_id),
                }
            });
            
            let node_type = match scope_type {
                ScopeType::Cooperative => "CooperativeGenesis",
                ScopeType::Community => "CommunityGenesis",
            };
            
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "name": options.scope_id,
                "federationId": options.federation_id,
                "description": desc,
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "founder": did.to_string(),
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(options.federation_id.clone())
                .with_scope(scope_type.to_node_scope())
                .with_scope_id(options.scope_id.clone())
                .with_label(node_type.to_string())
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            let scope_name = match scope_type {
                ScopeType::Cooperative => "cooperative",
                ScopeType::Community => "community",
            };
            
            println!("Created {} '{}' in federation '{}' with genesis node {}", 
                scope_name, options.scope_id, options.federation_id, cid);
            
            Ok(())
        },
        
        ScopeCommands::Propose { options, title, content } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read proposal content from file
            let content_str = fs::read_to_string(content)
                .map_err(|e| CliError::IoError(format!("Failed to read content file: {}", e)))?;
            
            let node_type = match scope_type {
                ScopeType::Cooperative => "CooperativeProposal",
                ScopeType::Community => "CommunityProposal",
            };
            
            // Create a proposal node
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "title": title,
                "content": content_str,
                "proposedAt": chrono::Utc::now().to_rfc3339(),
                "proposer": did.to_string(),
                "status": "Open",
            }));
            
            // Get the latest nodes from this scope to use as parents
            let search_type = match scope_type {
                ScopeType::Cooperative => "Cooperative",
                ScopeType::Community => "Community",
            };
            
            let scope_nodes = dag_store.get_nodes_by_payload_type(search_type).await?;
            let mut parent_cids = Vec::new();
            
            for node in scope_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == &options.scope_id {
                        let cid = node.ensure_cid()?;
                        parent_cids.push(cid);
                    }
                }
            }
            
            let mut builder = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(options.federation_id.clone())
                .with_scope(scope_type.to_node_scope())
                .with_scope_id(options.scope_id.clone())
                .with_label(node_type.to_string());
            
            // Add parents if available
            if !parent_cids.is_empty() {
                builder = builder.with_parents(parent_cids);
            }
            
            let node = builder.build()?;
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            let scope_name = match scope_type {
                ScopeType::Cooperative => "cooperative",
                ScopeType::Community => "community",
            };
            
            println!("Created proposal '{}' in {} '{}' with CID {}", 
                title, scope_name, options.scope_id, cid);
            
            Ok(())
        },
        
        ScopeCommands::Vote { options, proposal_cid, vote, comment } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the CID
            let proposal_cid_obj = cid_from_string(proposal_cid)?;
            
            // Get the proposal node
            let proposal_node = dag_store.get_node(&proposal_cid_obj).await?;
            
            let node_type = match scope_type {
                ScopeType::Cooperative => "CooperativeVote",
                ScopeType::Community => "CommunityVote",
            };
            
            // Create a vote node
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "vote": vote,
                "comment": comment,
                "votedAt": chrono::Utc::now().to_rfc3339(),
                "voter": did.to_string(),
                "proposalCid": proposal_cid,
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(options.federation_id.clone())
                .with_scope(scope_type.to_node_scope())
                .with_scope_id(options.scope_id.clone())
                .with_label(node_type.to_string())
                .with_parent(proposal_cid_obj)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Recorded vote '{}' on proposal {} with CID {}", 
                vote, proposal_cid, cid);
            
            Ok(())
        },
        
        ScopeCommands::CreateCharter { options, title, content } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read charter content from file
            let content_str = fs::read_to_string(content)
                .map_err(|e| CliError::IoError(format!("Failed to read content file: {}", e)))?;
            
            let node_type = match scope_type {
                ScopeType::Cooperative => "CooperativeBylaws",
                ScopeType::Community => "CommunityCharter",
            };
            
            // Create a charter/bylaws node
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "title": title,
                "content": content_str,
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "author": did.to_string(),
                "status": "Active",
            }));
            
            // Get scope genesis node as parent
            let genesis_type = match scope_type {
                ScopeType::Cooperative => "CooperativeGenesis",
                ScopeType::Community => "CommunityGenesis",
            };
            
            let scope_nodes = dag_store.get_nodes_by_payload_type(genesis_type).await?;
            let mut parent_cid = None;
            
            for node in scope_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == &options.scope_id {
                        let cid = node.ensure_cid()?;
                        parent_cid = Some(cid);
                        break;
                    }
                }
            }
            
            let mut builder = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(options.federation_id.clone())
                .with_scope(scope_type.to_node_scope())
                .with_scope_id(options.scope_id.clone())
                .with_label(node_type.to_string());
            
            // Add parent if available
            if let Some(cid) = parent_cid {
                builder = builder.with_parent(cid);
            }
            
            let node = builder.build()?;
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            let (scope_name, doc_type) = match scope_type {
                ScopeType::Cooperative => ("cooperative", "bylaws"),
                ScopeType::Community => ("community", "charter"),
            };
            
            println!("Created {} '{}' for {} '{}' with CID {}", 
                doc_type, title, scope_name, options.scope_id, cid);
            
            Ok(())
        },
        
        ScopeCommands::ExportThread { options, output } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            
            // Get all nodes for this scope
            let all_nodes = dag_store.get_ordered_nodes().await?;
            let mut scope_nodes = Vec::new();
            
            for node in all_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == &options.scope_id && node.node.metadata.scope == scope_type.to_node_scope() {
                        scope_nodes.push(node);
                    }
                }
            }
            
            // Export the nodes to a file
            let json = serde_json::to_string_pretty(&scope_nodes)
                .map_err(|e| CliError::SerializationError(e.to_string()))?;
            
            fs::write(output, json)
                .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
            
            let scope_name = match scope_type {
                ScopeType::Cooperative => "cooperative",
                ScopeType::Community => "community",
            };
            
            println!("Exported {} nodes from {} '{}' to {}", 
                scope_nodes.len(), scope_name, options.scope_id, output.display());
            
            Ok(())
        },
        
        ScopeCommands::JoinFederation { options } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Get the federation genesis node
            let federation_nodes = dag_store.get_nodes_by_payload_type("FederationGenesis").await?;
            if federation_nodes.is_empty() {
                return Err(CliError::ValidationError(
                    format!("Federation '{}' not found", options.federation_id)));
            }
            
            let federation_genesis = &federation_nodes[0];
            let federation_genesis_cid = federation_genesis.ensure_cid()?;
            
            // Get the scope genesis node
            let genesis_type = match scope_type {
                ScopeType::Cooperative => "CooperativeGenesis",
                ScopeType::Community => "CommunityGenesis",
            };
            
            let scope_nodes = dag_store.get_nodes_by_payload_type(genesis_type).await?;
            let mut scope_genesis = None;
            
            for node in scope_nodes {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    if scope_id == &options.scope_id {
                        scope_genesis = Some(node);
                        break;
                    }
                }
            }
            
            let scope_genesis = match scope_genesis {
                Some(node) => node,
                None => {
                    let scope_name = match scope_type {
                        ScopeType::Cooperative => "Cooperative",
                        ScopeType::Community => "Community",
                    };
                    return Err(CliError::ValidationError(
                        format!("{} '{}' not found", scope_name, options.scope_id)));
                },
            };
            
            let scope_genesis_cid = scope_genesis.ensure_cid()?;
            
            // Create a join request node
            let request_type = match scope_type {
                ScopeType::Cooperative => "CooperativeJoinRequest",
                ScopeType::Community => "CommunityJoinRequest",
            };
            
            let scope_id_field = match scope_type {
                ScopeType::Cooperative => "cooperativeId",
                ScopeType::Community => "communityId",
            };
            
            let scope_cid_field = match scope_type {
                ScopeType::Cooperative => "cooperativeGenesisCid",
                ScopeType::Community => "communityGenesisCid",
            };
            
            let mut payload_json = json!({
                "type": request_type,
                "federationId": options.federation_id,
                "federationGenesisCid": federation_genesis_cid.to_string(),
                "requestedAt": chrono::Utc::now().to_rfc3339(),
                "requester": did.to_string(),
            });
            
            payload_json[scope_id_field] = json!(options.scope_id);
            payload_json[scope_cid_field] = json!(scope_genesis_cid.to_string());
            
            let payload = DagPayload::Json(payload_json);
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(options.federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label(request_type.to_string())
                .with_parent(federation_genesis_cid)
                .with_parent(scope_genesis_cid)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            let scope_name = match scope_type {
                ScopeType::Cooperative => "cooperative",
                ScopeType::Community => "community",
            };
            
            println!("Submitted join request for {} '{}' to federation '{}' with CID {}", 
                scope_name, options.scope_id, options.federation_id, cid);
            
            Ok(())
        },
    }
}

// Helper function to parse a CID from a string
fn cid_from_string(cid_str: &str) -> CliResult<icn_types::Cid> {
    icn_types::Cid::from_bytes(cid_str.as_bytes())
        .map_err(|e| CliError::ValidationError(format!("Invalid CID: {}", e)))
} 