use anyhow::{Result, anyhow};
use wasmtime::Caller;
use icn_types::{Did, PolicyError};
use crate::abi::context::HostContext;
use crate::policy::{MembershipIndex, PolicyLoader, ScopeType};
use log::{debug, error};

/// Host function to check if a DID is authorized to perform an action within a scope
///
/// # Parameters
/// * `scope_type` - The scope type (Federation, Cooperative, Community)
/// * `scope_id` - The ID of the scope
/// * `action` - The action being performed
/// * `did` - The DID to validate
///
/// # Returns
/// * `0` if the action is authorized
/// * Non-zero error code if the action is not authorized
pub fn host_check_policy_authorization(
    mut caller: Caller<'_, impl HostContext>,
    scope_type_ptr: i32,
    scope_type_len: i32,
    scope_id_ptr: i32,
    scope_id_len: i32,
    action_ptr: i32,
    action_len: i32,
    did_ptr: i32,
    did_len: i32,
) -> i32 {
    let ctx = caller.data();
    
    // First read all strings to avoid multiple mutable borrows of caller
    let scope_type_result = ctx.read_string(&mut caller, scope_type_ptr, scope_type_len);
    let scope_id_result = ctx.read_string(&mut caller, scope_id_ptr, scope_id_len);
    let action_result = ctx.read_string(&mut caller, action_ptr, action_len);
    let did_str_result = ctx.read_string(&mut caller, did_ptr, did_len);
    
    // Now process each result separately
    let scope_type = match scope_type_result {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read scope_type: {}", e);
            return -1;
        }
    };
    
    let scope_id = match scope_id_result {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read scope_id: {}", e);
            return -2;
        }
    };
    
    let action = match action_result {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read action: {}", e);
            return -3;
        }
    };
    
    let did_str = match did_str_result {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read did: {}", e);
            return -4;
        }
    };
    
    let did = match Did::try_from(did_str) {
        Ok(d) => d,
        Err(e) => {
            error!("Invalid DID format: {}", e);
            return -5;
        }
    };
    
    // Now we can safely access policy_loader from the context
    let ctx = caller.data();
    let policy_loader = match ctx.policy_loader() {
        Some(loader) => loader,
        None => {
            error!("Policy loader not available");
            return -6;
        }
    };
    
    debug!("Checking authorization for {} to perform {} in {}/{}", did, action, scope_type, scope_id);
    
    // Perform the policy check
    match policy_loader.check_authorization(&scope_type, &scope_id, &action, &did) {
        Ok(()) => 0, // Authorized
        Err(PolicyError::Unauthorized) => 1,
        Err(PolicyError::InvalidScope) => 2,
        Err(PolicyError::InvalidAction) => 3,
        Err(e) => {
            error!("Policy check error: {}", e);
            -10
        }
    }
} 