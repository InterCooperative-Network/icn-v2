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

/// Metrics context for collecting and exposing mesh activity
pub struct MetricsContext {
    prometheus_handle: PrometheusHandle,
    federation_id: String,
}

impl MetricsContext {
    /// Create a new metrics context for a federation
    pub fn new(federation_id: &str) -> Self {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder.install_recorder().expect("Failed to install Prometheus recorder");
        
        Self {
            prometheus_handle,
            federation_id: federation_id.to_string(),
        }
    }
    
    /// Start a metrics server on the given address
    pub fn start_server(&self, addr: SocketAddr) -> Result<()> {
        let prometheus_handle = self.prometheus_handle.clone();
        
        thread::spawn(move || {
            let rt = Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async {
                let metrics_service = make_service_fn(move |_| {
                    let handle = prometheus_handle.clone();
                    async move {
                        Ok::<_, hyper::Error>(service_fn(move |_: Request<Body>| {
                            let handle = handle.clone();
                            async move {
                                let metrics = handle.render();
                                Ok::<_, hyper::Error>(Response::new(Body::from(metrics)))
                            }
                        }))
                    }
                });
                
                println!("Metrics server listening on http://{}/metrics", addr);
                let server = Server::bind(&addr).serve(metrics_service);
                if let Err(e) = server.await {
                    eprintln!("Metrics server error: {}", e);
                }
            });
        });
        
