use anyhow::Result;
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use prometheus::{
    register_counter, register_gauge, register_histogram,
    Counter, Gauge, Histogram,
    core::AtomicU64,
    IntCounter, IntGauge,
};
use log::{info, error};
use anyhow::{Result, anyhow};

/// Metrics types tracked in the ICN compute mesh
pub enum MetricType {
    /// Task metrics
    TaskPublished,
    TaskExecuted,
    TaskRejected,
    
    /// Bid metrics
    BidSubmitted,
    BidAccepted,
    BidRejected,
    
    /// Execution metrics
    ExecutionTime,
    ExecutionMemory,
    ExecutionCpu,
    
    /// Token metrics
    TokensTransferred,
    TokenBalance,
    
    /// Peer metrics
    PeersConnected,
    PeerSync,
    PeerLatency,
    
    /// Resource usage metrics
    ResourceUsageCpu,
    ResourceUsageMemory,
    ResourceUsageIO,
    ResourceUsageGpu,
    ResourceUsageStorage,
    ResourceUsageBandwidthIn,
    ResourceUsageBandwidthOut,
    ResourceUsageSensor,
    ResourceUsageEnvironmental,
    ResourceUsageActuation,
    ResourceUsageSpecialized,
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Failed to register metric: {0}")]
    RegistrationError(String),
    
    #[error("Failed to initialize metrics server: {0}")]
    ServerError(String),
    
    #[error("Other metrics error: {0}")]
    Other(String),
}

/// Metrics server for collecting and exposing mesh activity
pub struct MetricsServer {
    handle: PrometheusHandle,
    registry: Arc<prometheus::Registry>,
    address: SocketAddr,
}

impl MetricsServer {
    /// Create a new metrics server listening on the given address
    pub fn new(address: SocketAddr) -> Result<Self, MetricsError> {
        let registry = Arc::new(prometheus::Registry::new());
        let builder = PrometheusBuilder::new();
        
        let handle = builder
            .with_http_listener(address.clone())
            .build()
            .map_err(|e| MetricsError::ServerError(format!("Failed to create metrics server: {}", e)))?;
            
        Ok(Self {
            handle,
            registry,
            address,
        })
    }
    
    /// Start the metrics server in a background thread
    pub fn start_server(&self) -> Result<(), MetricsError> {
        info!("Starting metrics server on {}", self.address);
        
        // The PrometheusBuilder already starts the server when build() is called
        
        Ok(())
    }
    
    /// Register a counter metric
    pub fn register_counter(&self, name: &str, help: &str, labels: &[&str]) -> Result<IntCounter, MetricsError> {
        let counter = IntCounter::new(name, help)
            .map_err(|e| MetricsError::RegistrationError(format!("Failed to create counter: {}", e)))?;
            
        self.registry.register(Box::new(counter.clone()))
            .map_err(|e| MetricsError::RegistrationError(format!("Failed to register counter: {}", e)))?;
            
        Ok(counter)
    }
    
    /// Register a gauge metric
    pub fn register_gauge(&self, name: &str, help: &str, labels: &[&str]) -> Result<IntGauge, MetricsError> {
        let gauge = IntGauge::new(name, help)
            .map_err(|e| MetricsError::RegistrationError(format!("Failed to create gauge: {}", e)))?;
            
        self.registry.register(Box::new(gauge.clone()))
            .map_err(|e| MetricsError::RegistrationError(format!("Failed to register gauge: {}", e)))?;
            
        Ok(gauge)
    }
    
    /// Increment a counter metric
    pub fn increment_counter(&self, metric_type: MetricType, value: u64) {
        let name = self.get_metric_name(metric_type);
        counter!(&name, value as u64);
    }
    
    /// Set a gauge metric
    pub fn set_gauge(&self, metric_type: MetricType, value: i64) {
        let name = self.get_metric_name(metric_type);
        gauge!(&name, value as f64);
    }
    
    /// Record a histogram value
    pub fn record_histogram(&self, metric_type: MetricType, value: f64) {
        let name = self.get_metric_name(metric_type);
        histogram!(&name, value);
    }
    
