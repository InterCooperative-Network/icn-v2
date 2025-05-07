use clap::{Args, Subcommand, ValueHint};
use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::PathBuf;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use icn_types::dag::{DagNodeMetadata, DagPayload, NodeScope, SignedDagNode};
use icn_types::{Cid, Did};
use chrono::{DateTime, Utc};
use serde_json::json;
use colored::*;

/// Observability command options
#[derive(Debug, Args)]
pub struct ObservabilityOptions {
    /// Optional path to DAG storage directory
    #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
    
    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    pub output: String,
    
    /// Maximum number of results to show
    #[arg(long, default_value = "50")]
    pub limit: usize,
}

/// Scope-specific observability options
#[derive(Debug, Args)]
pub struct ScopeObservabilityOptions {
    /// Scope type (cooperative, community, or federation)
    #[arg(long)]
    pub scope_type: String,
    
    /// Scope ID (cooperative ID, community ID, or federation ID)
    #[arg(long)]
    pub scope_id: String,
    
    /// Optional path to DAG storage directory
    #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
    pub dag_dir: Option<PathBuf>,
    
    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    pub output: String,
    
    /// Maximum number of results to show
    #[arg(long, default_value = "50")]
    pub limit: usize,
}

/// Observability commands
#[derive(Debug, Subcommand)]
pub enum ObservabilityCommands {
    /// View DAG thread for a specific scope
    #[command(name = "dag-view")]
    DagView(ScopeObservabilityOptions),
    
    /// Inspect policy for a specific scope
    #[command(name = "inspect-policy")]
    InspectPolicy(ScopeObservabilityOptions),
    
    /// Validate quorum proof for a DAG node
    #[command(name = "validate-quorum")]
    ValidateQuorum {
        /// CID of the DAG node to validate quorum for
        #[arg(long)]
        cid: String,
        
        /// Show signer details
        #[arg(long)]
        show_signers: bool,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        output: String,
    },
    
    /// View governance activity log for a specific scope
    #[command(name = "activity-log")]
    ActivityLog(ScopeObservabilityOptions),
    
    /// View overview of a federation
    #[command(name = "federation-overview")]
    FederationOverview {
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
        
        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        output: String,
    },
}

/// DAG node information for display
#[derive(Debug)]
struct DagNodeInfo {
    cid: Cid,
    timestamp: DateTime<Utc>,
    signer_did: Did,
    payload_type: String,
    payload_preview: String,
    parent_cids: Vec<Cid>,
    scope_type: NodeScope,
    scope_id: Option<String>,
    federation_id: String,
}

impl DagNodeInfo {
    fn from_signed_node(signed_node: &SignedDagNode, cid: &Cid) -> Self {
        let payload_type = match &signed_node.node.payload {
            DagPayload::Raw(_) => "Raw".to_string(),
            DagPayload::Json(json) => {
                if let Some(t) = json.get("type").and_then(|v| v.as_str()) {
                    t.to_string()
                } else {
                    "Json".to_string()
                }
            },
            DagPayload::Reference(_) => "Reference".to_string(),
            DagPayload::TrustBundle(_) => "TrustBundle".to_string(),
            DagPayload::ExecutionReceipt(_) => "ExecutionReceipt".to_string(),
        };
        
        let payload_preview = match &signed_node.node.payload {
            DagPayload::Raw(data) => format!("<{} bytes>", data.len()),
            DagPayload::Json(json) => {
                if let Some(t) = json.get("type").and_then(|v| v.as_str()) {
                    // Special handling for known payload types
                    match t {
                        "Proposal" => {
                            if let Some(title) = json.get("title").and_then(|v| v.as_str()) {
                                format!("Proposal: {}", title)
                            } else {
                                "Proposal".to_string()
                            }
                        },
                        "Vote" => {
                            if let Some(vote) = json.get("vote").and_then(|v| v.as_str()) {
                                format!("Vote: {}", vote)
                            } else {
                                "Vote".to_string()
                            }
                        },
                        "PolicyUpdateApproval" => "Policy Update Approval".to_string(),
                        _ => format!("{}", t),
                    }
                } else {
                    json.to_string().chars().take(30).collect::<String>() + "..."
                }
            },
            DagPayload::Reference(ref_cid) => format!("Reference to {}", ref_cid),
            DagPayload::TrustBundle(bundle_cid) => format!("TrustBundle {}", bundle_cid),
            DagPayload::ExecutionReceipt(receipt_cid) => format!("ExecutionReceipt {}", receipt_cid),
        };
        
        DagNodeInfo {
            cid: cid.clone(),
            timestamp: signed_node.node.metadata.timestamp,
            signer_did: signed_node.node.author.clone(),
            payload_type,
            payload_preview,
            parent_cids: signed_node.node.parents.clone(),
            scope_type: signed_node.node.metadata.scope.clone(),
            scope_id: signed_node.node.metadata.scope_id.clone(),
            federation_id: signed_node.node.metadata.federation_id.clone(),
        }
    }
    
    fn to_json(&self) -> serde_json::Value {
        json!({
            "cid": self.cid.to_string(),
            "timestamp": self.timestamp,
            "signer_did": self.signer_did.to_string(),
            "payload_type": self.payload_type,
            "payload_preview": self.payload_preview,
            "parent_cids": self.parent_cids.iter().map(|cid| cid.to_string()).collect::<Vec<String>>(),
            "scope_type": format!("{:?}", self.scope_type),
            "scope_id": self.scope_id.clone(),
            "federation_id": self.federation_id.clone(),
        })
    }
}

