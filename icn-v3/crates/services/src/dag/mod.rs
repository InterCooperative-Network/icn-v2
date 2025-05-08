//! DAG storage and verification services
//! 
//! This module provides persistent storage for DAG nodes, allowing
//! for verifiable lineage traversal and replay verification.

mod storage;
mod verifier;
#[cfg(test)]
mod tests;

pub use storage::{DagStorage, DagStorageBackend, RocksDbDagStorage, DagMetadata};
pub use verifier::{
    DagReplayVerifier, DefaultDagReplayVerifier, 
    LineageVerificationResult, LineageVerificationError
}; 