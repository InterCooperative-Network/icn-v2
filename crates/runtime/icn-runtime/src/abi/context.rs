use async_trait::async_trait;
use icn_types::Did;
use std::sync::Arc;
use crate::policy::{MembershipIndex, PolicyLoader};
use anyhow::Result;

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
    
    /// Helper function to read a string from WASM memory
    fn read_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, len: i32) -> Result<String> {
        let memory = match caller.get_export("memory") {
            Some(export) => export.into_memory(),
            None => return Err(anyhow::anyhow!("No memory export found")),
        };
        
        let memory = match memory {
            Some(mem) => mem,
            None => return Err(anyhow::anyhow!("Export is not a memory")),
        };
        
        let mem_slice = memory
            .data_mut(caller)
            .get_mut(ptr as usize..(ptr + len) as usize)
            .ok_or_else(|| anyhow::anyhow!("Pointer/length out of bounds"))?;
            
        String::from_utf8(mem_slice.to_vec())
            .map_err(|_| anyhow::anyhow!("Invalid UTF-8 sequence in memory"))
    }
    
    /// Get the policy loader, if available
    fn policy_loader(&self) -> Option<Arc<PolicyLoader>> {
        None
    }
    
    /// Get the membership index, if available
    fn membership_index(&self) -> Option<Arc<MembershipIndex>> {
        None
    }

    // TODO: Add more host functions as needed:
    // - DAG operations (get_node, anchor_data)
    // - Resource access (storage_read, storage_write)
    // - Economic actions (transfer_resource, check_balance)
    // - Time access (get_current_time)
} 