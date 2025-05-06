// Re-export core sync types
pub use super::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};

// Include the memory-based implementation
pub mod memory;

// Tests module
#[cfg(test)]
mod tests; 