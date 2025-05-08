mod policy_inspector;
mod quorum_validator;
mod activity_log;
mod federation_overview;

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::Path;
use icn_types::dag::NodeScope;
use clap::{Args, Subcommand, ValueHint};
use std::path::PathBuf;

// Re-export the types and functions we need
pub use policy_inspector::inspect_policy;
pub use quorum_validator::validate_quorum;
pub use activity_log::get_activity_log;
pub use federation_overview::get_federation_overview;

/// Observability options
#[derive(Debug, Args, Clone)]
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
#[derive(Debug, Subcommand, Clone)]
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

/// Handle DAG view command
pub async fn handle_dag_view(ctx: &mut CliContext, options: &ScopeObservabilityOptions) -> CliResult<()> {
    let dag_store = ctx.get_dag_store(options.dag_dir.as_ref().map(|p| p.as_path()))?;
    let scope_type = parse_scope_type(&options.scope_type)?;
    let scope_id = Some(options.scope_id.as_str());
    
    // We need to implement DAGInspector here
    // For now, just return a placeholder
    println!("DAG view for {} {}", options.scope_type, options.scope_id);
    
    Ok(())
}

/// Handle policy inspection command
pub async fn handle_inspect_policy(ctx: &mut CliContext, options: &ScopeObservabilityOptions) -> CliResult<()> {
    let scope_type = parse_scope_type(&options.scope_type)?;
    let scope_id = Some(options.scope_id.as_str());
    
    policy_inspector::inspect_policy(
        ctx, 
        scope_type, 
        scope_id, 
        options.dag_dir.as_ref().map(|p| p.as_path()),
        &options.output
    ).await
}

/// Handle quorum validation command
pub async fn handle_validate_quorum(
    ctx: &mut CliContext, 
    cid_str: &str, 
    show_signers: bool, 
    dag_dir: Option<&Path>, 
    output: &str
) -> CliResult<()> {
    quorum_validator::validate_quorum(
        ctx,
        cid_str,
        show_signers,
        dag_dir,
        output
    ).await
}

/// Handle activity log command
pub async fn handle_activity_log(ctx: &mut CliContext, options: &ScopeObservabilityOptions) -> CliResult<()> {
    let scope_type = parse_scope_type(&options.scope_type)?;
    let scope_id = Some(options.scope_id.as_str());
    
    activity_log::get_activity_log(
        ctx,
        scope_type,
        scope_id,
        options.dag_dir.as_ref().map(|p| p.as_path()),
        options.limit,
        &options.output
    ).await
}

/// Handle federation overview command
pub async fn handle_federation_overview(
    ctx: &mut CliContext, 
    federation_id: &str, 
    dag_dir: Option<&Path>, 
    output: &str
) -> CliResult<()> {
    federation_overview::get_federation_overview(
        ctx,
        federation_id,
        dag_dir,
        output
    ).await
}

/// Convert scope type string to NodeScope enum
fn parse_scope_type(scope_type: &str) -> Result<NodeScope, CliError> {
    match scope_type.to_lowercase().as_str() {
        "cooperative" | "coop" => Ok(NodeScope::Cooperative),
        "community" => Ok(NodeScope::Community),
        "federation" => Ok(NodeScope::Federation),
        _ => Err(CliError::SerializationError(format!("Invalid scope type: {}", scope_type))),
    }
} 