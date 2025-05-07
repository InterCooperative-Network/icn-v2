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
pub use observability::handle_observability_command;

// Export coop module components
pub use coop::CoopCommands;
pub use coop::handle_coop_command;

// Export community module components
pub use community::CommunityCommands;
pub use community::handle_community_command;

// Export scope module components
pub use scope::ScopeCommands;
pub use scope::handle_scope_command;

// Export observability module components
pub use observability::ObservabilityCommands; 