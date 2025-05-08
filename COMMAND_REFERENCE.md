# ICN CLI Command Reference

This document provides a comprehensive reference for all commands available in the ICN CLI.

## Global Options

These options apply to all commands:

```
-v, --verbose           Increase verbosity of output (can be used multiple times)
--help                  Show help for a command
--version               Show version information
```

## Key Management

### key-gen

Generate a new DID key.

```bash
cargo run -p icn-cli -- key-gen --output <file>
```

Options:
- `--output <file>`: Output file to save the key (defaults to ~/.icn/key.json)
- `--key-type <type>`: Key type (default: ed25519)
- `--force`: Overwrite existing file

### key-gen import

Import an existing DID key.

```bash
cargo run -p icn-cli -- key-gen import --file <file> --output <output>
```

Options:
- `--file <file>`: Path to key file to import
- `--output <file>`: Output file (defaults to ~/.icn/key.json)
- `--force`: Overwrite existing file

### key-gen info

Display information about a DID key.

```bash
cargo run -p icn-cli -- key-gen info --file <file>
```

Options:
- `--file <file>`: Path to key file (defaults to ~/.icn/key.json)

## Federation Commands

### federation init

Bootstrap a new federation with a genesis TrustBundle.

```bash
cargo run -p icn-cli -- federation init --name <name> [options]
```

Options:
- `--name <name>`: Name of the federation
- `--output-dir <dir>`: Directory to output federation files
- `--dry-run`: Run in dry-run mode without writing files
- `--participant <file>`: Paths to participant key files (can be used multiple times)
- `--quorum <type>`: Quorum type (all, majority, threshold:<num>)
- `--export-keys`: Export the federation keys to a file (default: true)
- `--key-format <format>`: Key format for exported keys (jwk or base58)

### federation submit-proposal

Submit a new proposal to a federation.

```bash
cargo run -p icn-cli -- federation submit-proposal --file <file> --to <url> [options]
```

Options:
- `--file <file>`: File containing the proposal in TOML format
- `--to <url>`: Federation node URL to submit the proposal to
- `--key <file>`: Path to the key file for signing the proposal
- `--output <file>`: Output file to save the proposal details

### federation vote

Vote on an existing federation proposal.

```bash
cargo run -p icn-cli -- federation vote --proposal-id <id> [options]
```

Options:
- `--proposal-id <id>`: ID of the proposal to vote on
- `--decision <decision>`: Vote decision (approve/reject, default: approve)
- `--reason <reason>`: Reason for the vote
- `--key <file>`: Path to the key file for signing the vote
- `--to <url>`: Federation node URL to submit the vote to

### federation execute

Execute an approved proposal.

```bash
cargo run -p icn-cli -- federation execute --proposal-id <id> [options]
```

Options:
- `--proposal-id <id>`: ID of the proposal to execute
- `--key <file>`: Path to the key file for signing the execution
- `--to <url>`: Federation node URL to execute the proposal on
- `--output <file>`: Output file to save the execution receipt

### federation export

Export a federation to a CAR archive for cold-sync.

```bash
cargo run -p icn-cli -- federation export --federation-dir <dir> [options]
```

Options:
- `--federation-dir <dir>`: Path to the federation directory
- `--output <file>`: Output path for the CAR archive
- `--include-keys`: Include keys in the export (warning: contains private keys)
- `--include <path>`: Include additional files or directories in the export

### federation import

Import a federation from a CAR archive.

```bash
cargo run -p icn-cli -- federation import --archive-path <file> [options]
```

Options:
- `--archive-path <file>`: Path to the CAR archive file
- `--output-dir <dir>`: Directory to output the imported federation files
- `--verify-only`: Perform verification only without writing files
- `--override-existing`: Override existing federation with the same name
- `--no-keys`: Skip importing federation keys

## Mesh Network Commands

### mesh submit-job

Submit a job to the mesh network.

```bash
cargo run -p icn-cli -- mesh submit-job --manifest <file> [options]
```

Options:
- `--manifest <file>`: Path to the job manifest file
- `--to <url>`: URL of the node to submit the job to
- `--key <file>`: Path to the key file for signing the submission

### mesh get-bids

Get bids for a job.

```bash
cargo run -p icn-cli -- mesh get-bids --job-id <id> [options]
```

Options:
- `--job-id <id>`: ID of the job to get bids for
- `--limit <num>`: Maximum number of bids to return
- `--sort-by <field>`: Field to sort bids by (price, confidence)

### mesh select-bid

Select a bid for execution.

```bash
cargo run -p icn-cli -- mesh select-bid --job-id <id> --bid-id <bid> [options]
```

Options:
- `--job-id <id>`: ID of the job
- `--bid-id <bid>`: ID of the bid to select
- `--key <file>`: Path to the key file for signing the selection

### mesh job-status

Check the status of a job.

```bash
cargo run -p icn-cli -- mesh job-status --job-id <id> [options]
```

Options:
- `--job-id <id>`: ID of the job to check
- `--to <url>`: URL of the node to query

### mesh verify-receipt

Verify an execution receipt.

```bash
cargo run -p icn-cli -- mesh verify-receipt --receipt-id <id> [options]
```

Options:
- `--receipt-id <id>`: ID or CID of the receipt to verify

## DAG Commands

### dag sync-p2p

Advanced DAG sync commands with libp2p support.

```bash
cargo run -p icn-cli -- dag sync-p2p [command] [options]
```

Subcommands:
- `genesis`: Create a new federation and start a genesis node
- `join`: Join an existing federation
- `start`: Start a federation node

### observe dag-view

View the contents of a DAG.

```bash
cargo run -p icn-cli -- observe dag-view --dag-dir <dir> [options]
```

Options:
- `--dag-dir <dir>`: Directory containing the DAG data
- `--output <file>`: Output file for the DAG visualization
- `--format <format>`: Output format (json, dot, mermaid)

## Policy Commands

### policy create

Create a new trust policy.

```bash
cargo run -p icn-cli -- policy create --name <name> [options]
```

Options:
- `--name <name>`: Name of the policy
- `--rules <file>`: Path to the rules file
- `--output <file>`: Output file for the policy

### policy verify

Verify a credential against a policy.

```bash
cargo run -p icn-cli -- policy verify --policy <file> --credential <file> [options]
```

Options:
- `--policy <file>`: Path to the policy file
- `--credential <file>`: Path to the credential file
- `--verbose`: Show detailed verification results 