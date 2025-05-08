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

// Import command structs from the commands module
use crate::commands::*;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(flatten)
    pub global_opts: GlobalOpts,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Parser, Debug)]
pub struct GlobalOpts {
     #[clap(short, long, action = clap::ArgAction::Count, global = true)]
     pub verbose: u8,
}

#[derive(Subcommand, Debug, Clone)] // Added Clone
pub enum Commands {
    Coop(coop::CoopCmd),
    Receipt(receipt::ReceiptCmd),
    Mesh(mesh::MeshCmd),
    SyncP2P(sync_p2p::SyncP2PCmd),
    Community(community::CommunityCmd),
    Federation(federation::FederationCmd),
    Scope(scope::ScopeCmd),
    Dag(dag::DagCmd),
    KeyGen { output: Option<std::path::PathBuf> },
    Bundle(bundle::BundleCmd),
    Runtime(runtime::RuntimeCmd),
    Policy(policy::PolicyCmd),
    Proposal(proposal::ProposalCommands),
    Vote(vote::VoteCommands),
    Observe(observability::ObservabilityCommands),
    Doctor,
    GenCliDocs(gen_cli_docs::GenCliDocsCmd),
    
    /// Interact with AgoraNet deliberation threads.
    #[cfg(feature = "agora")]
    Agora(agora::AgoraCmd), // NEW
} 