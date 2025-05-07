# ICN Command Line Interface (CLI)

The ICN CLI provides a comprehensive set of commands for interacting with the InterCooperative Network, including federation governance, identity management, and distributed mesh computing.

## Installation

```bash
# Build and install from source
cargo install --path .
```

## Key Features

- 🔑 **Identity Management**: Generate and manage DIDs and keypairs
- 🏛️ **Federation Governance**: Create and participate in federations, submit and vote on proposals
- 🌐 **Mesh Computing**: Submit jobs, view node capabilities, and manage job execution
- 📜 **Credential Verification**: Verify execution receipts and other credentials

## Mesh Computing Workflow

The ICN mesh provides distributed compute capabilities across federated nodes. The complete workflow is:

1. **Job Creation**: Define a job manifest with resource requirements
2. **Job Submission**: Submit the job to the network with identity verification
3. **Node Discovery**: Find capable nodes that can execute the job
4. **Bidding**: Receive bids from nodes willing to execute the job
5. **Bid Selection**: Select the most suitable bid based on price, capability, and reputation
6. **Execution**: The selected node executes the job and generates results
7. **Verification**: Verify the execution receipt and token compensation

## Commands

### Identity Management

```bash
# Generate a new identity key
icn-cli keygen generate --output my-key.json

# Show information about an identity
icn-cli keygen info --key-path my-key.json
```

### Mesh Computing

```bash
# Submit a job using a TOML manifest
icn-cli mesh submit-job --manifest-path job.toml --key-path my-key.json

# Submit a job with inline parameters
icn-cli mesh submit-job --wasm-module-cid bafybeihykld7uyxzogax6vgyvag42y7464eywpf55hnrwvgzxwvjmnx7fy \
  --memory-mb 2048 --cpu-cores 4 --key-path my-key.json

# List available nodes with filtering
icn-cli mesh list-nodes --min-memory 1024 --min-cores 2

# Get bids for a job
icn-cli mesh get-bids --job-id job-12345 --sort-by price

# Select and accept a bid
icn-cli mesh select-bid --job-id job-12345 --bid-id 1 --key-path my-key.json

# Check job status
icn-cli mesh job-status --job-id job-12345

# Advertise node capabilities
icn-cli mesh advertise-capability --cpu-cores 8 --memory-mb 4096 --key-path node-key.json

# Submit a bid for a job
icn-cli mesh submit-bid --job-id job-12345 --price 50 --confidence 0.95 --key-path node-key.json
```

## Job Manifest Format

The job manifest can be defined in TOML or JSON format. Here's an example:

```toml
# ICN Mesh Job Manifest
id = "job-12345"
wasm_module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
owner_did = "did:icn:requester"

[[resource_requirements]]
type = "RamMb"
value = 1024

[[resource_requirements]]
type = "CpuCores"
value = 2

[parameters]
input_data = "Sample input data"
iterations = 100
```

## Verifiable Credentials

All operations in the ICN use W3C Verifiable Credentials for secure, auditable interactions. Each job submission, bid, and execution result includes a cryptographic proof that can be verified by any participant.

## Demo Script

A comprehensive demo script is provided to showcase the complete workflow:

```bash
# Run the demo script
./run_mesh_demo.sh
```

## Federation Governance

In addition to mesh computing, the ICN CLI provides commands for participating in federation governance:

```bash
# Create a new federation
icn-cli federation create --name "Solar Farm Cooperative" --key-path admin-key.json

# Join a federation
icn-cli federation join --id solar-farm-coop --key-path my-key.json

# Submit a governance proposal
icn-cli federation propose --federation-id solar-farm-coop --type "ConfigChange" \
  --params '{"key": "min_bid_timeout", "value": "12h"}' --key-path my-key.json

# Vote on a proposal
icn-cli federation vote --proposal-id prop-12345 --vote "approve" --key-path my-key.json
```

## Development

Contributions are welcome! To set up the development environment:

```bash
# Clone the repository
git clone https://github.com/InterCooperative/icn-v2.git
cd icn-v2

# Build the CLI
cargo build -p icn-cli

# Run tests
cargo test -p icn-cli
```

## License

This software is licensed under the terms of the Apache License 2.0. 