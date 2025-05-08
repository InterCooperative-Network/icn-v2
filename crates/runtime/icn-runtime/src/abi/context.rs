use icn_types::Did;
use std::sync::Arc;
use crate::policy::{MembershipIndex, PolicyLoader};
use anyhow::Result;
use wasmtime;

/// Context for host functions that can be called from WASM modules
/// 
/// This trait provides access to contextual information and capabilities
/// needed by host functions
#[async_trait::async_trait]
pub trait HostContext: Send + Sync + Clone {
    /// Read string from WASM memory
    fn read_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, len: i32) -> Result<String>;
    
    /// Write string to WASM memory
    fn write_string(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32, max_len: i32, s: &str) -> Result<i32>;
    
    /// Allocate memory in WASM module
    fn malloc(&self, caller: &mut impl wasmtime::AsContextMut, size: i32) -> Result<i32>;
    
    /// Free memory in WASM module
    fn free(&self, caller: &mut impl wasmtime::AsContextMut, ptr: i32) -> Result<()>;
    
    /// Get caller DID
    fn get_caller_did(&self) -> Did;
    
    /// Log message
    fn log_message(&self, message: &str);
    
    /// Verify signature
    async fn verify_signature(&self, did: &Did, message: &[u8], signature: &[u8]) -> bool;
    
    /// Set error message
    fn set_error(&self, message: String);
    
    /// Get error message
    fn get_error(&self) -> Option<String>;
    
    /// Clear error message
    fn clear_error(&self);
    
    /// Get policy loader
    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>>;
    
    /// Get membership index
    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>>;

    // TODO: Add more host functions as needed:
    // - DAG operations (get_node, anchor_data)
    // - Resource access (storage_read, storage_write)
    // - Economic actions (transfer_resource, check_balance)
    // - Time access (get_current_time)
} 