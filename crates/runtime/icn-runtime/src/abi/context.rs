use async_trait::async_trait;
use icn_types::Did;
use std::sync::Arc;
use crate::policy::{MembershipIndex, PolicyLoader};
use anyhow::{Result, anyhow};
use wasmtime::Caller;

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
    
    /// Read string from WASM memory
    fn read_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, len: i32) -> Result<String>;
    
    /// Write string to WASM memory
    fn write_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, max_len: i32, s: &str) -> Result<i32>;
    
    /// Allocate memory in WASM module
    fn malloc(&self, caller: &mut impl wasmtime::AsContextMut, size: i32) -> Result<i32>;
    
    /// Free memory in WASM module
    fn free(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32) -> Result<()>;
    
    /// Set an error message
    fn set_error(&self, message: String);
    
    /// Get the last error message
    fn get_error(&self) -> Option<String>;
    
    /// Clear the last error message
    fn clear_error(&self);
    
    /// Get the policy loader for checking policy rules
    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>>;
    
    /// Get the membership index for checking federation/cooperative/community memberships
    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>>;

    // TODO: Add more host functions as needed:
    // - DAG operations (get_node, anchor_data)
    // - Resource access (storage_read, storage_write)
    // - Economic actions (transfer_resource, check_balance)
    // - Time access (get_current_time)
}

/// Common implementation of read_string for HostContext implementors
pub fn read_wasm_string(caller: &mut impl wasmtime::AsContextMut, ptr: i32, len: i32) -> Result<String> {
    let mut caller_ctx = caller.as_context_mut();
    let memory = match caller_ctx.get_export("memory") {
        Some(export) => export.into_memory(),
        None => return Err(anyhow!("No memory export found")),
    };
    
    let memory = match memory {
        Some(mem) => mem,
        None => return Err(anyhow!("Export is not a memory")),
    };
    
    let mem_slice = memory
        .data_mut(&mut caller_ctx)
        .get_mut(ptr as usize..(ptr + len) as usize)
        .ok_or_else(|| anyhow!("Memory access out of bounds"))?;
    
    let s = String::from_utf8(mem_slice.to_vec())
        .map_err(|e| anyhow!("Invalid UTF-8 string: {}", e))?;
    
    Ok(s)
} 