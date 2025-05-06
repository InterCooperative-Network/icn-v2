//! DAG Synchronization Logic

pub mod network;
pub mod transport;

// Re-export key types from submodules
pub use network::{DAGSyncService, FederationPeer, SyncError, VerificationResult};
// Assuming DAGSyncBundle might be defined elsewhere or needs adjustment
// pub use transport::{DAGSyncTransport, TransportConfig}; // Example if needed

// Include the memory-based implementation
pub mod memory;

// Re-export transport types
pub use transport::{DAGSyncMessage, DAGSyncTransport, TransportConfig};

// Re-export network types
pub use network::{NetworkDagSyncService, SyncPolicy};

// Tests module
#[cfg(test)]
mod tests;