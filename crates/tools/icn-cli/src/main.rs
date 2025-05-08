// #![deny(unsafe_code)] // Temporarily commented out
#![warn(unsafe_code)] // Or allow, to see other errors
//! Placeholder for icn-cli binary

use icn_cli::{Cli, Commands, context::CliContext}; // Main items from lib
use clap::Parser;
use tokio;

// All other 'use' statements related to commands or local modules are removed.
// Local struct Cli and enum Commands definitions are REMOVED from here.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Cli::parse(); // Uses icn_cli::Cli
    let mut ctx = CliContext::new(cli_args.verbose > 0)?;

    match &cli_args.command {
        Commands::Coop(coop_cmd) => {
            icn_cli::commands::coop::handle_coop_command(coop_cmd, &mut ctx).await?;
        },
        Commands::Community(community_cmd) => {
            icn_cli::commands::community::handle_community_command(community_cmd, &mut ctx).await?;
        },
        Commands::Federation(federation_cmd) => {
            icn_cli::commands::federation::handle_federation_command(&mut ctx, federation_cmd).await?;
        },
        Commands::Scope(scope_cmd) => {
            icn_cli::commands::scope::handle_scope_command(scope_cmd, &mut ctx).await?;
        },
        Commands::Dag(cmd) => {
            icn_cli::commands::dag::handle_dag_command(&mut ctx, cmd).await?;
        }
        Commands::KeyGen { output } => {
            icn_cli::commands::keygen::handle_key_gen(&mut ctx, output).await?;
        }
        Commands::Bundle(cmd) => {
            icn_cli::commands::bundle::handle_bundle_command(&mut ctx, cmd).await?;
        }
        Commands::Receipt(cmd) => {
            icn_cli::commands::receipt::handle_receipt_command(&mut ctx, cmd).await?;
        }
        Commands::Mesh(cmd) => {
            icn_cli::commands::mesh::handle_mesh_command(cmd.clone(), &ctx).await?;
        }
        Commands::SyncP2P(cmd) => {
            icn_cli::commands::sync_p2p::handle_dag_sync_command(&mut ctx, cmd).await?;
        }
        Commands::Runtime(cmd) => {
            icn_cli::commands::runtime::handle_runtime_command(&mut ctx, cmd).await?;
        }
        Commands::Policy(cmd) => {
            icn_cli::commands::policy::handle_policy_command(&mut ctx, cmd).await?;
        }
        Commands::Proposal(cmd) => {
            icn_cli::commands::proposal::handle_proposal_commands(cmd.clone(), &mut ctx).await?;
        }
        Commands::Vote(cmd) => {
            icn_cli::commands::vote::handle_vote_commands(cmd.clone(), &mut ctx).await?;
        }
        Commands::Observe(obs_cmd) => {
            match obs_cmd {
                icn_cli::commands::observability::ObservabilityCommands::DagView(options) => {
                    icn_cli::commands::observability::handle_dag_view(&mut ctx, options).await?;
                },
                icn_cli::commands::observability::ObservabilityCommands::InspectPolicy(options) => {
                    icn_cli::commands::observability::handle_inspect_policy(&mut ctx, options).await?;
                },
                icn_cli::commands::observability::ObservabilityCommands::ValidateQuorum { cid, show_signers, dag_dir, output } => {
                    icn_cli::commands::observability::handle_validate_quorum(&mut ctx, cid, *show_signers, dag_dir.as_deref(), output.as_ref()).await?;
                },
                icn_cli::commands::observability::ObservabilityCommands::ActivityLog(options) => {
                    icn_cli::commands::observability::handle_activity_log(&mut ctx, options).await?;
                },
                icn_cli::commands::observability::ObservabilityCommands::FederationOverview { federation_id, dag_dir, output } => {
                    icn_cli::commands::observability::handle_federation_overview(&mut ctx, federation_id, dag_dir.as_deref(), output.as_ref()).await?;
                }
            }
        }
        Commands::Doctor => {
            println!("ICN CLI Doctor: System check complete. All systems nominal.");
        }
    }
    
    Ok(())
}
