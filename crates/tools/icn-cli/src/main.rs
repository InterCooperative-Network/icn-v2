//! Placeholder for icn-cli binary

// Use anyhow temporarily for non-refactored commands
// use anyhow::{Context, Result};

// New structure imports
mod error;
mod context;
mod commands;
// mod metrics; // Commented out
use clap::{Parser, Subcommand};
use context::CliContext;
use error::CliError;
use commands::handle_dag_command; // Import the specific handler
use commands::handle_mesh_command; // Add handle_mesh_command
use commands::handle_federation_command; // Add federation handler
use commands::runtime::handle_runtime_command; // 👈 NEW
use commands::handle_proposal_commands; // Add proposal handler
use commands::handle_vote_commands; // Add vote handler
use commands::{handle_bundle_command, handle_receipt_command, handle_dag_sync_command}; // ADDED
use commands::handle_policy_command; // Add policy handler import
use commands::handle_key_gen; // Add keygen handler
use commands::handle_observability_command; // Add observability handler
// use icn_types::ExecutionResult; // Needs locating
use std::path::PathBuf;
use tokio;

// Add command handlers
use commands::coop::CoopCommands;
use commands::coop::handle_coop_command;
use commands::community::CommunityCommands;
use commands::community::handle_community_command;
use commands::federation::FederationCommands;
use commands::scope::ScopeCommands;
use commands::ObservabilityCommands;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    // Global flags moved here
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

// Placeholder structs for non-refactored commands
// These will be moved to their respective command modules later
// #[derive(Subcommand, Debug, Clone)] enum BundleCommands { Temp } // REMOVED
// #[derive(Subcommand, Debug, Clone)] enum ReceiptCommands { Temp } // REMOVED
// #[derive(Subcommand, Debug, Clone)] enum DagSyncCommands { Temp } // REMOVED

#[derive(Subcommand, Debug)] // Added Debug
enum Commands {
    /// Generate a new DID key
    #[command(name = "key-gen")]
    KeyGen {
        /// Output file to save the key (defaults to ~/.icn/key.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// DAG commands
    #[command(subcommand)]
    Dag(commands::dag::DagCommands), // Use type from commands::dag module

    /// TrustBundle commands
    #[command(subcommand)]
    Bundle(commands::bundle::BundleCommands), // Updated path

    /// ExecutionReceipt commands
    #[command(subcommand)]
    Receipt(commands::receipt::ReceiptCommands), // Updated path
    
    /// Advanced DAG sync commands with libp2p support
    #[command(subcommand)]
    SyncP2P(commands::sync_p2p::DagSyncCommands), // Updated path

    /// Interact with the ICN mesh network (libp2p)
    #[command(subcommand)]
    Mesh(commands::mesh::MeshCommands),

    /// Federation management commands
    #[command(subcommand)]
    Federation(FederationCommands),

    /// Manage trust policies
    #[command(subcommand)]
    Policy(commands::policy::PolicyCommands),

    /// Runtime commands
    #[command(subcommand)]
    Runtime(commands::runtime::RuntimeCommands),
    
    /// Governance proposal commands
    #[command(subcommand)]
    Proposal(commands::proposal::ProposalCommands),
    
    /// Voting commands
    #[command(subcommand)]
    Vote(commands::vote::VoteCommands),

    /// Cooperative commands
    #[command(subcommand)]
    Coop(CoopCommands),
    
    /// Community commands
    #[command(subcommand)]
    Community(CommunityCommands),

    /// Generic scope commands (works with both cooperatives and communities)
    #[command(subcommand)]
    Scope(ScopeCommands),
    
    /// Observability commands for federation transparency
    #[command(subcommand)]
    Observe(ObservabilityCommands),
}

// Removed DagCommands enum definition from here (moved to commands/dag.rs)
// Removed BundleCommands, ReceiptCommands, MeshCommands, DagSyncCommands definitions temporarily


// Main function using the new structure
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut ctx = CliContext::new(false)?;

    match &cli.command {
        Commands::Coop(coop_cmd) => {
            commands::coop::handle_coop_command(coop_cmd, &mut ctx).await?;
        },
        Commands::Community(community_cmd) => {
            commands::community::handle_community_command(community_cmd, &mut ctx).await?;
        },
        Commands::Federation(federation_cmd) => {
            commands::federation::handle_federation_command(federation_cmd, &mut ctx).await?;
        },
        Commands::Scope(scope_cmd) => {
            commands::scope::handle_scope_command(scope_cmd, &mut ctx).await?;
        },
        Commands::Dag(cmd) => {
            handle_dag_command(&mut ctx, cmd).await?
        }
        Commands::KeyGen { output } => {
            handle_key_gen(&mut ctx, output).await?
        }
        Commands::Bundle(cmd) => {
            handle_bundle_command(&mut ctx, cmd).await?
        }
         Commands::Receipt(cmd) => {
            handle_receipt_command(&mut ctx, cmd).await?
        }
        Commands::Mesh(cmd) => {
            handle_mesh_command(cmd.clone(), &ctx).await?
        }
         Commands::SyncP2P(cmd) => {
            handle_dag_sync_command(&mut ctx, cmd).await?
        }
        Commands::Runtime(cmd) => {
            handle_runtime_command(&mut ctx, cmd).await?
        }
        Commands::Policy(cmd) => {
            handle_policy_command(&mut ctx, cmd).await?
        }
        Commands::Proposal(cmd) => {
            handle_proposal_commands(cmd.clone(), &mut ctx).await?
        }
        Commands::Vote(cmd) => {
            handle_vote_commands(cmd.clone(), &mut ctx).await?
        }
        Commands::Observe(cmd) => {
            handle_observability_command(cmd, &mut ctx).await?
        }
    }
    
    Ok(())
}

// Removed old handler functions (handle_dag_command, handle_mesh_command etc.)
// Removed parse_key_val helper (will be needed inside specific command handlers)
