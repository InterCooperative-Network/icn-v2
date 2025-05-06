use crate::cid::Cid;
use crate::identity::Did;
// use crate::quorum::QuorumProof; // Comment out unused import for now
use chrono::{DateTime, Utc};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};

/// Represents the result and proof of a WASM execution within the runtime.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExecutionReceipt {
    /// Content ID of the executed WASM module.
    pub module_cid: Cid,
    /// Content ID of the input data provided to the execution.
    pub input_cid: Option<Cid>,
    /// Content ID of the resulting state or output data.
    pub output_cid: Option<Cid>,
    /// DID of the entity that initiated the execution.
    pub actor: Did,
    /// Timestamp of when the execution completed.
    pub timestamp: DateTime<Utc>,
    /// Signatures from runtime nodes confirming the execution result.
    /// This might evolve into a full QuorumProof.
    pub signatures: Vec<(Did, Signature)>,
    /// Status or result code (e.g., success, error, fuel_exhausted).
    pub status: i32, // Consider using an enum
    /// Optional error message if execution failed.
    pub error_message: Option<String>,
} 