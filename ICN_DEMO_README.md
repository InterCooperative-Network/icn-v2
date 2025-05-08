# ICN Federation Demo

This is a simplified demo that shows the key features of the Interoperable Compute Network (ICN) federation system.

## Prerequisites

- Docker and Docker Compose
- Rust/Cargo (latest stable version)
- Bash shell environment

## Running the Demo

Simply execute the demo script:

```bash
# Make the script executable
chmod +x icn_demo.sh

# Run the demo
./icn_demo.sh
```

## What the Demo Shows

The demo demonstrates a complete ICN federation workflow:

1. Building the ICN workspace
2. Generating keys for federation participants
3. Creating a federation proposal
4. Setting up federation nodes (using Docker)
5. Submitting the proposal to create a federation
6. Voting on the proposal by federation validators
7. Executing the approved proposal

## Key Concepts

- **Federation**: A group of nodes working together with shared governance
- **Proposals**: Changes to the federation configuration that require voting
- **Validation**: Nodes validating proposals through cryptographic signatures
- **Consensus**: The process of reaching agreement on federation changes

## Troubleshooting

If you encounter issues:

1. Ensure Docker is running (`docker ps` should work)
2. Check if ports 5001-5003 are available
3. If a previous demo is stuck, run `cd demo/federation && docker-compose down`
4. Remove any previous demo files: `rm -rf .demo`

## Next Steps

After running the demo, explore more advanced ICN features:

- Submit computational jobs to the network
- Explore the DAG structure of the federation
- Try different validator configurations
- Set up custom federation policies

Refer to the main README.md for full documentation on the ICN system. 