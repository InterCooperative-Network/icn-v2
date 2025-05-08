use crate::metering::ResourceLimits;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// WASM runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Resource limits for execution
    pub resource_limits: ResourceLimits,
    
    /// Max execution time
    pub execution_timeout: Duration,
    
    /// Allowed host functions
    pub allowed_imports: Vec<String>,
    
    /// Whether to enable WASI
    pub enable_wasi: bool,
    
    /// WASI configuration
    pub wasi_config: WasiConfig,
    
    /// Enable deterministic mode (for reproducible execution)
    pub deterministic_mode: bool,
    
    /// Whether to pre-compile modules for faster execution
    pub pre_compile: bool,
    
    /// Cache directory for compiled modules
    pub cache_dir: Option<PathBuf>,
    
    /// Number of compilation threads to use
    pub compilation_threads: Option<usize>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            resource_limits: ResourceLimits::default(),
            execution_timeout: Duration::from_secs(5),
            allowed_imports: vec![
                // ICN host functions
                "icn:resource/check_resource_authorization".to_string(),
                "icn:resource/record_resource_usage".to_string(),
                "icn:identity/verify_credential".to_string(),
                
                // Core WASM imports that should always be allowed
                "wasi_snapshot_preview1".to_string(),
            ],
            enable_wasi: true,
            wasi_config: WasiConfig::default(),
            deterministic_mode: false,
            pre_compile: true,
            cache_dir: None,
            compilation_threads: None,
        }
    }
}

/// WASI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasiConfig {
    /// Allowed directories and their mappings
    pub allowed_dirs: Vec<(String, PathBuf)>,
    
    /// Allowed environment variables
    pub allowed_env_vars: Vec<String>,
    
    /// Command-line arguments to provide
    pub args: Vec<String>,
    
    /// Whether to inherit stdin
    pub inherit_stdin: bool,
    
    /// Whether to inherit stdout
    pub inherit_stdout: bool,
    
    /// Whether to inherit stderr
    pub inherit_stderr: bool,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            allowed_env_vars: vec![],
            args: vec![],
            inherit_stdin: false,
            inherit_stdout: true,
            inherit_stderr: true,
        }
    }
} 