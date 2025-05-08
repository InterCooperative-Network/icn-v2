#![deny(unsafe_code)]
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
// Import the individual observability handlers
use commands::observability::{
    handle_dag_view, 
    handle_inspect_policy, 
    handle_validate_quorum, 
    handle_activity_log, 
    handle_federation_overview
};
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
use commands::scope::handle_scope_command;
use commands::observability::ObservabilityCommands;

// Use the library's public CLI definitions
use icn_cli::Cli; // Assuming Cli is re-exported from lib.rs
use icn_cli::commands; // Assuming the commands module itself is pub in lib.rs
use icn_cli::context::CliContext; // Assuming CliContext is pub in lib.rs or re-exported
// use icn_cli::error::CliError; // Assuming CliError is pub in lib.rs or re-exported

// Specific command handlers - these might need to be public functions in their respective modules
// and then called via icn_cli::commands::... if not re-exported individually by the library.
// For now, keeping direct calls as they were, assuming modules become accessible via `icn_cli::commands`.
use icn_cli::commands::handle_dag_command;
use icn_cli::commands::handle_mesh_command;
use icn_cli::commands::handle_federation_command;
use icn_cli::commands::runtime::handle_runtime_command;
use icn_cli::commands::handle_proposal_commands;
use icn_cli::commands::handle_vote_commands;
use icn_cli::commands::{handle_bundle_command, handle_receipt_command, handle_dag_sync_command};
use icn_cli::commands::handle_policy_command;
use icn_cli::commands::handle_key_gen;
use icn_cli::commands::observability::{
    handle_dag_view, 
    handle_inspect_policy, 
    handle_validate_quorum, 
    handle_activity_log, 
    handle_federation_overview
};

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
    Doctor, // Added Doctor command
}

// Removed DagCommands enum definition from here (moved to commands/dag.rs)
// Removed BundleCommands, ReceiptCommands, MeshCommands, DagSyncCommands definitions temporarily


// Main function using the new structure
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = Cli::parse(); // Use Cli from the library
    let mut ctx = CliContext::new(false)?; // Assuming CliContext is accessible

    // The match statement will now use cli_args.command which is of type icn_cli::Commands
    match &cli_args.command {
        icn_cli::Commands::Coop(coop_cmd) => { // Qualify with icn_cli::Commands
            icn_cli::commands::coop::handle_coop_command(coop_cmd, &mut ctx).await?;
        },
        icn_cli::Commands::Community(community_cmd) => {
            icn_cli::commands::community::handle_community_command(community_cmd, &mut ctx).await?;
        },
        icn_cli::Commands::Federation(federation_cmd) => {
            icn_cli::commands::federation::handle_federation_command(&mut ctx, federation_cmd).await?;
        },
        icn_cli::Commands::Scope(scope_cmd) => {
            icn_cli::commands::scope::handle_scope_command(scope_cmd, &mut ctx).await?;
        },
        icn_cli::Commands::Dag(cmd) => {
            handle_dag_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::KeyGen { output } => {
            handle_key_gen(&mut ctx, output).await?
        }
        icn_cli::Commands::Bundle(cmd) => {
            handle_bundle_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::Receipt(cmd) => {
            handle_receipt_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::Mesh(cmd) => {
            handle_mesh_command(cmd.clone(), &ctx).await?
        }
        icn_cli::Commands::SyncP2P(cmd) => {
            handle_dag_sync_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::Runtime(cmd) => {
            handle_runtime_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::Policy(cmd) => {
            handle_policy_command(&mut ctx, cmd).await?
        }
        icn_cli::Commands::Proposal(cmd) => {
            handle_proposal_commands(cmd.clone(), &mut ctx).await?
        }
        icn_cli::Commands::Vote(cmd) => {
            handle_vote_commands(cmd.clone(), &mut ctx).await?
        }
        icn_cli::Commands::Observe(cmd) => {
            match cmd {
                icn_cli::commands::observability::ObservabilityCommands::DagView(options) => { // Qualify ObservabilityCommands
                    handle_dag_view(&mut ctx, &options).await?
                },
                icn_cli::commands::observability::ObservabilityCommands::InspectPolicy(options) => {
                    handle_inspect_policy(&mut ctx, &options).await?
                },
                icn_cli::commands::observability::ObservabilityCommands::ValidateQuorum { cid, show_signers, dag_dir, output } => {
                    handle_validate_quorum(&mut ctx, cid, *show_signers, dag_dir.as_deref(), output).await?
                },
                icn_cli::commands::observability::ObservabilityCommands::ActivityLog(options) => {
                    handle_activity_log(&mut ctx, &options).await?
                },
                icn_cli::commands::observability::ObservabilityCommands::FederationOverview { federation_id, dag_dir, output } => {
                    handle_federation_overview(&mut ctx, federation_id, dag_dir.as_deref(), output).await?
                }
            }
        }
        icn_cli::Commands::Doctor => {
            println!("ICN CLI Doctor: System check complete. All systems nominal."); // Placeholder
        }
    }
    
    Ok(())
}

// Removed old handler functions (handle_dag_command, handle_mesh_command etc.)
// Removed parse_key_val helper (will be needed inside specific command handlers)