        Ok(())
    }
    
    /// Record task published event
    pub fn record_task_published(&self, task_cid: &str, wasm_size_bytes: u64) {
        counter!("icn_task_published_total", "federation" => self.federation_id.clone()).increment(1);
        gauge!("icn_task_size_bytes", "task_cid" => task_cid.to_string(), "federation" => self.federation_id.clone()).set(wasm_size_bytes as f64);
    }
    
    /// Record bid submitted event
    pub fn record_bid_submitted(&self, bid_cid: &str, task_cid: &str, latency: u64, score: f64) {
        counter!("icn_bid_submitted_total", "federation" => self.federation_id.clone()).increment(1);
        gauge!("icn_bid_latency_ms", "bid_cid" => bid_cid.to_string(), "task_cid" => task_cid.to_string()).set(latency as f64);
        gauge!("icn_bid_score", "bid_cid" => bid_cid.to_string(), "task_cid" => task_cid.to_string()).set(score);
    }
    
    /// Record bid accepted event
    pub fn record_bid_accepted(&self, bid_cid: &str, task_cid: &str) {
        counter!("icn_bid_accepted_total", "federation" => self.federation_id.clone()).increment(1);
    }
    
    /// Record execution started
    pub fn record_execution_started(&self, task_cid: &str, bid_cid: &str) {
        counter!("icn_execution_started_total", "federation" => self.federation_id.clone()).increment(1);
    }
    
    /// Record execution completed
    pub fn record_execution_completed(&self, task_cid: &str, execution_time_ms: u64, memory_mb: u64, cpu_pct: u64) {
        counter!("icn_execution_completed_total", "federation" => self.federation_id.clone()).increment(1);
        histogram!("icn_execution_time_ms", "federation" => self.federation_id.clone()).record(execution_time_ms as f64);
        histogram!("icn_execution_memory_mb", "federation" => self.federation_id.clone()).record(memory_mb as f64);
        histogram!("icn_execution_cpu_pct", "federation" => self.federation_id.clone()).record(cpu_pct as f64);
    }
    
    /// Record token transfer
    pub fn record_token_transfer(&self, from_did: &str, to_did: &str, amount: f64) {
        counter!("icn_token_transfers_total", "federation" => self.federation_id.clone()).increment(1);
        histogram!("icn_token_transfer_amount", "federation" => self.federation_id.clone(), "from" => from_did.to_string(), "to" => to_did.to_string()).record(amount);
    }
    
    /// Record peer connected
    pub fn record_peer_connected(&self, peer_id: &str) {
        counter!("icn_peer_connections_total", "federation" => self.federation_id.clone()).increment(1);
        gauge!("icn_peers_connected", "federation" => self.federation_id.clone()).increment(1.0);
    }
    
    /// Record peer disconnected
    pub fn record_peer_disconnected(&self, peer_id: &str) {
        counter!("icn_peer_disconnections_total", "federation" => self.federation_id.clone()).increment(1);
        gauge!("icn_peers_connected", "federation" => self.federation_id.clone()).decrement(1.0);
    }
    
    /// Record peer sync
    pub fn record_peer_sync(&self, peer_id: &str, nodes_accepted: usize, nodes_rejected: usize) {
        counter!("icn_peer_sync_total", "federation" => self.federation_id.clone()).increment(1);
        counter!("icn_peer_sync_accepted_nodes", "federation" => self.federation_id.clone()).increment(nodes_accepted as u64);
        counter!("icn_peer_sync_rejected_nodes", "federation" => self.federation_id.clone()).increment(nodes_rejected as u64);
    }
    
    /// Record peer latency
    pub fn record_peer_latency(&self, peer_id: &str, latency_ms: u64) {
        histogram!("icn_peer_latency_ms", "federation" => self.federation_id.clone(), "peer_id" => peer_id.to_string()).record(latency_ms as f64);
    }
    
    /// Record resource usage - CPU
    pub fn record_resource_cpu(&self, task_cid: &str, cpu_seconds: f64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "cpu").increment(1);
        histogram!("icn_cpu_seconds_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).record(cpu_seconds);
    }
    
    /// Record resource usage - Memory
    pub fn record_resource_memory(&self, task_cid: &str, memory_mb: f64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "memory").increment(1);
        histogram!("icn_memory_mb_seconds_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).record(memory_mb);
    }
    
    /// Record resource usage - I/O
    pub fn record_resource_io(&self, task_cid: &str, read_bytes: u64, write_bytes: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "io").increment(1);
        counter!("icn_io_read_bytes_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).increment(read_bytes);
        counter!("icn_io_write_bytes_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).increment(write_bytes);
    }
    
    /// Record resource usage - GPU
    pub fn record_resource_gpu(&self, task_cid: &str, gpu_seconds: f64, gpu_model: &str) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "gpu").increment(1);
        histogram!("icn_gpu_seconds_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string(), "gpu_model" => gpu_model.to_string()).record(gpu_seconds);
    }
    
    /// Record resource usage - Storage
    pub fn record_resource_storage(&self, task_cid: &str, storage_mb: f64, duration_seconds: f64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "storage").increment(1);
        counter!("icn_storage_mb_seconds_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).increment((storage_mb * duration_seconds) as u64);
    }
    
    /// Record resource usage - Bandwidth ingress
    pub fn record_resource_bandwidth_in(&self, task_cid: &str, bytes: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "bandwidth_in").increment(1);
        counter!("icn_bandwidth_in_bytes_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).increment(bytes);
    }
    
    /// Record resource usage - Bandwidth egress
    pub fn record_resource_bandwidth_out(&self, task_cid: &str, bytes: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "bandwidth_out").increment(1);
        counter!("icn_bandwidth_out_bytes_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string()).increment(bytes);
    }
    
    /// Record resource usage - Sensor input
    pub fn record_resource_sensor(&self, task_cid: &str, sensor_type: &str, event_count: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "sensor").increment(1);
        counter!("icn_sensor_events_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string(), "sensor_type" => sensor_type.to_string()).increment(event_count);
    }
    
    /// Record resource usage - Environmental data
    pub fn record_resource_environmental(&self, task_cid: &str, data_type: &str, event_count: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "environmental").increment(1);
        counter!("icn_environmental_events_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string(), "data_type" => data_type.to_string()).increment(event_count);
    }
    
    /// Record resource usage - Actuation
    pub fn record_resource_actuation(&self, task_cid: &str, actuation_type: &str, trigger_count: u64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "actuation").increment(1);
        counter!("icn_actuation_trigger_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string(), "actuation_type" => actuation_type.to_string()).increment(trigger_count);
    }
    
    /// Record resource usage - Specialized hardware
    pub fn record_resource_specialized(&self, task_cid: &str, hardware_type: &str, usage_seconds: f64) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => "specialized").increment(1);
        histogram!("icn_specialized_seconds_total", "federation" => self.federation_id.clone(), "task_cid" => task_cid.to_string(), "hardware_type" => hardware_type.to_string()).record(usage_seconds);
    }
    
    /// Record generic resource usage for any resource type
    pub fn record_resource_usage(&self, task_cid: &str, resource_type: &str, amount: f64, metadata: Option<serde_json::Value>) {
        counter!("icn_resource_usage_total", "federation" => self.federation_id.clone(), "type" => resource_type).increment(1);
        
        // Extract any additional labels from metadata if provided
        let mut labels = vec![
            ("federation", self.federation_id.clone()),
            ("task_cid", task_cid.to_string()),
            ("resource_type", resource_type.to_string())
        ];
        
        if let Some(md) = metadata {
            if let Some(md_obj) = md.as_object() {
                for (k, v) in md_obj {
                    if let Some(v_str) = v.as_str() {
                        labels.push((k.as_str(), v_str.to_string()));
                    }
                }
            }
        }
        
        // Record the amount as a histogram to allow for percentile analysis
        let mut hist = histogram!("icn_resource_amount", "federation" => self.federation_id.clone(), "type" => resource_type.to_string());
        hist.record(amount);
    }
}

// Global function to initialize metrics
pub fn init_metrics(federation_id: &str, metrics_addr: Option<SocketAddr>) -> Result<MetricsContext> {
    let context = MetricsContext::new(federation_id);
    
    if let Some(addr) = metrics_addr {
        context.start_server(addr)?;
    }
    
    Ok(context)
} 