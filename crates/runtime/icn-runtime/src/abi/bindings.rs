use wasmtime::{Linker, Caller, Memory, AsContextMut};
use crate::abi::context::HostContext;
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
    // --- Logging ---
    linker.func_wrap(
        "icn",
        "host_log",
        move |mut caller: Caller<'_, Arc<T>>, ptr: i32, len: i32| {
            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return,  // Silently fail if memory is missing
            };

            let msg = match read_string_from_memory(caller.as_context_mut(), &memory, ptr, len) {
                Ok(s) => s,
                Err(_) => return,  // Silently fail if reading fails
            };

            // Call the host context method
            caller.data().log_message(&msg);
        },
    )?;

    // --- Identity ---
    linker.func_wrap(
        "icn",
        "host_get_caller_did",
        move |mut caller: Caller<'_, Arc<T>>, ptr: i32, len: i32| -> i32 {
            let ctx = caller.data().clone(); // Clone Arc for context access
            let did = ctx.get_caller_did();
            let did_string = did.to_string(); // Convert to String first
            let did_bytes = did_string.as_bytes(); // Get bytes from the owned String

            if did_bytes.len() > len as usize {
                return -1; // Guest buffer too small
            }

            let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return -1,  // Return error if memory is missing
            };

            match memory.write(&mut caller, ptr as usize, did_bytes) {
                Ok(_) => 0,  // Success
                Err(_) => -1, // Error writing to memory
            }
        },
    )?;

    // TODO: Implement host_get_caller_did_len if dynamic allocation required
    // TODO: Implement host_verify_signature using async + func_wrap_async

    Ok(())
}
