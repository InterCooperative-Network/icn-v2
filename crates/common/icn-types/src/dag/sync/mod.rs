// Re-export core sync types
pub use super::sync::{DAGSyncBundle, DAGSyncService, FederationPeer, SyncError, VerificationResult};

// Include the memory-based implementation
pub mod memory;

// Include the transport layer
pub mod transport;

// Include the network-based implementation
pub mod network;

// Re-export transport types
pub use transport::{DAGSyncMessage, DAGSyncTransport, TransportConfig};

// Re-export network types
pub use network::{NetworkDagSyncService, SyncPolicy};

// Tests module
#[cfg(test)]
mod tests; 