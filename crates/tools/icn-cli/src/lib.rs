//! icn-cli placeholder

#![allow(missing_docs)] // TODO: Remove this once docs are added

pub mod cli;
pub mod commands;
pub mod context;
pub mod error;
pub mod config;
// pub mod metrics; // If needed

// Re-export key types
pub use cli::{Cli, Commands, GlobalOpts}; // Assuming these are defined in cli.rs
pub use context::CliContext;
pub use error::CliResult;
pub use crate::commands::ObservabilityCommands;

/// Main library entry point.
pub async fn run(cli: Cli) -> CliResult<()> {
    let mut ctx = CliContext::new(cli.global_opts.verbose > 0)?;

    // Match on commands defined in cli.rs and call handlers from commands::*
    match cli.command {
        Commands::Coop { command } => commands::handle_coop_command(&command, &mut ctx).await?,
        Commands::Receipt { command } => commands::handle_receipt_command(&mut ctx, &command).await?,
        Commands::Mesh { command } => commands::handle_mesh_command(command, &ctx).await?,
        Commands::SyncP2P { command } => commands::handle_dag_sync_command(&mut ctx, &command).await?,
        Commands::Community { command } => commands::handle_community_command(&command, &mut ctx).await?,
        Commands::Federation { command } => commands::handle_federation_command(&mut ctx, &command).await?,
        Commands::Scope { command } => commands::handle_scope_command(&command, &mut ctx).await?,
        Commands::Dag { command } => commands::handle_dag_command(&mut ctx, &command).await?,
        Commands::KeyGen { output } => commands::handle_key_gen(&mut ctx, &output).await?,
        Commands::Bundle { command } => commands::handle_bundle_command(&mut ctx, &command).await?,
        Commands::Runtime { command } => commands::handle_runtime_command(&mut ctx, &command).await?,
        Commands::Policy { command } => commands::handle_policy_command(&mut ctx, &command).await?,
        Commands::Proposal { command } => commands::handle_proposal_commands(command, &mut ctx).await?,
        Commands::Vote { command } => commands::handle_vote_commands(command, &mut ctx).await?,
        Commands::Observe { command } => match command {
            ObservabilityCommands::DagView(options) => 
                commands::observability::handle_dag_view(&mut ctx, &options).await?,
            ObservabilityCommands::InspectPolicy(options) => 
                commands::observability::handle_inspect_policy(&mut ctx, &options).await?,
            ObservabilityCommands::ValidateQuorum { cid, show_signers, dag_dir, output } => 
                commands::observability::handle_validate_quorum(&mut ctx, &cid, show_signers, dag_dir.as_deref(), &output).await?,
            ObservabilityCommands::ActivityLog(options) => 
                commands::observability::handle_activity_log(&mut ctx, &options).await?,
            ObservabilityCommands::FederationOverview { federation_id, dag_dir, output } => 
                commands::observability::handle_federation_overview(&mut ctx, &federation_id, dag_dir.as_deref(), &output).await?,
        },
        Commands::Doctor => println!("ICN CLI Doctor: System check complete. All systems nominal."),
        Commands::GenCliDocs(cmd) => commands::generate_cli_docs::<Cli>(&cmd)?,

        #[cfg(feature = "agora")]
        Commands::Agora { command } => commands::handle_agora_cmd(command).await?,
    }

    Ok(())
}
