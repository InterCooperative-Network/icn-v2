use wasmtime::{Linker, Caller, Memory, AsContextMut};
use crate::abi::context::HostContext;
use crate::host::policy::host_check_policy_authorization;
use std::sync::Arc;
use anyhow::anyhow;

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
pub fn register_host_functions<T: HostContext + Send + Sync + 'static>(
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
        |caller: Caller<'_, Arc<T>>, scope_type_ptr: i32, scope_type_len: i32, 
         scope_id_ptr: i32, scope_id_len: i32, action_ptr: i32, action_len: i32,
         did_ptr: i32, did_len: i32| -> i32 {
            host_check_policy_authorization(
                caller,
                scope_type_ptr,
                scope_type_len,
                scope_id_ptr,
                scope_id_len,
                action_ptr,
                action_len,
                did_ptr,
                did_len
            )
        }
    )?;
    
    // TODO: Add more host functions here
    
    Ok(())
}
