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

/// Main library entry point.
pub async fn run(cli: Cli) -> CliResult<()> {
    let mut ctx = CliContext::new(cli.global_opts.verbose > 0)?;

    // Match on commands defined in cli.rs and call handlers from commands::*
    match cli.command {
        Commands::Coop(cmd) => commands::coop::handle_coop_command(&cmd, &mut ctx).await?,
        Commands::Receipt(cmd) => commands::receipt::handle_receipt_command(&mut ctx, &cmd).await?,
        Commands::Mesh(cmd) => commands::mesh::handle_mesh_command(cmd, &ctx).await?,
        Commands::SyncP2P(cmd) => commands::sync_p2p::handle_dag_sync_command(&mut ctx, &cmd).await?,
        Commands::Community(cmd) => commands::community::handle_community_command(&cmd, &mut ctx).await?,
        Commands::Federation(cmd) => commands::federation::handle_federation_command(&mut ctx, &cmd).await?,
        Commands::Scope(cmd) => commands::scope::handle_scope_command(&cmd, &mut ctx).await?,
        Commands::Dag(cmd) => commands::dag::handle_dag_command(&mut ctx, &cmd).await?,
        Commands::KeyGen { output } => commands::keygen::handle_key_gen(&mut ctx, output.as_deref()).await?,
        Commands::Bundle(cmd) => commands::bundle::handle_bundle_command(&mut ctx, &cmd).await?,
        Commands::Runtime(cmd) => commands::runtime::handle_runtime_command(&mut ctx, &cmd).await?,
        Commands::Policy(cmd) => commands::policy::handle_policy_command(&mut ctx, &cmd).await?,
        Commands::Proposal(cmd) => commands::proposal::handle_proposal_commands(cmd, &mut ctx).await?,
        Commands::Vote(cmd) => commands::vote::handle_vote_commands(cmd, &mut ctx).await?,
        Commands::Observe(cmd) => commands::observability::handle_observability_command(&mut ctx, cmd).await?,
        Commands::Doctor => println!("ICN CLI Doctor: System check complete. All systems nominal."),
        Commands::GenCliDocs(cmd) => commands::gen_cli_docs::generate_cli_docs::<Cli>(&cmd)?,

        #[cfg(feature = "agora")]
        Commands::Agora(cmd) => commands::agora::handle_agora_cmd(cmd).await?,
    }

    Ok(())
}
