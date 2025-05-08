use crate::error::RuntimeError;
use icn_common::resource::ResourceType;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmtime::{Config, Caller, Engine, Store, StoreLimits, StoreLimitsBuilder};

/// Resource limits for WebAssembly execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum amount of memory (in bytes) the module can use
    pub max_memory: u64,
    
    /// Maximum number of table elements
    pub max_table_elements: u32,
    
    /// Maximum number of instances that can be created
    pub max_instances: u32,
    
    /// Maximum number of modules that can be defined
    pub max_modules: u32,
    
    /// Maximum number of functions that can be defined or imported
    pub max_functions: u32,
    
    /// Maximum number of instructions that can be executed (fuel)
    pub max_instructions: u64,
    
    /// Maximum number of function parameters
    pub max_params: u32,
    
    /// Maximum function result values
    pub max_results: u32,
    
    /// Maximum size of linear memories in pages (64KiB per page)
    pub max_memory_pages: u64,
    
    /// Maximum number of simultaneous tables
    pub max_tables: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 100 * 1024 * 1024, // 100 MB
            max_table_elements: 10_000,
            max_instances: 10,
            max_modules: 10,
            max_functions: 10_000,
            max_instructions: 10_000_000, // 10 million instructions
            max_params: 100,
            max_results: 100,
            max_memory_pages: 1600, // 100 MB in 64 KiB pages
            max_tables: 1,
        }
    }
}

impl ResourceLimits {
    /// Apply these limits to a Wasmtime engine configuration
    pub fn apply_to_config(&self, config: &mut Config) {
        // Set static limits
        config.static_memory_maximum_size(self.max_memory);
        config.static_memory_guard_size(65536); // 64 KiB guard
        config.wasm_reference_types(true);
        config.wasm_multi_value(true);
        config.wasm_bulk_memory(true);
        config.wasm_simd(true);
        
        // Set memory limits
        config.static_memory_maximum_size(self.max_memory);
        
        // Set consumable resources (instructions)
        config.consume_fuel(true);
        
        // Set other static limits
        config.max_wasm_stack(65536); // 64 KiB stack
        
        // Misc configuration
        config.cache_config_load_default().ok(); // Use default cache if available
    }
    
    /// Create Wasmtime store limits from these resource limits
    pub fn create_store_limits(&self) -> StoreLimits {
        StoreLimitsBuilder::new()
            .memory_size(self.max_memory)
            .table_elements(self.max_table_elements)
            .instances(self.max_instances)
            .tables(self.max_tables)
            .memory_pages(self.max_memory_pages)
            .build()
    }
}

/// Collects resource usage during execution
#[derive(Debug, Clone, Default)]
pub struct ResourceUsageCollector {
    /// Resources used by the module
    pub resources: HashMap<ResourceType, u64>,
}

impl ResourceUsageCollector {
    /// Create a new resource usage collector
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }
    
    /// Record usage of a resource
    pub fn record_usage(&mut self, resource_type: ResourceType, amount: u64) {
        let current = self.resources.entry(resource_type).or_insert(0);
        *current += amount;
    }
    
    /// Check if usage is within limits
    pub fn check_limits(&self, limits: &HashMap<ResourceType, u64>) -> Result<(), RuntimeError> {
        for (resource_type, &used) in &self.resources {
            if let Some(&limit) = limits.get(resource_type) {
                if used > limit {
                    return Err(RuntimeError::ResourceLimitExceeded(
                        format!("Resource {:?} exceeded limit: used {} of {}", resource_type, used, limit)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Get total usage for a resource type
    pub fn get_usage(&self, resource_type: &ResourceType) -> u64 {
        *self.resources.get(resource_type).unwrap_or(&0)
    }
}

/// Resource meter for monitoring and limiting WebAssembly execution
pub struct ResourceMeter {
    /// Resource usage collector
    usage: Arc<Mutex<ResourceUsageCollector>>,
    
    /// Resource limits
    limits: HashMap<ResourceType, u64>,
    
    /// Identity that this execution is attributed to
    execution_identity: String,
    
    /// Scope this execution is running in
    execution_scope: String,
}

impl ResourceMeter {
    /// Create a new resource meter with specified limits
    pub fn new(
        limits: HashMap<ResourceType, u64>,
        execution_identity: String,
        execution_scope: String,
    ) -> Self {
        Self {
            usage: Arc::new(Mutex::new(ResourceUsageCollector::new())),
            limits,
            execution_identity,
            execution_scope,
        }
    }
    
    /// Check if a resource allocation is authorized
    pub fn check_resource_authorization(
        &self,
        resource_type: ResourceType,
        amount: u64,
    ) -> Result<bool, RuntimeError> {
        // Get current usage
        let usage = self.usage.lock().unwrap();
        let current_usage = usage.get_usage(&resource_type);
        
        // Check against limit
        if let Some(&limit) = self.limits.get(&resource_type) {
            if current_usage + amount > limit {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Record resource usage
    pub fn record_resource_usage(
        &self,
        resource_type: ResourceType,
        amount: u64,
    ) -> Result<(), RuntimeError> {
        // Check authorization first
        if !self.check_resource_authorization(resource_type.clone(), amount)? {
            return Err(RuntimeError::ResourceLimitExceeded(
                format!("Resource {:?} would exceed limit", resource_type)
            ));
        }
        
        // Record the usage
        let mut usage = self.usage.lock().unwrap();
        usage.record_usage(resource_type, amount);
        
        Ok(())
    }
    
    /// Get the current resource usage
    pub fn get_usage(&self) -> ResourceUsageCollector {
        self.usage.lock().unwrap().clone()
    }
    
    /// Get a clone of the usage tracker for sharing
    pub fn usage_tracker(&self) -> Arc<Mutex<ResourceUsageCollector>> {
        self.usage.clone()
    }
    
    /// Get the execution identity
    pub fn execution_identity(&self) -> &str {
        &self.execution_identity
    }
    
    /// Get the execution scope
    pub fn execution_scope(&self) -> &str {
        &self.execution_scope
    }
} 