#![allow(missing_docs)] // TODO: Remove this once docs are added
use clap::{Parser, Subcommand};
use std::path::PathBuf;

// Re-export command modules if they are not public or are complex
// For now, assuming they are structured to be found or made public as needed.
// If commands::dag::DagCommands etc. are private, they need to be pub in their respective mods
// or re-exported here.

// NOTE: This requires that the modules like `commands::dag`, `commands::mesh` etc.
// make their `*Commands` structs public.
// E.g., in `commands/dag.rs` it should be `pub struct DagCommands { ... }`

pub mod commands {
    // This is a placeholder to satisfy the paths used in the Commands enum below.
    // The actual command modules (commands::dag, commands::mesh, etc.)
    // need to be correctly structured and made public from the icn_cli library root (lib.rs)
    // so that `icn_cli::commands::...` resolves.
    // For simplicity in this step, I am assuming these paths will be made available
    // through `pub mod commands;` in `lib.rs` and then `pub mod dag;` etc. within `commands/mod.rs`

    // This is a simplified view. The actual modules must be made public.
    pub use crate::commands::bundle;
    pub use crate::commands::coop;
    pub use crate::commands::community;
    pub use crate::commands::dag;
    pub use crate::commands::federation;
    pub use crate::commands::keygen; // Assuming key_gen might have its own struct/enum
    pub use crate::commands::mesh;
    pub use crate::commands::observability;
    pub use crate::commands::policy;
    pub use crate::commands::proposal;
    pub use crate::commands::receipt;
    pub use crate::commands::runtime;
    pub use crate::commands::scope;
    pub use crate::commands::sync_p2p;
    pub use crate::commands::vote;
}

#[derive(Parser, Debug)] // Added Debug for gen_clap_docs if needed
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate a new DID key
    #[command(name = "key-gen")]
    KeyGen {
        /// Output file to save the key (defaults to ~/.icn/key.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// DAG commands
    #[command(subcommand)]
    Dag(commands::dag::DagCommands),

    /// TrustBundle commands
    #[command(subcommand)]
    Bundle(commands::bundle::BundleCommands),

    /// ExecutionReceipt commands
    #[command(subcommand)]
    Receipt(commands::receipt::ReceiptCommands),
    
    /// Advanced DAG sync commands with libp2p support
    #[command(subcommand)]
    SyncP2P(commands::sync_p2p::DagSyncCommands),

    /// Interact with the ICN mesh network (libp2p)
    #[command(subcommand)]
    Mesh(commands::mesh::MeshCommands),

    /// Federation management commands
    #[command(subcommand)]
    Federation(commands::federation::FederationCommands),

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
    Coop(commands::coop::CoopCommands),
    
    /// Community commands
    #[command(subcommand)]
    Community(commands::community::CommunityCommands),

    /// Generic scope commands (works with both cooperatives and communities)
    #[command(subcommand)]
    Scope(commands::scope::ScopeCommands),
    
    /// Observability commands for federation transparency
    #[command(subcommand)]
    Observe(commands::observability::ObservabilityCommands),
    Doctor,
} 