mod policy_inspector;
mod quorum_validator;
mod activity_log;
mod federation_overview;

use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::path::Path;
use icn_types::dag::NodeScope;

// Re-export the types and functions we need
pub use policy_inspector::inspect_policy;
pub use quorum_validator::validate_quorum;
pub use activity_log::get_activity_log;
pub use federation_overview::get_federation_overview;

/// Handle DAG view command
pub async fn handle_dag_view(ctx: &mut CliContext, options: &super::ObservabilityCommands) -> CliResult<()> {
    if let super::ObservabilityCommands::DagView(scope_options) = options {
        let dag_store = ctx.get_dag_store(scope_options.dag_dir.as_ref().map(|p| p.as_path()))?;
        let scope_type = parse_scope_type(&scope_options.scope_type)?;
        let scope_id = Some(scope_options.scope_id.as_str());
        
        let dag_inspector = super::DAGInspector::new(dag_store);
        let nodes = dag_inspector.get_scope_nodes(scope_type, scope_id).await?;
        
        if nodes.is_empty() {
            println!("No DAG nodes found for the specified scope.");
            return Ok(());
        }
        
        match scope_options.output.to_lowercase().as_str() {
            "json" => {
                println!("{}", dag_inspector.render_json(&nodes, scope_options.limit));
            },
            _ => {
                println!("{}", dag_inspector.render_text(&nodes, scope_options.limit));
            }
        }
        
        Ok(())
    } else {
        Err(CliError::ValidationError("Invalid command".to_string()))
    }
}

/// Handle policy inspection command
pub async fn handle_inspect_policy(ctx: &mut CliContext, options: &super::ObservabilityCommands) -> CliResult<()> {
    if let super::ObservabilityCommands::InspectPolicy(scope_options) = options {
        let scope_type = parse_scope_type(&scope_options.scope_type)?;
        let scope_id = Some(scope_options.scope_id.as_str());
        
        policy_inspector::inspect_policy(
            ctx, 
            scope_type, 
            scope_id, 
            scope_options.dag_dir.as_ref().map(|p| p.as_path()),
            &scope_options.output
        ).await
    } else {
        Err(CliError::ValidationError("Invalid command".to_string()))
    }
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
pub async fn handle_activity_log(ctx: &mut CliContext, options: &super::ObservabilityCommands) -> CliResult<()> {
    if let super::ObservabilityCommands::ActivityLog(scope_options) = options {
        let scope_type = parse_scope_type(&scope_options.scope_type)?;
        let scope_id = Some(scope_options.scope_id.as_str());
        
        activity_log::get_activity_log(
            ctx,
            scope_type,
            scope_id,
            scope_options.dag_dir.as_ref().map(|p| p.as_path()),
            scope_options.limit,
            &scope_options.output
        ).await
    } else {
        Err(CliError::ValidationError("Invalid command".to_string()))
    }
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
        _ => Err(CliError::ValidationError(format!("Invalid scope type: {}", scope_type))),
    }
} 