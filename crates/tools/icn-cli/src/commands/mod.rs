pub mod dag;
pub mod mesh;
pub mod bundle;
pub mod receipt;
pub mod sync_p2p;
pub mod federation;
pub mod runtime;
pub mod proposal;
pub mod vote;

// Re-export handlers for main.rs
pub use dag::handle_dag_command;
pub use mesh::handle_mesh_command;
pub use federation::handle_federation_command;
pub use proposal::handle_proposal_commands;
pub use vote::handle_vote_commands;
// Add pub use for other handlers when created
// pub use bundle::handle_bundle_command;
// pub use receipt::handle_receipt_command;
// pub use sync_p2p::handle_dag_sync_command; 