    /// Get the metric name for a metric type
    fn get_metric_name(&self, metric_type: MetricType) -> String {
        match metric_type {
            MetricType::TaskPublished => "icn_mesh_tasks_published_total",
            MetricType::TaskExecuted => "icn_mesh_tasks_executed_total",
            MetricType::TaskRejected => "icn_mesh_tasks_rejected_total",
            
            MetricType::BidSubmitted => "icn_mesh_bids_submitted_total",
            MetricType::BidAccepted => "icn_mesh_bids_accepted_total",
            MetricType::BidRejected => "icn_mesh_bids_rejected_total",
            
            MetricType::ExecutionTime => "icn_mesh_execution_time_seconds",
            MetricType::ExecutionMemory => "icn_mesh_execution_memory_bytes",
            MetricType::ExecutionCpu => "icn_mesh_execution_cpu_percent",
            
            MetricType::TokensTransferred => "icn_mesh_tokens_transferred_total",
            MetricType::TokenBalance => "icn_mesh_token_balance",
            
            MetricType::PeersConnected => "icn_mesh_peers_connected",
            MetricType::PeerSync => "icn_mesh_peer_sync_total",
            MetricType::PeerLatency => "icn_mesh_peer_latency_seconds",
            
            MetricType::ResourceUsageCpu => "icn_mesh_resource_usage_cpu_percent",
            MetricType::ResourceUsageMemory => "icn_mesh_resource_usage_memory_bytes",
            MetricType::ResourceUsageIO => "icn_mesh_resource_usage_io_bytes",
            MetricType::ResourceUsageGpu => "icn_mesh_resource_usage_gpu_percent",
            MetricType::ResourceUsageStorage => "icn_mesh_resource_usage_storage_bytes",
            MetricType::ResourceUsageBandwidthIn => "icn_mesh_resource_usage_bandwidth_in_bytes",
            MetricType::ResourceUsageBandwidthOut => "icn_mesh_resource_usage_bandwidth_out_bytes",
            MetricType::ResourceUsageSensor => "icn_mesh_resource_usage_sensor_total",
            MetricType::ResourceUsageEnvironmental => "icn_mesh_resource_usage_environmental_total",
            MetricType::ResourceUsageActuation => "icn_mesh_resource_usage_actuation_total",
            MetricType::ResourceUsageSpecialized => "icn_mesh_resource_usage_specialized_total",
        }.to_string()
    }
}

/// Metrics context for collecting and exposing mesh activity
pub struct MetricsContext {
    prometheus_handle: PrometheusHandle,
    federation_id: String,
    task_counter: Option<IntCounter>,
    bid_counter: Option<IntCounter>,
    execution_counter: Option<IntCounter>,
    token_counter: Option<IntCounter>,
    manifest_verification_failures: Option<IntCounter>,
    manifest_verification_failure_reasons: Option<Gauge<AtomicU64>>,
}

impl MetricsContext {
    /// Create a new metrics context for a federation with basic initialization
    pub fn new(federation_id: &str) -> Self {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder.install_recorder().expect("Failed to install Prometheus recorder");
        
        Self {
            prometheus_handle,
            federation_id: federation_id.to_string(),
            task_counter: None,
            bid_counter: None,
            execution_counter: None,
            token_counter: None,
            manifest_verification_failures: None,
            manifest_verification_failure_reasons: None,
        }
    }
    
