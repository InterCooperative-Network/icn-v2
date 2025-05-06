use async_trait::async_trait;
use icn_types::Did;

/// Trait defining the capabilities provided by the host environment
/// to the WASM guest module.
#[async_trait]
pub trait HostContext: Send + Sync {
    /// Returns the DID of the entity invoking the current WASM execution.
    fn get_caller_did(&self) -> Did;

    /// Logs a message from the WASM guest to the host environment.
    fn log_message(&self, message: &str);

    /// Verifies a signature using the host's cryptographic capabilities.
    /// Returns true if the signature is valid for the given DID and message.
    async fn verify_signature(&self, did: &Did, message: &[u8], signature: &[u8]) -> bool;

    // TODO: Add more host functions as needed:
    // - DAG operations (get_node, anchor_data)
    // - Resource access (storage_read, storage_write)
    // - Economic actions (transfer_resource, check_balance)
    // - Time access (get_current_time)
} 