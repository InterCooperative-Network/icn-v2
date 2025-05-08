use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration related to WASM execution within the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// If true, automatically issue ExecutionReceipts after successful runs.
    #[serde(default = "default_true")]
    pub auto_issue_receipts: bool,

    /// If true, and auto_issue_receipts is true, anchor the receipt into the DAG.
    #[serde(default = "default_true")]
    pub anchor_receipts: bool,

    /// Optional directory to export issued receipts as JSON files.
    /// If None, receipts are not exported to the filesystem.
    #[serde(default = "default_receipt_export_dir")]
    pub receipt_export_dir: Option<PathBuf>,
    
    // Placeholder for other execution-related configurations
    // pub max_execution_time_ms: u64,
    // pub max_memory_pages: u32,
}

fn default_true() -> bool {
    true
}

fn default_receipt_export_dir() -> Option<PathBuf> {
    Some(PathBuf::from("output/receipts"))
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            auto_issue_receipts: default_true(),
            anchor_receipts: default_true(),
            receipt_export_dir: default_receipt_export_dir(),
            // max_execution_time_ms: 5000, // example
            // max_memory_pages: 256,      // example (16MB)
        }
    }
}

/// Main runtime configuration, potentially encompassing more than just execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub execution: ExecutionConfig,
    // pub networking: NetworkingConfig, // Example for other configs
    // pub storage: StorageConfig,       // Example for other configs
}

// Example for other config structs if needed in the future
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct NetworkingConfig { /* ... */ }
// impl Default for NetworkingConfig { /* ... */ }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct StorageConfig { /* ... */ }
// impl Default for StorageConfig { /* ... */ } 