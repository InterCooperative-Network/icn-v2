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

// Import command structs from the commands module
// Use fully qualified paths to potentially help clap resolve traits.
// use crate::commands::*; // Removed wildcard import

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(flatten)]
    pub global_opts: GlobalOpts,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Args, Debug)]
pub struct GlobalOpts {
     #[clap(short, long, action = clap::ArgAction::Count, global = true)]
     pub verbose: u8,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Coop(crate::commands::coop::CoopCommands),
    Receipt(crate::commands::receipt::ReceiptCommands),
    Mesh(crate::commands::mesh::MeshCommands),
    SyncP2P(crate::commands::sync_p2p::DagSyncCommands),
    Community(crate::commands::community::CommunityCommands),
    Federation(crate::commands::federation::FederationCommands),
    Scope(crate::commands::scope::ScopeCommands),
    Dag(crate::commands::dag::DagCommands),
    KeyGen { output: Option<std::path::PathBuf> },
    Bundle(crate::commands::bundle::BundleCommands),
    Runtime(crate::commands::runtime::RuntimeCommands),
    Policy(crate::commands::policy::PolicyCommands),
    Proposal(crate::commands::proposal::ProposalCommands),
    Vote(crate::commands::vote::VoteCommands),
    Observe(crate::commands::observability::ObservabilityCommands),
    Doctor,
    GenCliDocs(crate::commands::gen_cli_docs::GenCliDocsCmd),
    
    /// Interact with AgoraNet deliberation threads.
    #[cfg(feature = "agora")]
    Agora(crate::commands::agora::AgoraCmd),
} 