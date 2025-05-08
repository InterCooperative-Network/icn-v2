# Cooperative and Community Integration in ICN

This document outlines how cooperatives and communities are integrated as first-class functional units within the InterCooperative Network (ICN).

## Table of Contents

- [Overview](#overview)
- [Distinctions and Parallels](#distinctions-and-parallels)
- [Core Concepts](#core-concepts)
- [DAG Scoping Model](#dag-scoping-model)
- [P2P Topic Conventions](#p2p-topic-conventions)
- [Resource Tokens and Metering](#resource-tokens-and-metering)
- [Cross-Cooperative Job Execution](#cross-cooperative-job-execution)
- [Governance and Proposals](#governance-and-proposals)
- [CLI Commands](#cli-commands)
- [Examples](#examples)

## Overview

The ICN is designed to support multiple cooperatives and communities working together under the umbrella of a federation. Each maintains its own identity, governance, and resource allocation, while participating in shared federation activities.

Key components:
- **Scoped Identity**: Each cooperative/community has its own DID and DAG thread
- **Resource Tokens**: Track and exchange computational resources (primarily for cooperatives)
- **Scoped DAGs**: Proposals and governance happen in scope-specific DAG threads
- **Cross-Scope Operations**: Enabled through federation-level coordination

## Distinctions and Parallels

| Dimension             | **Cooperative**                                           | **Community**                                         |
| --------------------- | --------------------------------------------------------- | ----------------------------------------------------- |
| **Primary Function**  | Economic production & shared resource management          | Cultural identity, governance, or geographic grouping |
| **Scope Type**        | `NodeScope::Cooperative`                                  | `NodeScope::Community`                                |
| **Identity**          | DID assigned to worker-owned or user-owned entity         | DID representing neighborhood, cultural group, etc.   |
| **DAG Thread**        | Tracks proposals, jobs, tokens, and internal votes        | Tracks charters, social norms, proposals, resolutions |
| **Token Usage**       | Issues `ScopedResourceToken` for compute, bandwidth, etc. | May allocate social tokens or non-monetary reputation |
| **Governance**        | Emphasis on resource voting, job dispatch, ledger         | Emphasis on deliberation, moderation, community trust |
| **Interoperability**  | Trades resources across federations                       | Anchors social norms and resolves internal disputes   |
| **Integration Point** | Scheduler, Economics, Mesh, Governance                    | Governance, DAG, Identity, Federation Layer           |

While both cooperatives and communities use similar technical infrastructure, they serve different purposes in the network. Cooperatives focus on economic production and resource management, while communities emphasize social organization and cultural identity.

## Core Concepts

### NodeScope

The `NodeScope` enum represents the scope of a DAG node:

```rust
pub enum NodeScope {
    /// Node belongs to a Cooperative's DAG
    Cooperative,
    /// Node belongs to a Community's DAG
    Community,
    /// Node belongs to a Federation's DAG
    Federation,
}
```

### LineageAttestation

The `LineageAttestation` structure creates a cryptographically verifiable link between a cooperative/community DAG thread and the federation DAG:

```rust
pub struct LineageAttestation {
    /// Parent scope (usually Federation)
    pub parent_scope: NodeScope,
    /// Parent scope ID (federation_id)
    pub parent_scope_id: String,
    /// CID of the parent node in the parent scope's DAG
    pub parent_cid: Cid,
    
    /// Child scope (Cooperative or Community)
    pub child_scope: NodeScope,
    /// Child scope ID (coop_id or community_id)
    pub child_scope_id: String,
    /// CID of the child node in the child scope's DAG
    pub child_cid: Cid,
    
    /// Signatures from both scopes
    pub signatures: Vec<ScopeSignature>,
}
```

### ScopedResourceToken

The `ScopedResourceToken` structure represents a resource token owned by a cooperative:

```rust
pub struct ScopedResourceToken {
    /// Base token information
    pub token: ResourceToken,
    
    /// Cooperative or community ID this token belongs to
    pub scope_id: String,
    
    /// DID of the issuer
    pub issuer: Did,
    
    /// Signature of the issuer over the token data
    pub signature: Vec<u8>,
}
```

This pattern could be extended to community-specific tokens if needed.

## DAG Scoping Model

DAG nodes include scope information in their metadata:

```rust
pub struct DagNodeMetadata {
    /// Federation ID this node belongs to
    pub federation_id: String,
    /// Timestamp when this node was created
    pub timestamp: DateTime<Utc>,
    /// Optional label for the node
    pub label: Option<String>,
    /// Scope of this node (Cooperative, Community, or Federation)
    pub scope: NodeScope,
    /// ID of the scope (coop_id or community_id), null for Federation scope
    pub scope_id: Option<String>,
}
```

A cooperative or community's DAG thread can be anchored to the federation DAG using `LineageAttestation` objects. This allows for:

1. Independent evolution of scope-specific governance
2. Cryptographic proof of DAG integrity
3. Federation-level awareness of important decisions

## P2P Topic Conventions

The ICN uses the following convention for P2P topic names:

| Scope | Topic Format | Example |
|-------|--------------|---------|
| Federation | `icn/{federation_id}/mesh` | `icn/solar-farm-coop/mesh` |
| Cooperative | `icn/{federation_id}/{coop_id}/mesh` | `icn/solar-farm-coop/urban-farmers/mesh` |
| Community | `icn/{federation_id}/{community_id}/mesh` | `icn/solar-farm-coop/neighborhood-a/mesh` |
| Trade | `icn/{federation_id}/trade` | `icn/solar-farm-coop/trade` |

Messages are published to the appropriate topic based on their scope and purpose.

## Resource Tokens and Metering

Cooperatives track resources using the `icn-economics` crate:

1. **Resource Types**: CPU, RAM, storage, bandwidth, etc.
2. **Token Operations**: 
   - `credit`: Add tokens to a cooperative's balance
   - `debit`: Remove tokens from a cooperative's balance
   - `transfer`: Move tokens between cooperatives
3. **Transaction Records**: All token operations are recorded in the DAG

Each token has:
- A resource type
- An amount
- A cooperative ID (scope)
- An optional expiration date
- A cryptographic signature

While communities typically don't directly participate in resource token exchanges, they may have their own token systems for social governance.

## Cross-Cooperative Job Execution

Jobs can be submitted by one cooperative and executed by nodes in another:

1. **Job Submission**: Cooperative A submits a job with its ID
2. **Resource Verification**: Scheduler checks if Cooperative A has sufficient tokens
3. **Bid Selection**: Scheduler selects the best node, which may be in Cooperative B
4. **Resource Transfer**: Tokens are transferred from A to B
5. **Execution**: The job executes on Cooperative B's node
6. **Receipt**: An execution receipt is issued with both cooperative IDs

The `dispatch_cross_coop` method in the scheduler handles this flow.

## Governance and Proposals

Both cooperatives and communities can create and vote on proposals within their scoped DAG:

1. **Proposal Creation**: A member creates a proposal node in the scoped DAG
2. **Voting**: Members vote by creating vote nodes that reference the proposal
3. **Execution**: When enough votes are collected, the proposal can be executed
4. **Federation Anchoring**: Key decisions can be anchored to the federation DAG

Communities may also establish charters that define their governance rules and social norms.

## CLI Commands

The ICN CLI includes commands for working with both cooperatives and communities:

### Cooperative Commands

```
icn coop create --coop-id <ID> --federation-id <ID> --key <KEYFILE>
icn coop propose --coop-id <ID> --federation-id <ID> --title <TITLE> --content <FILE> --key <KEYFILE>
icn coop vote --coop-id <ID> --federation-id <ID> --proposal-cid <CID> --vote <yes|no> --key <KEYFILE>
icn coop export-thread --coop-id <ID> --federation-id <ID> --output <FILE>
icn coop join-federation --coop-id <ID> --federation-id <ID> --key <KEYFILE>
```

### Community Commands

```
icn community create --community-id <ID> --federation-id <ID> --key <KEYFILE>
icn community create-charter --community-id <ID> --federation-id <ID> --title <TITLE> --content <FILE> --key <KEYFILE>
icn community propose --community-id <ID> --federation-id <ID> --title <TITLE> --content <FILE> --key <KEYFILE>
icn community vote --community-id <ID> --federation-id <ID> --proposal-cid <CID> --vote <yes|no> --key <KEYFILE>
icn community export-thread --community-id <ID> --federation-id <ID> --output <FILE>
icn community join-federation --community-id <ID> --federation-id <ID> --key <KEYFILE>
```

## Examples

### Creating a Cooperative

```bash
# Generate a DID key for the cooperative
icn key-gen --output coop-key.json

# Create a new cooperative
icn coop create --coop-id urban-farmers --federation-id solar-farm-coop \
  --key coop-key.json --description "Urban farmers cooperative"
```

### Creating a Community

```bash
# Generate a DID key for the community
icn key-gen --output community-key.json

# Create a new community
icn community create --community-id neighborhood-a --federation-id solar-farm-coop \
  --key community-key.json --description "Neighborhood A Community"

# Create a community charter
icn community create-charter --community-id neighborhood-a --federation-id solar-farm-coop \
  --title "Neighborhood A Charter" --content charter.md --key community-key.json
```

### Cross-Cooperative Job Execution

```bash
# Cooperative A: Submit a job
icn mesh submit-job --wasm-file computation.wasm --coop-id urban-farmers \
  --federation-id solar-farm-coop --key coop-a-key.json \
  --resource-requirement "RamMb=1024" --resource-requirement "CpuCores=2"

# Cooperative B: Node executes the job (automatic via scheduler)
# ...

# Verify the execution
icn receipt show --receipt-ref <CID>
```

### Cooperative Governance

```bash
# Create a proposal
icn coop propose --coop-id urban-farmers --federation-id solar-farm-coop \
  --title "Increase compute allocation" --content proposal.md --key member-key.json

# Vote on the proposal
icn coop vote --coop-id urban-farmers --federation-id solar-farm-coop \
  --proposal-cid <CID> --vote yes --comment "Good idea" --key another-member-key.json

# Export the thread to see all proposals and votes
icn coop export-thread --coop-id urban-farmers --federation-id solar-farm-coop \
  --output coop-thread.json
```

### Community Governance

```bash
# Create a community proposal
icn community propose --community-id neighborhood-a --federation-id solar-farm-coop \
  --title "Community garden initiative" --content garden-proposal.md --key member-key.json

# Vote on the community proposal
icn community vote --community-id neighborhood-a --federation-id solar-farm-coop \
  --proposal-cid <CID> --vote yes --comment "Great for our neighborhood" --key another-member-key.json

# Export the community thread
icn community export-thread --community-id neighborhood-a --federation-id solar-farm-coop \
  --output community-thread.json
``` 