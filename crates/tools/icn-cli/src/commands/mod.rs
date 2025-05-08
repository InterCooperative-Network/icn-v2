pub mod dag;
pub mod mesh;
pub mod bundle;
pub mod receipt;
pub mod sync_p2p;
pub mod federation;
pub mod runtime;
pub mod proposal;
pub mod vote;
pub mod policy;
pub mod keygen;
pub mod coop;
pub mod community;
pub mod scope;
pub mod observability;
pub mod doctor;
pub mod gen_cli_docs;

#[cfg(feature = "agora")]
pub mod agora;

// Re-export handlers for main.rs
pub use dag::handle_dag_command;
pub use mesh::handle_mesh_command;
pub use federation::handle_federation_command;
pub use proposal::handle_proposal_commands;
pub use vote::handle_vote_commands;
// Add pub use for other handlers when created
pub use bundle::handle_bundle_command;
pub use receipt::handle_receipt_command;
pub use sync_p2p::handle_dag_sync_command;
pub use policy::handle_policy_command;
pub use keygen::handle_key_gen;
pub use runtime::handle_runtime_command;
pub use gen_cli_docs::generate_cli_docs;

#[cfg(feature = "agora")]
pub use agora::handle_agora_cmd;

// Re-export Command structs for cli.rs
pub use dag::DagCommands;
pub use mesh::MeshCommands;
pub use bundle::BundleCommands;
pub use receipt::ReceiptCommands;
pub use sync_p2p::DagSyncCommands;
pub use federation::FederationCommands;
pub use runtime::RuntimeCommands;
pub use proposal::ProposalCommands;
pub use vote::VoteCommands;
pub use policy::PolicyCommands;
pub use coop::CoopCommands;
pub use community::CommunityCommands;
pub use scope::ScopeCommands;
pub use observability::ObservabilityCommands;
pub use gen_cli_docs::GenCliDocsCmd;

#[cfg(feature = "agora")]
pub use agora::AgoraCmd;

// Export coop module components
pub use coop::handle_coop_command;

// Export community module components
pub use community::handle_community_command;

// Export scope module components
pub use scope::handle_scope_command;

// Export observability module components
pub use observability::{handle_inspect_policy, handle_validate_quorum, handle_activity_log, handle_federation_overview, handle_dag_view};