use wasmtime::Caller;
use icn_types::{Did, PolicyError};
use crate::abi::context::HostContext;
use log::{debug, error};

/// Helper function to read a string from WASM memory that also handles clone to avoid borrow checker issues
fn read_string_safe<T: HostContext + Clone>(
    caller: &mut Caller<'_, T>,
    ptr: i32,
    len: i32,
    error_code: i32
) -> Result<String, i32> {
    let context = caller.data().clone();
    match context.read_string(caller, ptr, len) {
        Ok(s) => Ok(s),
        Err(e) => {
            error!("Failed to read string: {}", e);
            Err(error_code)
        }
    }
}

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
pub fn host_check_policy_authorization<T: HostContext + Clone>(
    mut caller: Caller<'_, T>,
    scope_type_ptr: i32,
    scope_type_len: i32,
    scope_id_ptr: i32,
    scope_id_len: i32,
    action_ptr: i32,
    action_len: i32,
    did_ptr: i32,
    did_len: i32,
) -> i32 {
    // Read scope_type
    let scope_type = match read_string_safe(&mut caller, scope_type_ptr, scope_type_len, -1) {
        Ok(s) => s,
        Err(code) => return code,
    };
    
    // Read scope_id
    let scope_id = match read_string_safe(&mut caller, scope_id_ptr, scope_id_len, -2) {
        Ok(s) => s,
        Err(code) => return code,
    };
    
    // Read action
    let action = match read_string_safe(&mut caller, action_ptr, action_len, -3) {
        Ok(s) => s,
        Err(code) => return code,
    };
    
    // Read DID string
    let did_str = match read_string_safe(&mut caller, did_ptr, did_len, -4) {
        Ok(s) => s,
        Err(code) => return code,
    };
    
    // Parse the DID
    let did = match Did::try_from(did_str) {
        Ok(d) => d,
        Err(e) => {
            error!("Invalid DID format: {}", e);
            return -5;
        }
    };
    
    // We need to clone the context to avoid borrowing issues
    let ctx = caller.data().clone();
    
    // Get the policy loader
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
        Err(PolicyError::ActionNotPermitted) => 1,
        Err(PolicyError::UnauthorizedScopeAccess) => 2,
        Err(PolicyError::DidNotInAllowlist) => 3,
        Err(PolicyError::PolicyNotFound) => 4,
        Err(PolicyError::InternalError(_)) => 5,
    }
} 