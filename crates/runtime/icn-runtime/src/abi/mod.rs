pub mod context;
pub mod bindings;

// Keep HostSyscall enum for potential future direct interaction/planning?
// Or remove if only using Wasmtime bindings directly.
use icn_types::Did;

#[derive(Debug)]
pub enum HostSyscall {
    Log(String),
    GetCallerDid,
    VerifySignature { did: Did, message: Vec<u8>, signature: Vec<u8> },
    // Add more as needed
} 