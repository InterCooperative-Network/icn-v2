#![deny(unsafe_code)]
#![warn(missing_docs)] // Good practice for a new foundational crate

//! Core types and logic for AgoraNet, the ICN Deliberation Layer.
//! 
//! Provides structures for messages, threads, storage, and error handling.

/// Error types for AgoraNet operations.
pub mod error;
/// Defines the core Message structure and body content types.
pub mod message;
/// Defines the AsyncStorage trait for pluggable storage backends.
pub mod storage;
/// Defines the AgoraThread structure and associated operations.
pub mod thread;
/// Placeholder for forwarding anchors to the federation layer.
pub mod forwarder;

// Re-exports based on the battle-plan hint
// These will cause errors until the types are defined in their respective modules.
pub use message::{Message, Body as MessageBody, ThreadAnchor}; // Renamed Body to avoid conflict if thread::Body exists
pub use thread::{AgoraThread, Proposal, ThreadOperations}; // Added ThreadOperations trait
pub use forwarder::forward_anchor;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