    /// Initialize all metrics for the context
    pub fn with_all_metrics(federation_id: &str) -> Result<Self> {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder.install_recorder().expect("Failed to install Prometheus recorder");
        
        // Initialize metrics
        let task_counter = register_int_counter!(
            "icn_tasks_published_total",
            "Total number of tasks published",
            &["federation"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        let bid_counter = register_int_counter!(
            "icn_bids_submitted_total",
            "Total number of bids submitted",
            &["federation"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        let execution_counter = register_int_counter!(
            "icn_tasks_executed_total",
            "Total number of tasks executed",
            &["federation"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        let token_counter = register_int_counter!(
            "icn_tokens_transferred_total",
            "Total number of token transfers",
            &["federation", "from", "to"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        let manifest_verification_failures = register_int_counter!(
            "icn_manifest_verification_failures_total",
            "Total number of manifest verification failures",
            &["federation", "did"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        let manifest_verification_failure_reasons = register_gauge!(
            "icn_manifest_verification_failure_reasons",
            "Counts of different manifest verification failure reasons",
            &["federation", "reason"]
        ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?;
        
        Ok(Self {
            prometheus_handle,
            federation_id: federation_id.to_string(),
            task_counter: Some(task_counter),
            bid_counter: Some(bid_counter),
            execution_counter: Some(execution_counter),
            token_counter: Some(token_counter),
            manifest_verification_failures: Some(manifest_verification_failures),
            manifest_verification_failure_reasons: Some(manifest_verification_failure_reasons),
        })
    }
    
    /// Initialize specific metrics as needed
    pub fn initialize_verification_metrics(&mut self) -> Result<()> {
        if self.manifest_verification_failures.is_none() {
            self.manifest_verification_failures = Some(register_int_counter!(
                "icn_manifest_verification_failures_total",
                "Total number of manifest verification failures",
                &["federation", "did"]
            ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?);
        }
        
        if self.manifest_verification_failure_reasons.is_none() {
            self.manifest_verification_failure_reasons = Some(register_gauge!(
                "icn_manifest_verification_failure_reasons",
                "Counts of different manifest verification failure reasons",
                &["federation", "reason"]
            ).map_err(|e| MetricsError::RegistrationError(e.to_string()))?);
        }
        
        Ok(())
    }
    
    /// Record a task publication
    pub fn record_task_published(&self, task_cid: &str, wasm_size: u64) {
        counter!("icn_tasks_published", "federation" => self.federation_id.clone()).increment(1);
        histogram!("icn_tasks_wasm_size", "federation" => self.federation_id.clone()).record(wasm_size as f64);
        
        if let Some(counter) = &self.task_counter {
            counter.with_label_values(&[&self.federation_id]).inc();
        }
    }
    
    /// Record a bid submission
    pub fn record_bid_submitted(&self, bid_cid: &str, task_cid: &str, latency: u64, score: f64) {
        counter!("icn_bids_submitted", "federation" => self.federation_id.clone()).increment(1);
        histogram!("icn_bids_latency", "federation" => self.federation_id.clone()).record(latency as f64);
        histogram!("icn_bids_score", "federation" => self.federation_id.clone()).record(score);
        
        if let Some(counter) = &self.bid_counter {
            counter.with_label_values(&[&self.federation_id]).inc();
        }
    }
    
    /// Record an execution completion
    pub fn record_execution_completed(&self, task_cid: &str, exec_time_ms: u64, memory_mb: u64, cpu_pct: u64) {
        counter!("icn_executions_completed", "federation" => self.federation_id.clone()).increment(1);
        histogram!("icn_execution_time_ms", "federation" => self.federation_id.clone()).record(exec_time_ms as f64);
        histogram!("icn_execution_memory_mb", "federation" => self.federation_id.clone()).record(memory_mb as f64);
        histogram!("icn_execution_cpu_pct", "federation" => self.federation_id.clone()).record(cpu_pct as f64);
        
        if let Some(counter) = &self.execution_counter {
            counter.with_label_values(&[&self.federation_id]).inc();
        }
    }
    
    /// Record a token transfer
    pub fn record_token_transfer(&self, from: &str, to: &str, amount: f64) {
        counter!("icn_token_transfers", 
            "federation" => self.federation_id.clone(),
            "from" => from.to_string(),
            "to" => to.to_string()
        ).increment(1);
        
        histogram!("icn_token_transfer_amount", 
            "federation" => self.federation_id.clone()
        ).record(amount);
        
        if let Some(counter) = &self.token_counter {
            counter.with_label_values(&[&self.federation_id, from, to]).inc();
        }
    }
    
    /// Record the creation of a genesis state
    pub fn record_genesis_state_created(&self, state_cid: &str, policy_id: &str, signatures: u64, anchors: u64) {
        counter!("icn_genesis_states", "federation" => self.federation_id.clone()).increment(1);
        gauge!("icn_genesis_signatures", "federation" => self.federation_id.clone()).set(signatures as f64);
        gauge!("icn_genesis_anchors", "federation" => self.federation_id.clone()).set(anchors as f64);
    }
    
    /// Record scheduler startup
    pub fn record_scheduler_started(&self) {
        counter!("icn_scheduler_starts", "federation" => self.federation_id.clone()).increment(1);
    }
    
    /// Record a manifest verification failure
    pub fn record_manifest_verification_failure(&self, did: &str, reason: &str) {
        // Initialize metrics if needed
        if let Err(e) = self.initialize_verification_metrics() {
            error!("Failed to initialize verification metrics: {:?}", e);
            return;
        }
        
        // Increment general counter via metrics crate
        counter!("icn_manifest_verification_failures", 
            "federation" => self.federation_id.clone(),
            "did" => did.to_string()
        ).increment(1);
        
        // Use our registered metrics if available
        if let Some(counter) = &self.manifest_verification_failures {
            counter.with_label_values(&[&self.federation_id, did]).inc();
        }
        
        if let Some(gauge) = &self.manifest_verification_failure_reasons {
            gauge.with_label_values(&[&self.federation_id, reason]).inc();
        }
        
        // Log the failure
        error!("Manifest verification failed for DID {} in federation {}: {}", 
            did, self.federation_id, reason);
    }
}

// Global function to initialize metrics
pub fn init_metrics(federation_id: &str, metrics_addr: Option<SocketAddr>) -> Result<MetricsContext> {
    let mut context = MetricsContext::new(federation_id);
    
    if let Some(addr) = metrics_addr {
        // Start a metrics server if an address is provided
        
        // Use Prometheus metrics
        let builder = PrometheusBuilder::new();
        let builder = builder
            .with_http_listener(addr)
            .with_prefix("icn")
            .with_default_metrics();
            
        // Install global recorder
        builder.install().map_err(|e| anyhow!("Failed to install Prometheus metrics recorder: {}", e))?;
        
        info!("Metrics server started on http://{}/metrics", addr);
    }
    
    // Initialize verification metrics
    let _ = context.initialize_verification_metrics();
    
    Ok(context)
} 