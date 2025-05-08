# ICN Command Line Interface

The ICN CLI tool provides a command-line interface for interacting with the Interoperable Compute Network.

## Building

From the repository root:

```bash
# Build with release optimizations
cargo build --release -p icn-cli

# The binary will be located at target/release/icn-cli
```

## Usage

```bash
# View available commands
cargo run --release -p icn-cli -- --help

# Run with verbose output
cargo run --release -p icn-cli -- -v <command>
```

## Key Commands

### Key Management

```bash
# Generate a new key
cargo run --release -p icn-cli -- key-gen --output my-key.json

# Import an existing key
cargo run --release -p icn-cli -- key-gen import --file existing-key.json --output my-key.json

# View key information
cargo run --release -p icn-cli -- key-gen info --file my-key.json
```

### Federation Commands

```bash
# Initialize a new federation
cargo run --release -p icn-cli -- federation init \
  --name "My Federation" \
  --output-dir ./federation-data \
  --participant node1-key.json \
  --participant node2-key.json \
  --quorum "threshold:67"

# Submit a proposal to a federation
cargo run --release -p icn-cli -- federation submit-proposal \
  --file proposal.toml \
  --to http://localhost:5001 \
  --key my-key.json

# Vote on a proposal
cargo run --release -p icn-cli -- federation vote \
  --proposal-id <proposal-id> \
  --key my-key.json \
  --to http://localhost:5001 \
  --decision approve

# Execute an approved proposal
cargo run --release -p icn-cli -- federation execute \
  --proposal-id <proposal-id> \
  --key my-key.json \
  --to http://localhost:5001

# Export a federation to a CAR archive
cargo run --release -p icn-cli -- federation export \
  --federation-dir ./federation-data \
  --output federation.car

# Import a federation from a CAR archive
cargo run --release -p icn-cli -- federation import \
  --archive-path federation.car \
  --output-dir ./new-federation
```

### Mesh Network Commands

```bash
# Submit a job to the mesh network
cargo run --release -p icn-cli -- mesh submit-job \
  --manifest job.toml \
  --to http://localhost:5001 \
  --key my-key.json

# Get bids for a job
cargo run --release -p icn-cli -- mesh get-bids \
  --job-id <job-id> \
  --limit 10 \
  --sort-by price

# Select a bid for execution
cargo run --release -p icn-cli -- mesh select-bid \
  --job-id <job-id> \
  --bid-id <bid-id> \
  --key my-key.json

# Check job status
cargo run --release -p icn-cli -- mesh job-status \
  --job-id <job-id> \
  --to http://localhost:5001

# Verify execution receipt
cargo run --release -p icn-cli -- mesh verify-receipt \
  --receipt-id <receipt-id>
```

### DAG Commands

```bash
# Sync with the federation DAG
cargo run --release -p icn-cli -- dag sync-p2p \
  --federation "my-federation" \
  --key my-key.json \
  --listen-addr "/ip4/0.0.0.0/tcp/9000"

# View DAG events
cargo run --release -p icn-cli -- observe dag-view \
  --dag-dir ./data
```

## Example Files

### Federation Proposal

```toml
# Federation Proposal (proposal.toml)
federation_id = "my-federation"
name = "My ICN Federation"
description = "A demonstration federation for ICN testing"

# Nodes in the federation
[[nodes]]
did = "did:icn:node1"
role = "Validator"

[[nodes]]
did = "did:icn:node2"
role = "Validator"

# Federation policies
[policies]
voting_threshold = 0.67
min_validators = 2
max_validators = 10
```

### Job Manifest

```toml
# Job Manifest (job.toml)
id = "sample-job-001"
wasm_module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
owner_did = "did:icn:requester"
federation_id = "my-federation"

# Resource requirements
[[resource_requirements]]
type = "RamMb"
value = 1024

[[resource_requirements]]
type = "CpuCores"
value = 2

[parameters]
input_data = "Sample input data for the job"
iterations = 100
verbose = true
``` 