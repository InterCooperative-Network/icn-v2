# Mesh Computation Guide

This guide explains how to use ICN's distributed mesh computation system, which enables latency-aware, resource-efficient task distribution across federation nodes.

## Overview

The mesh computation system allows you to:

1. Publish computational tasks as WASM modules
2. Receive resource bids from nodes based on latency, compute capacity, and reputation
3. Execute tasks on the most suitable nodes
4. Record verifiable execution results in the DAG

## Prerequisites

- ICN CLI installed
- A federation with at least two nodes (see [Federation Sync Guide](federation_sync.md))
- WebAssembly module(s) for computation

## Publishing a Task

First, prepare a WebAssembly module that contains your computational task:

```bash
# Example: Compile a Rust program to WASM (requires wasm-pack)
wasm-pack build --target nodejs
```

Then publish the task to your federation:

```bash
icn mesh publish-task \
  --wasm-file ./target/wasm32-unknown-unknown/release/my_task.wasm \
  --input "s3://mybucket/dataset1" \
  --input "ipfs://QmHash123" \
  --max-latency 500 \
  --memory 1024 \
  --cores 2 \
  --priority 75 \
  --federation "my-federation" \
  --key ./key.json \
  --dag-dir ./dag-data
```

This will:
1. Create a `TaskTicket` node in the DAG
2. Compute a hash of your WASM file for verification
3. Broadcast the task ticket to federation peers
4. Return a CID that identifies your task

## Bidding on Tasks

Nodes with available resources can bid on published tasks:

```bash
icn mesh bid \
  --task-cid QmTaskCid123 \
  --latency 50 \
  --memory 2048 \
  --cores 4 \
  --reputation 85 \
  --renewable 70 \
  --key ./compute-node.json \
  --dag-dir ./dag-data
```

The bid includes:
- Current latency to reach the node (in milliseconds)
- Available memory and CPU cores
- Reputation score of the bidder
- Percentage of renewable energy used (optional sustainability metric)

## Running a Scheduler

A scheduler node automatically matches tasks with the best bids:

```bash
icn mesh scheduler \
  --federation "my-federation" \
  --key ./scheduler.json \
  --dag-dir ./dag-data \
  --listen "/ip4/0.0.0.0/tcp/9001"
```

The scheduler:
1. Monitors the DAG for new `TaskTicket` and `TaskBid` nodes
2. Evaluates bids using a scoring function that considers latency, resources, and reputation
3. Selects the optimal bid for each task
4. Creates task assignments in the DAG

### Capability-Based Scheduling

You can specify capability requirements to filter nodes based on their manifests before considering bids:

```bash
icn mesh scheduler \
  --federation "my-federation" \
  --key ./scheduler.json \
  --dag-dir ./dag-data \
  --listen "/ip4/0.0.0.0/tcp/9001" \
  --require "arch=x86_64" \
  --require "min_cores=4" \
  --require "min_ram_mb=8192" \
  --require "gpu_api=cuda" \
  --require "gpu_vram_mb=4096" \
  --require "min_renewable=50"
```

Multiple `--require` flags can be added, each with a key=value pair. The scheduler will only consider nodes
whose manifests match all the specified requirements.

#### Available Capability Requirements

| Key | Value Format | Description |
|-----|--------------|-------------|
| `arch` | `x86_64`, `arm64`, `riscv32`, `riscv64`, `wasm32` | Required CPU architecture |
| `min_cores` | Integer | Minimum number of CPU cores |
| `min_ram_mb` | Integer | Minimum RAM in megabytes |
| `min_storage_gb` | Integer | Minimum storage in gigabytes |
| `gpu_vram_mb` | Integer | Minimum GPU VRAM in megabytes |
| `gpu_cores` | Integer | Minimum number of GPU cores |
| `gpu_tensor_cores` | `true`/`false` | Whether tensor cores are required |
| `gpu_api` | `cuda`, `vulkan`, `metal`, `webgpu`, `opencl`, `directx` | Required GPU API |
| `gpu_feature` | String | Required GPU feature (can be specified multiple times) |
| `sensor` | `type:protocol:active` | Required sensor type, optional protocol, and active status |
| `actuator` | `type:protocol:active` | Required actuator type, optional protocol, and active status |
| `min_renewable` | Integer (0-100) | Minimum renewable energy percentage |
| `energy_source` | `grid`, `solar`, `wind`, `battery`, `generator` | Required energy source |
| `requires_battery` | `true`/`false` | Whether battery power is required |
| `requires_charging` | `true`/`false` | Whether charging status is required |
| `max_power_watts` | Decimal | Maximum power consumption in watts |