/// DAG Inspector utility
pub struct DAGInspector {
    dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>,
}

impl DAGInspector {
    /// Create a new DAG inspector
    pub fn new(dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>) -> Self {
        DAGInspector { dag_store }
    }
    
    /// Get DAG nodes for a specific scope
    pub async fn get_scope_nodes(
        &self, 
        scope_type: NodeScope, 
        scope_id: Option<&str>,
    ) -> Result<Vec<(Cid, SignedDagNode)>, CliError> {
        let all_nodes = self.dag_store.get_ordered_nodes().await
            .map_err(CliError::Dag)?;
        
        let filtered_nodes = all_nodes.into_iter()
            .filter(|node| {
                // Filter by scope type
                node.node.metadata.scope == scope_type 
                // Filter by scope ID if provided
                && match (scope_id, &node.node.metadata.scope_id) {
                    (Some(id), Some(node_id)) => id == node_id,
                    (Some(_), None) => false,
                    (None, _) => true,
                }
            })
            .map(|node| {
                // Calculate CID if not already calculated
                let cid = if let Some(cid) = &node.cid {
                    cid.clone()
                } else {
                    // This error should not happen if the node is retrieved from the store
                    node.calculate_cid().map_err(CliError::Dag)?
                };
                Ok((cid, node))
            })
            .collect::<Result<Vec<_>, CliError>>()?;
        
        Ok(filtered_nodes)
    }
    
    /// Render DAG nodes as text
    pub fn render_text(&self, nodes: &[(Cid, SignedDagNode)], limit: usize) -> String {
        let mut output = String::new();
        
        let display_nodes = nodes.iter()
            .take(limit)
            .map(|(cid, node)| DagNodeInfo::from_signed_node(node, cid))
            .collect::<Vec<_>>();
        
        for node_info in display_nodes {
            output.push_str(&format!("\n{}\n", "=".repeat(80)));
            output.push_str(&format!("CID: {}\n", node_info.cid.to_string().green()));
            output.push_str(&format!("Timestamp: {}\n", node_info.timestamp));
            output.push_str(&format!("Signer: {}\n", node_info.signer_did.to_string().yellow()));
            output.push_str(&format!("Payload Type: {}\n", node_info.payload_type.blue()));
            output.push_str(&format!("Payload: {}\n", node_info.payload_preview));
            output.push_str(&format!("Scope: {:?}", node_info.scope_type));
            if let Some(scope_id) = &node_info.scope_id {
                output.push_str(&format!(" ({})", scope_id));
            }
            output.push('\n');
            output.push_str("Parents: ");
            if node_info.parent_cids.is_empty() {
                output.push_str("(Genesis Node)");
            } else {
                for (i, parent) in node_info.parent_cids.iter().enumerate() {
                    if i > 0 {
                        output.push_str(", ");
                    }
                    output.push_str(&parent.to_string()[0..16]);
                    output.push_str("...");
                }
            }
            output.push('\n');
        }
        
        if nodes.len() > limit {
            output.push_str(&format!("\nDisplaying {} of {} nodes. Use --limit to show more.\n", limit, nodes.len()));
        }
        
        output
    }
    
    /// Render DAG nodes as JSON
    pub fn render_json(&self, nodes: &[(Cid, SignedDagNode)], limit: usize) -> String {
        let display_nodes = nodes.iter()
            .take(limit)
            .map(|(cid, node)| DagNodeInfo::from_signed_node(node, cid))
            .collect::<Vec<_>>();
        
        let json_nodes = display_nodes.iter()
            .map(|node| node.to_json())
            .collect::<Vec<_>>();
        
        let response = json!({
            "total_nodes": nodes.len(),
            "displayed_nodes": json_nodes.len(),
            "nodes": json_nodes
        });
        
        serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Error generating JSON".to_string())
    }
}

/// Handle observability commands
pub async fn handle_observability_command(
    command: &ObservabilityCommands,
    ctx: &mut CliContext,
) -> CliResult<()> {
    match command {
        ObservabilityCommands::DagView(_) => {
            // Delegate to the module handler
            crate::commands::observability::handle_dag_view(ctx, command).await
        },
        ObservabilityCommands::InspectPolicy(_) => {
            // Delegate to the module handler
            crate::commands::observability::handle_inspect_policy(ctx, command).await
        },
        ObservabilityCommands::ValidateQuorum { cid, show_signers, dag_dir, output } => {
            // Delegate to the module handler
            crate::commands::observability::handle_validate_quorum(
                ctx, 
                cid, 
                *show_signers, 
                dag_dir.as_ref().map(|p| p.as_path()), 
                output
            ).await
        },
        ObservabilityCommands::ActivityLog(_) => {
            // Delegate to the module handler
            crate::commands::observability::handle_activity_log(ctx, command).await
        },
        ObservabilityCommands::FederationOverview { federation_id, dag_dir, output } => {
            // Delegate to the module handler
            crate::commands::observability::handle_federation_overview(
                ctx, 
                federation_id, 
                dag_dir.as_ref().map(|p| p.as_path()), 
                output
            ).await
        },
    }
}

/// Convert scope type string to NodeScope enum
fn parse_scope_type(scope_type: &str) -> Result<NodeScope, CliError> {
    match scope_type.to_lowercase().as_str() {
        "cooperative" | "coop" => Ok(NodeScope::Cooperative),
        "community" => Ok(NodeScope::Community),
        "federation" => Ok(NodeScope::Federation),
        _ => Err(CliError::ValidationError(format!("Invalid scope type: {}", scope_type))),
    }
} 