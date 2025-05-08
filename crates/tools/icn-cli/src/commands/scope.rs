use clap::{Args, Subcommand, ValueHint};
use crate::context::{CliContext, get_cid};
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::fs;
use icn_types::dag::{DagNodeBuilder, DagPayload, NodeScope};
use icn_types::Did;
use serde_json::json;
use crate::commands::observability::{ObservabilityCommands, ScopeObservabilityOptions, handle_dag_view, handle_inspect_policy, handle_activity_log};

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
            _ => Err(CliError::SerializationError(format!("Invalid scope type: {}", s))),
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
    
    /// Set a policy for a scope
    #[command(name = "set-policy")]
    SetPolicy {
        /// Scope options (type, id, federation)
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Path to a JSON file containing the policy configuration
        #[arg(long, value_hint = ValueHint::FilePath)]
        policy_file: PathBuf,
    },
    
    /// Propose a policy update for a scope
    #[command(name = "propose-policy-update")]
    ProposeUpdatePolicy {
        /// Scope options (type, id, federation)
        #[command(flatten)]
        options: ScopeOptions,
        
        /// Path to a JSON file containing the new policy configuration
        #[arg(long, value_hint = ValueHint::FilePath)]
        policy_file: PathBuf,
        
        /// Description of the proposed policy update
        #[arg(long)]
        description: String,
    },
    
    /// Vote on a policy update proposal
    #[command(name = "vote-policy-update")]
    VoteUpdatePolicy {
        /// Scope options (type, id, federation)
        #[command(flatten)]
        options: ScopeOptions,
        
        /// CID of the policy update proposal
        #[arg(long)]
        proposal_cid: String,
        
        /// Vote choice (approve/reject)
        #[arg(long)]
        vote: String,
        
        /// Optional reason for your vote
        #[arg(long)]
        reason: Option<String>,
    },
    
    /// Finalize a policy update with a quorum proof
    #[command(name = "finalize-policy-update")]
    FinalizeUpdatePolicy {
        /// Scope options (type, id, federation)
        #[command(flatten)]
        options: ScopeOptions,
        
        /// CID of the policy update proposal
        #[arg(long)]
        proposal_cid: String,
    },
    
    /// View DAG thread for a scope
    #[command(name = "dag-view")]
    DagView {
        /// The scope type (cooperative or community)
        #[arg(long)]
        scope_type: String,
        
        /// Scope ID (cooperative ID or community ID)
        #[arg(long)]
        scope_id: String,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        output: String,
        
        /// Maximum number of results to show
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    
    /// Inspect policy for a scope
    #[command(name = "inspect-policy")]
    InspectPolicy {
        /// The scope type (cooperative or community)
        #[arg(long)]
        scope_type: String,
        
        /// Scope ID (cooperative ID or community ID)
        #[arg(long)]
        scope_id: String,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        output: String,
        
        /// Maximum number of results to show
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    
    /// View governance activity log for a scope
    #[command(name = "activity-log")]
    ActivityLog {
        /// The scope type (cooperative or community)
        #[arg(long)]
        scope_type: String,
        
        /// Scope ID (cooperative ID or community ID)
        #[arg(long)]
        scope_id: String,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        output: String,
        
        /// Maximum number of results to show
        #[arg(long, default_value = "50")]
        limit: usize,
    },
}

pub async fn handle_scope_command(command: &ScopeCommands, ctx: &mut CliContext) -> CliResult<()> {
    match command {
        ScopeCommands::Create { options, description } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
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
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
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
                        let cid = get_cid(&node)?;
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
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
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
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
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
                        let cid = get_cid(&node)?;
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
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            
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
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Get the federation genesis node
            let federation_nodes = dag_store.get_nodes_by_payload_type("FederationGenesis").await?;
            if federation_nodes.is_empty() {
                return Err(CliError::SerializationError(
                    format!("Federation '{}' not found", options.federation_id)));
            }
            
            let federation_genesis = &federation_nodes[0];
            let federation_genesis_cid = get_cid(federation_genesis)?;
            
            // Get the scope genesis node
            let genesis_type = match scope_type {
                ScopeType::Cooperative => "CooperativeGenesis",
                ScopeType::Community => "CommunityGenesis",
            };
            
            let scope_nodes = dag_store.get_nodes_by_payload_type(genesis_type).await?;
            let mut scope_genesis = None;
            
            for node in scope_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == &options.scope_id {
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
                    return Err(CliError::SerializationError(
                        format!("{} '{}' not found", scope_name, options.scope_id)));
                },
            };
            
            let scope_genesis_cid = get_cid(&scope_genesis)?;
            
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
        
        ScopeCommands::SetPolicy { options, policy_file } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read policy configuration from file
            let policy_str = fs::read_to_string(policy_file)
                .map_err(|e| CliError::IoError(format!("Failed to read policy file: {}", e)))?;
            
            // Parse policy from JSON
            let mut policy_config: icn_types::ScopePolicyConfig = serde_json::from_str(&policy_str)
                .map_err(|e| CliError::SerializationError(format!("Failed to parse policy: {}", e)))?;
            
            // Override scope_type and scope_id to ensure they match options
            policy_config.scope_type = scope_type.to_node_scope();
            policy_config.scope_id = options.scope_id.clone();
            
            // Create a policy node in the DAG
            let node_type = match scope_type {
                ScopeType::Cooperative => "CooperativePolicy",
                ScopeType::Community => "CommunityPolicy",
            };
            
            // Create payload with the policy
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "policy": serde_json::to_value(&policy_config).unwrap(),
                "createdAt": chrono::Utc::now().to_rfc3339(),
                "author": did.to_string(),
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
                        let cid = get_cid(&node)?;
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
            
            println!("Created policy for {} '{}' with CID {}", 
                scope_name, options.scope_id, cid);
            
            Ok(())
        },
        
        ScopeCommands::ProposeUpdatePolicy { options, policy_file, description } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Read policy configuration from file
            let policy_str = fs::read_to_string(policy_file)
                .map_err(|e| CliError::IoError(format!("Failed to read policy file: {}", e)))?;
            
            // Parse policy from JSON
            let policy_config: icn_types::ScopePolicyConfig = serde_json::from_str(&policy_str)
                .map_err(|e| CliError::SerializationError(format!("Failed to parse policy: {}", e)))?;
            
            // Create a policy update proposal node
            let node_type = "PolicyUpdateProposal";
            
            // Convert the policy to a JSON string
            let policy_json = serde_json::to_string(&policy_config)
                .map_err(|e| CliError::SerializationError(format!("Failed to serialize policy to JSON: {}", e)))?;
            
            // Create the payload
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "scope_type": format!("{:?}", scope_type.to_node_scope()),
                "scope_id": options.scope_id,
                "proposed_policy": policy_json,
                "proposer_did": did.to_string(),
                "description": description,
                "proposed_at": chrono::Utc::now().to_rfc3339(),
            }));
            
            // Get parent nodes from the current scope
            let search_type = match scope_type {
                ScopeType::Cooperative => "Cooperative",
                ScopeType::Community => "Community",
            };
            
            let scope_nodes = dag_store.get_nodes_by_payload_type(search_type).await?;
            let mut parent_cids = Vec::new();
            
            for node in scope_nodes {
                if let Some(scope) = node.node.metadata.scope_id.as_ref() {
                    if scope == &options.scope_id {
                        let cid = get_cid(&node)?;
                        parent_cids.push(cid);
                    }
                }
            }
            
            // Build and sign the node
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
            
            println!("Created policy update proposal for {} '{}' with CID {}", 
                scope_name, options.scope_id, cid);
            
            Ok(())
        },
        
        ScopeCommands::VoteUpdatePolicy { options, proposal_cid, vote, reason } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the proposal CID
            let proposal_cid_obj = cid_from_string(proposal_cid)?;
            
            // Retrieve the proposal to verify it exists
            let proposal_node = dag_store.get_node(&proposal_cid_obj).await?;
            
            // Create a vote node
            let node_type = "PolicyUpdateVote";
            
            // Create the payload
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "proposal_cid": proposal_cid,
                "choice": vote,
                "reason": reason,
                "voter_did": did.to_string(),
                "voted_at": chrono::Utc::now().to_rfc3339(),
            }));
            
            // Build and sign the node
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
            
            println!("Recorded vote '{}' on policy update proposal {} with CID {}", 
                vote, proposal_cid, cid);
            
            Ok(())
        },
        
        ScopeCommands::FinalizeUpdatePolicy { options, proposal_cid } => {
            let scope_type = ScopeType::from_str(&options.scope_type)?;
            let mut dag_store = ctx.get_dag_store(options.dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(&options.key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the proposal CID
            let proposal_cid_obj = cid_from_string(proposal_cid)?;
            
            // Retrieve the proposal
            let proposal_node = dag_store.get_node(&proposal_cid_obj).await?;
            
            // Get all votes for this proposal
            let all_nodes = dag_store.get_ordered_nodes().await?;
            let mut votes = Vec::new();
            
            for node in all_nodes {
                if let DagPayload::Json(payload) = &node.node.payload {
                    if let Some(node_type) = payload.get("type").and_then(|t| t.as_str()) {
                        if node_type == "PolicyUpdateVote" {
                            if let Some(vote_proposal_cid) = payload.get("proposal_cid").and_then(|c| c.as_str()) {
                                if vote_proposal_cid == proposal_cid {
                                    votes.push(node);
                                }
                            }
                        }
                    }
                }
            }
            
            // Simple quorum check (in a real implementation, this would be more sophisticated)
            if votes.len() < 3 {
                return Err(CliError::SerializationError(
                    format!("Not enough votes to approve policy update. Need at least 3, got {}", votes.len())
                ));
            }
            
            // Count approvals
            let mut approvals = 0;
            for vote in &votes {
                if let DagPayload::Json(payload) = &vote.node.payload {
                    if let Some(choice) = payload.get("choice").and_then(|c| c.as_str()) {
                        if choice == "approve" {
                            approvals += 1;
                        }
                    }
                }
            }
            
            // Check if majority approved
            if approvals * 2 <= votes.len() {
                return Err(CliError::SerializationError(
                    format!("Policy update not approved. Need majority, got {}/{}", approvals, votes.len())
                ));
            }
            
            // Create a simplified QuorumProof (in a real implementation, this would include signatures, etc.)
            let quorum_proof = icn_types::receipts::QuorumProof {
                content_cid: proposal_cid_obj.clone(),
                signatures: votes.iter().map(|v| {
                    let did = v.node.author.clone();
                    // This is a simplified version; in a real implementation, proper signatures would be collected
                    let signature_bytes = vec![]; // Empty signature as placeholder
                    (did, signature_bytes)
                }).collect(),
            };
            
            // Create the approval node
            let node_type = "PolicyUpdateApproval";
            
            // Create the payload
            let payload = DagPayload::Json(json!({
                "type": node_type,
                "proposal_cid": proposal_cid,
                "quorum_proof": {
                    "content_cid": quorum_proof.content_cid.to_string(),
                    "signatures": quorum_proof.signatures.iter().map(|(did, _)| did.to_string()).collect::<Vec<_>>(),
                    "approved": true,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "federation_id": options.federation_id,
                    "issuer": did.to_string()
                },
                "approver_did": did.to_string(),
                "approved_at": chrono::Utc::now().to_rfc3339(),
            }));
            
            // Build and sign the node
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
            
            println!("Policy update finalized with approval CID {}", cid);
            println!("This policy update will be applied by all nodes processing this DAG");
            
            Ok(())
        },
        
        ScopeCommands::DagView { scope_type, scope_id, dag_dir, output, limit } => {
            let options = ScopeObservabilityOptions {
                scope_type: scope_type.clone(),
                scope_id: scope_id.clone(),
                dag_dir: dag_dir.clone(),
                output: output.clone(),
                limit: *limit,
            };
            handle_dag_view(ctx, &options).await
        },
        
        ScopeCommands::InspectPolicy { scope_type, scope_id, dag_dir, output, limit } => {
            let options = ScopeObservabilityOptions {
                scope_type: scope_type.clone(),
                scope_id: scope_id.clone(),
                dag_dir: dag_dir.clone(),
                output: output.clone(),
                limit: *limit,
            };
            handle_inspect_policy(ctx, &options).await
        },
        
        ScopeCommands::ActivityLog { scope_type, scope_id, dag_dir, output, limit } => {
            let options = ScopeObservabilityOptions {
                scope_type: scope_type.clone(),
                scope_id: scope_id.clone(),
                dag_dir: dag_dir.clone(),
                output: output.clone(),
                limit: *limit,
            };
            handle_activity_log(ctx, &options).await
        },
    }
}

// Helper function to parse a CID from a string
fn cid_from_string(cid_str: &str) -> CliResult<icn_types::Cid> {
    icn_types::Cid::from_bytes(cid_str.as_bytes())
        .map_err(|e| CliError::SerializationError(format!("Invalid CID: {}", e)))
} 