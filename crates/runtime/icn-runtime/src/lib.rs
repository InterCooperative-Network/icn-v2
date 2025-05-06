// ! Placeholder for icn-runtime library - Now with modules!

pub mod abi;
pub mod engine;
pub mod host;
pub mod dag_anchor;
pub mod config;

// Re-export key components
pub use engine::WasmExecutor;
pub use engine::VmContext;
pub use engine::WasmEngine;

pub use host::receipt::{issue_execution_receipt, ReceiptError};
pub use dag_anchor::{anchor_execution_receipt, AnchorError};
pub use config::{RuntimeConfig, ExecutionConfig};

// Keeping VmConfig for now, needs proper definition location
// pub struct VmConfig { pub execution: ExecutionConfig }
// pub struct ExecutionConfig { pub auto_issue_receipts: bool, pub anchor_receipts: bool, pub receipt_export_dir: Option<std::path::PathBuf> }

pub fn placeholder() {}

pub fn init_runtime() {
    // TODO: Initialize runtime components, logging, etc.
    // This could be where a default RuntimeConfig is loaded or constructed.
}
