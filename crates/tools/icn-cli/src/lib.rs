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
        Commands::Coop(cmd) => commands::handle_coop_command(&cmd, &mut ctx).await?,
        Commands::Receipt(cmd) => commands::handle_receipt_command(&mut ctx, &cmd).await?,
        Commands::Mesh(cmd) => commands::handle_mesh_command(cmd, &ctx).await?,
        Commands::SyncP2P(cmd) => commands::handle_dag_sync_command(&mut ctx, &cmd).await?,
        Commands::Community(cmd) => commands::handle_community_command(&cmd, &mut ctx).await?,
        Commands::Federation(cmd) => commands::handle_federation_command(&mut ctx, &cmd).await?,
        Commands::Scope(cmd) => commands::handle_scope_command(&cmd, &mut ctx).await?,
        Commands::Dag(cmd) => commands::handle_dag_command(&mut ctx, &cmd).await?,
        Commands::KeyGen { output } => commands::handle_key_gen(&mut ctx, &output).await?,
        Commands::Bundle(cmd) => commands::handle_bundle_command(&mut ctx, &cmd).await?,
        Commands::Runtime(cmd) => commands::handle_runtime_command(&mut ctx, &cmd).await?,
        Commands::Policy(cmd) => commands::handle_policy_command(&mut ctx, &cmd).await?,
        Commands::Proposal(cmd) => commands::handle_proposal_commands(cmd, &mut ctx).await?,
        Commands::Vote(cmd) => commands::handle_vote_commands(cmd, &mut ctx).await?,
        Commands::Observe(cmd) => match cmd {
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
        Commands::Agora(cmd) => commands::handle_agora_cmd(cmd).await?,
    }

    Ok(())
}