#### Example Use Cases

**AI Workloads**:
```bash
icn mesh scheduler \
  --federation "ai-federation" \
  --key ./scheduler.json \
  --dag-dir ./dag-data \
  --require "gpu_api=cuda" \
  --require "gpu_vram_mb=8192" \
  --require "gpu_tensor_cores=true"
```

**IoT Sensor Network**:
```bash
icn mesh scheduler \
  --federation "iot-federation" \
  --key ./scheduler.json \
  --dag-dir ./dag-data \
  --require "sensor=temperature:i2c:true" \
  --require "sensor=humidity:i2c:true" \
  --require "requires_battery=true"
```

**Green Computing**:
```bash
icn mesh scheduler \
  --federation "green-federation" \
  --key ./scheduler.json \
  --dag-dir ./dag-data \
  --require "min_renewable=75" \
  --require "energy_source=solar" \
  --require "max_power_watts=50"
```

## Executing Tasks

When a bid is accepted, the winning node executes the task:

```bash
icn mesh execute \
  --task-cid QmTaskCid123 \
  --bid-cid QmBidCid456 \
  --key ./compute-node.json \
  --dag-dir ./dag-data \
  --output-dir ./results
```

After execution:
1. Results are saved to the specified output directory
2. An `ExecutionReceipt` is anchored to the DAG
3. The receipt contains execution metrics and results hash

## Bid Scoring Formula

The default bid scoring formula prioritizes low latency, high reputation, and efficient resource usage:

```
score = latency * (100 - reputation) / (memory * cores * (1 + renewable/100))
```

Lower scores are better. This balances:
- Fast response time (low latency)
- Trusted nodes (high reputation)
- Sufficient resources (memory and cores)
- Green computing (renewable energy percentage)

## Example: Distributed Image Processing

Let's walk through a complete example:

```bash
# Node 1: Create federation and publish an image processing task
icn dag sync-p2p genesis --federation "image-proc" --dag-dir ./node1-data --key ./founder.json --policy-id "compute.v1" --founding-dids did:example:node1,did:example:node2
icn mesh publish-task --wasm-file ./image-processor.wasm --input "s3://images/batch1/*" --federation "image-proc" --key ./founder.json --dag-dir ./node1-data

# Node 2: Join federation and start scheduler
icn dag sync-p2p auto-sync --federation "image-proc" --dag-dir ./node2-data --bootstrap-peers "/ip4/192.168.1.100/tcp/9000/p2p/QmPeer1"
icn mesh scheduler --federation "image-proc" --key ./node2.json --dag-dir ./node2-data

# Node 3: Join federation and bid on task
icn dag sync-p2p auto-sync --federation "image-proc" --dag-dir ./node3-data --bootstrap-peers "/ip4/192.168.1.100/tcp/9000/p2p/QmPeer1"
icn mesh bid --task-cid QmTask123 --latency 25 --memory 4096 --cores 8 --key ./node3.json --dag-dir ./node3-data

# Node 3: Execute task when bid is selected
icn mesh execute --task-cid QmTask123 --bid-cid QmBid456 --key ./node3.json --dag-dir ./node3-data --output-dir ./processed-images
```

## Visualizing Computation

To visualize the computational graph:

```bash
icn dag visualize --dag-dir ./dag-data --output compute-graph.dot --max-nodes 100
dot -Tpng compute-graph.dot -o compute-graph.png
```

The visualization will show:
- Task tickets (yellow)
- Bids (yellow)
- Execution receipts (green)
- Connections between related nodes

## Next Steps

- Create custom WASM tasks with the ICN WASM SDK (coming soon)
- Implement a custom bid scoring function for your federation's needs
- Set up automatic task publishing from your applications 