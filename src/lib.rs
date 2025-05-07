// ! ICN-Runtime: WASM execution environment for the InterCooperative Network

pub mod abi;
pub mod engine;
pub mod host;
pub mod dag_anchor;
pub mod config;

// Re-export the main executor types from engine module
pub use engine::executor::ModernWasmExecutor;
pub use engine::executor::ExecutionResult;
pub use engine::executor::ContextExtension;
pub use engine::WasmExecutor;

// Other re-exports
pub use host::receipt::{issue_execution_receipt, ReceiptError};
pub use dag_anchor::{anchor_execution_receipt, AnchorError};
pub use config::{RuntimeConfig, ExecutionConfig};

/// Initialize runtime components (logging, etc.)
pub fn init_runtime() {
    // Initialize logging if not already set up
    if let Err(_) = env_logger::try_init() {
        // Ignore error if logger is already initialized
    }
    
    // Future: Initialize other runtime components as needed
} 