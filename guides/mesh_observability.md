# Mesh Observability & Monitoring Guide

This guide explains how to monitor and visualize the ICN mesh network's performance metrics, token economics, and computation activities.

## Overview

The ICN mesh includes a complete observability layer that:

1. Collects real-time metrics from all mesh activities
2. Exposes metrics in Prometheus format
3. Visualizes key performance indicators in Grafana dashboards
4. Provides CLI tools for quick status checks

## Setting Up Monitoring Infrastructure

### Quick Start with Docker

The easiest way to deploy monitoring is with the provided setup script:

```bash
# Start Prometheus and Grafana in Docker containers
./scripts/devnet_monitor.sh
```

This will:
- Create a `monitoring` directory with all configurations
- Start Prometheus on port 9091
- Start Grafana on port 3000 (username/password: admin/admin)
- Configure dashboards automatically

### Starting the Metrics Server

To expose metrics from your ICN node:

```bash
# Start a metrics server for a specific federation
icn mesh metrics-server \
  --federation my-federation \
  --listen 127.0.0.1:9090 \
  --dag-dir ./dag-data
```

This will expose a `/metrics` endpoint that Prometheus will scrape.

## Checking Mesh Statistics from CLI

For quick insights without the dashboards, use the stats command:

```bash
# View task statistics
icn mesh stats \
  --federation my-federation \
  --dag-dir ./dag-data \
  --type-filter tasks

# View bid statistics
icn mesh stats \
  --federation my-federation \
  --dag-dir ./dag-data \
  --type-filter bids

# View token statistics and balances
icn mesh stats \
  --federation my-federation \
  --dag-dir ./dag-data \
  --type-filter tokens

# View execution receipt statistics
icn mesh stats \
  --federation my-federation \
  --dag-dir ./dag-data \
  --type-filter receipts

# View resource usage statistics
icn mesh stats \
  --federation my-federation \
  --dag-dir ./dag-data \
  --type-filter resources
```

## Available Metrics

The ICN mesh exposes the following metrics:

### Task Metrics

- `icn_task_published_total`: Counter of published tasks
- `icn_task_size_bytes`: Gauge showing task WASM size

### Bid Metrics

- `icn_bid_submitted_total`: Counter of submitted bids
- `icn_bid_accepted_total`: Counter of accepted bids
- `icn_bid_latency_ms`: Gauge showing bid latency
- `icn_bid_score`: Gauge showing bid score (lower is better)

### Execution Metrics

- `icn_execution_started_total`: Counter of started executions
- `icn_execution_completed_total`: Counter of completed executions
- `icn_execution_time_ms`: Histogram of execution times
- `icn_execution_memory_mb`: Histogram of memory usage
- `icn_execution_cpu_pct`: Histogram of CPU usage

### Token Metrics

- `icn_token_transfers_total`: Counter of token transfers
- `icn_token_transfer_amount`: Histogram of token amounts

### Peer Metrics

- `icn_peer_connections_total`: Counter of peer connections
- `icn_peers_connected`: Gauge showing currently connected peers
- `icn_peer_sync_total`: Counter of sync operations
- `icn_peer_latency_ms`: Histogram of peer latencies

### Resource Usage Metrics

- `icn_resource_usage_total`: Counter of all resource usage events by type
- `icn_cpu_seconds_total`: Histogram of CPU time consumed
- `icn_memory_mb_seconds_total`: Histogram of memory usage over time
- `icn_io_read_bytes_total`: Counter of bytes read
- `icn_io_write_bytes_total`: Counter of bytes written
- `icn_gpu_seconds_total`: Histogram of GPU time consumed
- `icn_storage_mb_seconds_total`: Counter of storage space used over time
- `icn_bandwidth_in_bytes_total`: Counter of ingress bandwidth usage
- `icn_bandwidth_out_bytes_total`: Counter of egress bandwidth usage
- `icn_sensor_events_total`: Counter of sensor access events
- `icn_environmental_events_total`: Counter of environmental data access events
- `icn_actuation_trigger_total`: Counter of actuation events
- `icn_specialized_seconds_total`: Histogram of specialized hardware usage

## Grafana Dashboard

The default Grafana dashboard provides visualizations for:

1. **Mesh Activity**: Rate of tasks, bids, and executions
2. **Connected Peers**: Number of connected peers over time
3. **Token Transfers**: Rate of token transfers
4. **Task Execution Time**: P50 and P95 execution latencies
5. **Peer Latency**: P50 and P95 peer latencies
6. **Token Transfer Amounts**: P50 and P95 token amounts
7. **Resource Usage Overview**: Breakdown of all resource types used
8. **Sensor Activity**: Visualization of sensor access events
9. **Actuation Events**: Frequency and distribution of actuation triggers
10. **Specialized Hardware**: Usage of specialized computing resources

You can access the dashboard at http://localhost:3000 after running the monitoring setup.

## Advanced Configuration

### Monitoring Multiple Federations

To monitor multiple federations, edit the `monitoring/prometheus/prometheus.yml` file:

```yaml
scrape_configs:
  - job_name: 'federation-1'
    static_configs:
      - targets: ['host.docker.internal:9090']
        labels:
          federation: 'federation-1'
          
  - job_name: 'federation-2'
    static_configs:
      - targets: ['host.docker.internal:9091']
        labels:
          federation: 'federation-2'
```

### Creating Custom Dashboards

You can create custom dashboards in Grafana by:

1. Logging into Grafana at http://localhost:3000
2. Clicking "Create" > "Dashboard"
3. Adding panels that query your metrics

Common visualization use cases:
- **Token Economy Health**: Graph token transfer rates and balances
- **Computation Efficiency**: Graph execution time vs resources used
- **Network Health**: Graph peer count and sync success rates
- **Resource Usage Patterns**: Compare different resource types across federations
- **Sensor & Actuation Activity**: Monitor real-world interactions with physical devices

## Resource Compensation Models

ICN provides a unified resource metering and compensation model covering:

1. **Compute Resources**: CPU, GPU, specialized hardware
2. **Storage Resources**: Disk space, memory
3. **Network Resources**: Bandwidth in/out
4. **Physical Resources**: Sensors, actuators, environmental data

Each federation can define pricing models for resources:

```toml
[pricing]
CPU = "0.1 per second"
Storage = "0.05 per MB per hour"
SensorInput = "0.25 per event"
BandwidthEgress = "0.01 per MB"
```

Resource usage is automatically tracked, compensated with tokens, and anchored to the DAG for verifiable history.

## Using Metrics for Reputation Scoring

The metrics can be used to build reputation systems:

```rust
// Example reputation calculation
let success_rate = completed_executions / total_executions;
let avg_execution_time_deviation = (actual_time - estimated_time).abs() / estimated_time;
let reputation_score = success_rate * (1.0 - avg_execution_time_deviation);
```

## Troubleshooting

### Metrics Not Appearing

If metrics aren't showing up in Grafana:

1. Check that the metrics server is running:
   ```bash
   curl http://localhost:9090/metrics
   ```

2. Verify Prometheus can reach your metrics:
   ```bash
   curl http://localhost:9091/targets
   ```

3. Ensure labels match between Prometheus config and queries

### Dashboard Queries

If dashboard panels show "No data":

1. Check the time range in the top-right corner
2. Verify the metric names in panel queries
3. Check federation labels match your configuration

## Next Steps

- Set up alerts for critical conditions (node disconnect, high latency)
- Add additional metrics for GPU utilization when GPU support is implemented
- Integrate with external monitoring systems through Prometheus remote_write
- Create specialized dashboards for different resource categories (sensor networks, compute clusters, storage nodes) 