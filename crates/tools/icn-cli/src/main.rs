//! Placeholder for icn-cli binary

// Use anyhow temporarily for non-refactored commands
// use anyhow::{Context, Result};

// New structure imports
mod error;
mod context;
mod commands;

use clap::{Parser, Subcommand};
use context::CliContext;
use error::CliError;
use commands::handle_dag_command; // Import the specific handler

// Keep these imports if needed by non-refactored commands/structs
use icn_identity_core::did::DidKey;
use icn_types::dag::{memory::MemoryDagStore, DagError, DagStore, SignedDagNode};
use icn_types::{anchor::AnchorRef, Did, ExecutionReceipt, ExecutionResult, TrustBundle};
use std::path::PathBuf;


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    // Global flags moved here
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

// Placeholder structs for non-refactored commands
// These will be moved to their respective command modules later
#[derive(Subcommand, Debug, Clone)] enum BundleCommands { Temp }
#[derive(Subcommand, Debug, Clone)] enum ReceiptCommands { Temp }
#[derive(Subcommand, Debug, Clone)] enum MeshCommands { Temp }
#[derive(Subcommand, Debug, Clone)] enum DagSyncCommands { Temp }

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
    Bundle(BundleCommands), // Keep placeholder for now

    /// ExecutionReceipt commands
    #[command(subcommand)]
    Receipt(ReceiptCommands), // Keep placeholder for now
    
    /// Mesh computation commands
    #[command(subcommand)]
    Mesh(MeshCommands), // Keep placeholder for now
    
    /// Advanced DAG sync commands with libp2p support
    #[command(subcommand)]
    SyncP2P(DagSyncCommands), // Moved SyncP2P to top level?
}

// Removed DagCommands enum definition from here (moved to commands/dag.rs)
// Removed BundleCommands, ReceiptCommands, MeshCommands, DagSyncCommands definitions temporarily


// Main function using the new structure
#[tokio::main]
async fn main() -> Result<(), CliError> {
    let cli = Cli::parse();

    // Initialize context
    let mut context = CliContext::new(cli.verbose > 0)?;

    // Match top-level command and dispatch
    match &cli.command {
        Commands::Dag(cmd) => {
            handle_dag_command(&mut context, cmd).await?
        }
        Commands::KeyGen { output } => {
            println!("Executing key-gen...");
            // TODO: Implement key-gen logic (could also be moved to commands/keygen.rs)
            unimplemented!("KeyGen handler")
        }
        Commands::Bundle(cmd) => {
            println!("Bundle command placeholder...");
            // TODO: Refactor Bundle commands
            unimplemented!("Bundle handler placeholder")
        }
         Commands::Receipt(cmd) => {
            println!("Receipt command placeholder...");
            // TODO: Refactor Receipt commands
            unimplemented!("Receipt handler placeholder")
        }
         Commands::Mesh(cmd) => {
            println!("Mesh command placeholder...");
            // TODO: Refactor Mesh commands
            unimplemented!("Mesh handler placeholder")
        }
         Commands::SyncP2P(cmd) => {
            println!("SyncP2P command placeholder...");
            // TODO: Refactor SyncP2P commands (into commands/sync_p2p.rs?)
            unimplemented!("SyncP2P handler placeholder")
        }
    }

    Ok(())
}

// Removed old handler functions (handle_dag_command, handle_mesh_command etc.)
// Removed parse_key_val helper (will be needed inside specific command handlers)
