use wasmtime::{Linker, Caller, Memory, Trap};
use crate::abi::context::HostContext;
use std::sync::Arc;

// Helper function to read a string from WASM memory
fn read_string_from_memory(
    caller: &impl wasmtime::AsContext,
    memory: &Memory,
    ptr: i32,
    len: i32,
) -> Result<String, Trap> {
    let mem_slice = memory
        .data(&caller)
        .get(ptr as usize..(ptr + len) as usize)
        .ok_or_else(|| Trap::new("Pointer/length out of bounds"))?;
    String::from_utf8(mem_slice.to_vec())
        .map_err(|_| Trap::new("Invalid UTF-8 sequence in memory"))
}

/// Registers the ICN host functions with the Wasmtime linker.
pub fn register_host_functions<T: HostContext + Send + Sync + 'static>(
    linker: &mut Linker<Arc<T>>,
) -> anyhow::Result<()> {
    // --- Logging ---    
    linker.func_wrap(
        "icn",
        "host_log",
        |mut caller: Caller<'_, Arc<T>>, ptr: i32, len: i32| -> Result<(), Trap> {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Trap::new("Missing required memory export"))?;
            
            let msg = read_string_from_memory(&caller, &memory, ptr, len)?;
            
            // Call the host context method
            caller.data().log_message(&msg);
            Ok(())
        }
    )?;

    // --- Identity --- 
    // TODO: Implement host_get_caller_did
    //       - Need to figure out how to return complex types like Did (e.g., serialize to bytes?)
    // linker.func_wrap("icn", "host_get_caller_did", ...)?;

    // TODO: Implement host_verify_signature
    //       - Need async host functions (requires linker.func_wrap_async)
    //       - Need to handle argument passing (DID bytes, message bytes, sig bytes) and return value (bool -> i32?)
    // linker.func_wrap_async("icn", "host_verify_signature", ...)?;

    Ok(())
} 