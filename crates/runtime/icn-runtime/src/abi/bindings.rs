use wasmtime::{Linker, Caller, Memory, AsContextMut};
use crate::abi::context::HostContext;
use std::sync::Arc;
use anyhow::anyhow;
use log;

// Helper function to read a string from WASM memory
fn read_string_from_memory(
    mut caller: impl AsContextMut,
    memory: &Memory,
    ptr: i32,
    len: i32,
) -> anyhow::Result<String> {
    let mem_slice = memory
        .data_mut(&mut caller)
        .get_mut(ptr as usize..(ptr + len) as usize)
        .ok_or_else(|| anyhow!("Pointer/length out of bounds"))?;
    String::from_utf8(mem_slice.to_vec())
        .map_err(|_| anyhow!("Invalid UTF-8 sequence in memory"))
}

/// Registers the ICN host functions with the Wasmtime linker.
pub fn register_host_functions<T: HostContext + 'static>(
    linker: &mut Linker<Arc<T>>,
) -> anyhow::Result<()> {
    // Register log function
    linker.func_wrap("env", "log", |mut caller: Caller<'_, Arc<T>>, ptr: i32, len: i32| {
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            if let Ok(message) = read_string_from_memory(&mut caller, &memory, ptr, len) {
                caller.data().log_message(&message);
            }
        }
    })?;
    
    // Register get_caller_did function
    linker.func_wrap("env", "get_caller_did", |mut caller: Caller<'_, Arc<T>>, ptr: i32, len: i32| -> i32 {
        let caller_did = caller.data().get_caller_did().to_string();
        
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let memory_data = memory.data_mut(&mut caller);
            if (ptr as usize) + caller_did.len() <= memory_data.len() && (ptr as usize) + (len as usize) <= memory_data.len() {
                // Copy the DID string to WASM memory
                let dst = &mut memory_data[ptr as usize..(ptr as usize) + caller_did.len()];
                dst.copy_from_slice(caller_did.as_bytes());
                return caller_did.len() as i32;
            }
        }
        
        // Return 0 on failure
        0
    })?;
    
    // Register policy evaluation function
    linker.func_wrap(
        "env", 
        "check_policy_authorization", 
        move |mut caller: Caller<'_, Arc<T>>, scope_type_ptr: i32, scope_type_len: i32, 
         scope_id_ptr: i32, scope_id_len: i32, action_ptr: i32, action_len: i32,
         did_ptr: i32, did_len: i32| -> i32 {
            // Extract the host context (dereference and clone)
            let context = &**caller.data();
            let cloned_context = context.clone();
            
            // Read strings from memory
            let scope_type = match read_wasm_string_with_context(
                &cloned_context, &mut caller, scope_type_ptr, scope_type_len) 
            {
                Ok(s) => s,
                Err(code) => return code - 1,
            };
            
            let scope_id = match read_wasm_string_with_context(
                &cloned_context, &mut caller, scope_id_ptr, scope_id_len) 
            {
                Ok(s) => s,
                Err(code) => return code - 2,
            };
            
            let action = match read_wasm_string_with_context(
                &cloned_context, &mut caller, action_ptr, action_len) 
            {
                Ok(s) => s,
                Err(code) => return code - 3,
            };
            
            let did_str = match read_wasm_string_with_context(
                &cloned_context, &mut caller, did_ptr, did_len) 
            {
                Ok(s) => s,
                Err(code) => return code - 4,
            };
            
            // Parse the DID
            let did = match icn_types::Did::try_from(did_str) {
                Ok(d) => d,
                Err(e) => {
                    log::error!("Invalid DID format: {}", e);
                    return -5;
                }
            };
            
            // Get the policy loader
            let policy_loader = match cloned_context.policy_loader() {
                Some(loader) => loader,
                None => {
                    log::error!("Policy loader not available");
                    return -6;
                }
            };
            
            log::debug!("Checking authorization for {} to perform {} in {}/{}", 
                     did, action, scope_type, scope_id);
            
            // Perform the policy check
            match policy_loader.check_authorization(&scope_type, &scope_id, &action, &did) {
                Ok(()) => 0, // Authorized
                Err(icn_types::PolicyError::ActionNotPermitted) => 1,
                Err(icn_types::PolicyError::UnauthorizedScopeAccess) => 2,
                Err(icn_types::PolicyError::DidNotInAllowlist) => 3,
                Err(icn_types::PolicyError::PolicyNotFound) => 4,
                Err(icn_types::PolicyError::InternalError(_)) => 5,
            }
        }
    )?;
    
    Ok(())
}

// Helper function to read a string using a cloned context
fn read_wasm_string_with_context<T: HostContext>(
    ctx: &T,
    caller: &mut impl AsContextMut,
    ptr: i32,
    len: i32,
) -> Result<String, i32> {
    match ctx.read_string(caller, ptr, len) {
        Ok(s) => Ok(s),
        Err(e) => {
            log::error!("Failed to read string: {}", e);
            Err(-1)
        }
    }
}
