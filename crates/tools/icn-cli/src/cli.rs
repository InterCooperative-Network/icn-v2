#![allow(missing_docs)] // TODO: Remove this once docs are added
use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

// Re-export command modules if they are not public or are complex
// For now, assuming they are structured to be found or made public as needed.
// If commands::dag::DagCommands etc. are private, they need to be pub in their respective mods
// or re-exported here.

// NOTE: This requires that the modules like `commands::dag`, `commands::mesh` etc.
// make their `*Commands` structs public.
// E.g., in `commands/dag.rs` it should be `pub struct DagCommands { ... }`

// Import command structs using fully qualified paths
// No longer using wildcard import here.

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(flatten)]
    pub global_opts: GlobalOpts,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Args, Debug)] // GlobalOpts should derive Args
pub struct GlobalOpts {
     #[clap(short, long, action = clap::ArgAction::Count, global = true)]
     pub verbose: u8,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Manage Cooperatives
    Coop { #[clap(subcommand)] command: crate::commands::coop::CoopCommands },
    /// Manage Receipts
    Receipt { #[clap(subcommand)] command: crate::commands::receipt::ReceiptCommands },
    /// Manage Planetary Mesh
    Mesh { #[clap(subcommand)] command: crate::commands::mesh::MeshCommands },
    /// Manage P2P DAG Sync
    SyncP2P { #[clap(subcommand)] command: crate::commands::sync_p2p::DagSyncCommands },
    /// Manage Communities
    Community { #[clap(subcommand)] command: crate::commands::community::CommunityCommands },
    /// Manage Federations
    Federation { #[clap(subcommand)] command: crate::commands::federation::FederationCommands },
    /// Manage Scopes (Coops/Communities)
    Scope { #[clap(subcommand)] command: crate::commands::scope::ScopeCommands },
    /// Manage DAGs
    Dag { #[clap(subcommand)] command: crate::commands::dag::DagCommands },
    /// Manage Keys
    KeyGen { output: Option<std::path::PathBuf> },
    /// Manage Bundles
    Bundle { #[clap(subcommand)] command: crate::commands::bundle::BundleCommands },
    /// Manage Runtimes
    Runtime { #[clap(subcommand)] command: crate::commands::runtime::RuntimeCommands },
    /// Manage Policies
    Policy { #[clap(subcommand)] command: crate::commands::policy::PolicyCommands },
    /// Manage Proposals
    Proposal { #[clap(subcommand)] command: crate::commands::proposal::ProposalCommands },
    /// Manage Votes
    Vote { #[clap(subcommand)] command: crate::commands::vote::VoteCommands },
    /// Observe system state
    Observe { #[clap(subcommand)] command: crate::commands::observability::ObservabilityCommands },
    /// Run diagnostics
    Doctor,
    /// Generate CLI documentation
    GenCliDocs(crate::commands::gen_cli_docs::GenCliDocsCmd), // This holds an Args struct now
    
    /// Interact with AgoraNet deliberation threads.
    #[cfg(feature = "agora")]
    Agora { #[clap(subcommand)] command: crate::commands::agora::AgoraSubcommand },
} 