# ICN Federation Observability

This document provides a comprehensive guide to the observability and governance transparency tools in the InterCooperative Network (ICN).

## Overview

The observability layer enables transparent inspection of DAG activity, governance, policy enforcement, and quorum decisions across cooperatives, communities, and federations. By making these processes visible, we enhance trust, accountability, and inclusivity in the network.

## Key Features

### 1. DAG Viewer

The DAG Viewer allows you to visualize the Directed Acyclic Graph (DAG) for any scope within the network. It displays node metadata, payload information, and parent-child relationships.

#### Usage

```bash
# View DAG for a cooperative
icn scope dag-view --scope-type cooperative --scope-id coop-x

# View DAG for a community
icn scope dag-view --scope-type community --scope-id oakridge

# View DAG for a federation
icn scope dag-view --scope-type federation --scope-id fed-main

# Get JSON output
icn scope dag-view --scope-type cooperative --scope-id coop-x --output json

# Limit results
icn scope dag-view --scope-type cooperative --scope-id coop-x --limit 20
```

#### Visualization Model

The DAG Viewer renders each node with:
- CID (Content Identifier)
- Timestamp
- Signer DID
- Payload type and preview
- Parent CIDs
- Scope information

### 2. Policy Inspector

The Policy Inspector displays the current active policy for a scope and its update history.

#### Usage

```bash
# Inspect policy for a cooperative
icn scope inspect-policy --scope-type cooperative --scope-id coop-x

# Inspect policy for a community
icn scope inspect-policy --scope-type community --scope-id oakridge

# Get JSON output
icn scope inspect-policy --scope-type cooperative --scope-id coop-x --output json
```

#### Policy Snapshot Lifecycle

1. **Policy Creation**: Initial policy setup during scope creation
2. **Policy Updates**: Proposals to modify policy rules
3. **Update Approval**: Voting process to approve policy changes
4. **Policy Activation**: Approved policy becomes active

### 3. Quorum Proof Validator

The Quorum Proof Validator checks the validity of quorum proofs in DAG nodes, showing required vs. actual signers.

#### Usage

```bash
# Validate quorum for a DAG node
icn observe validate-quorum --cid zQm... --show-signers

# Get JSON output
icn observe validate-quorum --cid zQm... --show-signers --output json
```

#### Quorum Validation Process

1. **Proof Extraction**: Extracts quorum proof from the node
2. **Signer Verification**: Validates each signer's identity and role
3. **Threshold Check**: Ensures required number of signers is met
4. **Role Check**: Confirms signers have appropriate roles for the action

### 4. Governance Activity Log

The Governance Activity Log shows recent governance actions for a specific scope.

#### Usage

```bash
# View governance activity for a cooperative
icn scope activity-log --scope-type cooperative --scope-id coop-x --limit 10

# View governance activity for a community
icn scope activity-log --scope-type community --scope-id oakridge --limit 10

# Get JSON output
icn scope activity-log --scope-type cooperative --scope-id coop-x --output json
```

The log displays:
- Proposals submitted
- Votes cast
- Policies changed
- Federation joins
- Other governance actions

### 5. Federation Overview

The Federation Overview provides a high-level view of a federation's member cooperatives and communities.

#### Usage

```bash
# View federation overview
icn observe federation-overview --federation-id fed-main

# Get JSON output
icn observe federation-overview --federation-id fed-main --output json
```

The overview includes:
- Member cooperatives and communities
- Latest DAG head for each scope
- Federation metadata

## Integration with Other ICN Components

The observability layer integrates with:

1. **DAG Storage**: Reads DAG nodes from the underlying storage layer
2. **Identity System**: Resolves DIDs to verify signatures and authority
3. **Policy Framework**: Interprets policy documents and update history
4. **Governance System**: Tracks and displays governance actions

## JSON Output Format

All commands support JSON output via the `--output json` flag, making it easy to integrate with other tools or create custom visualizations.

## Usage in Development

For developers working with the ICN codebase, these observability tools can be invaluable for:

1. **Debugging**: Inspect DAG state during development
2. **Testing**: Validate policy enforcement and quorum in tests
3. **Demos**: Create visualizations for demonstrations
4. **Monitoring**: Track network activity during operation 