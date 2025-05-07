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
    mut caller: Caller<'_, HostContext>,
    scope_type_ptr: i32,
    scope_type_len: i32,
    scope_id_ptr: i32,
    scope_id_len: i32,
    action_ptr: i32,
    action_len: i32,
    did_ptr: i32,
    did_len: i32,
) -> i32 {
    // Get host context
    let ctx = caller.data_mut();
    
    // Read arguments from WASM memory
    let scope_type = match ctx.read_string(&mut caller, scope_type_ptr, scope_type_len) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read scope_type from WASM memory: {}", e);
            return 1;
        }
    };
    
    let scope_id = match ctx.read_string(&mut caller, scope_id_ptr, scope_id_len) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read scope_id from WASM memory: {}", e);
            return 2;
        }
    };
    
    let action = match ctx.read_string(&mut caller, action_ptr, action_len) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read action from WASM memory: {}", e);
            return 3;
        }
    };
    
    let did_str = match ctx.read_string(&mut caller, did_ptr, did_len) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read DID from WASM memory: {}", e);
            return 4;
        }
    };
    
    // Parse the DID
    let did = match Did::from_string(&did_str) {
        Ok(did) => did,
        Err(_) => {
            error!("Failed to parse DID: {}", did_str);
            return 5;
        }
    };
    
    debug!("Checking policy authorization for action '{}' in {}/{} for DID {}", 
        action, scope_type, scope_id, did_str);
    
    // Get policy loader and membership index from context
    let policy_loader = match ctx.policy_loader() {
        Some(loader) => loader,
        None => {
            error!("Policy loader not available in host context");
            return 6;
        }
    };
    
    let membership_index = match ctx.membership_index() {
        Some(index) => index,
        None => {
            error!("Membership index not available in host context");
            return 7;
        }
    };
    
    // Load policy for this scope
    let policy = match policy_loader.load_for_scope(&scope_type, &scope_id) {
        Ok(policy) => policy,
        Err(PolicyError::PolicyNotFound) => {
            // No policy defined for this scope, allow the operation
            debug!("No policy found for scope {}/{}, allowing operation", scope_type, scope_id);
            return 0;
        }
        Err(err) => {
            error!("Policy error: {}", err);
            return 8;
        }
    };
    
    // Evaluate policy
    match crate::policy::evaluate_policy(&policy, &action, &did, &membership_index) {
        Ok(()) => 0, // Authorized
        Err(PolicyError::ActionNotPermitted) => 10,
        Err(PolicyError::UnauthorizedScopeAccess) => 11,
        Err(PolicyError::DidNotInAllowlist) => 12,
        Err(err) => {
            error!("Unexpected policy error: {}", err);
            20
        }
    }
} 