# Scoped Policy Enforcement in ICN v3

This document explains the scoped policy enforcement mechanisms in the Inter-Cooperative Network (ICN) v3, with a focus on the persistent DAG storage layer and lineage verification.

## Overview

The ICN enforces scoped policies through a combination of:

1. **Persistent DAG Storage**: A reliable, tamper-resistant record of all operations
2. **Cryptographic Verification**: Ensuring node authenticity and integrity
3. **Lineage Traversal**: Validating the complete ancestry of operations
4. **Scoped Authorization**: Enforcing permissions within appropriate contexts

## DAG Persistence and Structure

The DAG (Directed Acyclic Graph) is the core data structure for policy enforcement. It consists of:

```
                        ┌─────────────┐
                        │ Federation  │
                        │  Creation   │
                        └──────┬──────┘
                               │
              ┌────────────────┴─────────────────┐
              │                                  │
      ┌───────▼──────┐                  ┌────────▼─────┐
      │ Cooperative  │                  │ Cooperative  │
      │  Creation 1  │                  │  Creation 2  │
      └───────┬──────┘                  └──────┬───────┘
              │                                │
    ┌─────────┴──────────┐             ┌───────┴──────┐
    │                    │             │              │
┌───▼───┐           ┌────▼───┐    ┌────▼───┐     ┌────▼───┐
│ Vote  │           │Resource│    │Proposal│     │Resource│
│       │           │ Policy │    │        │     │ Policy │
└───────┘           └────────┘    └────────┘     └────────┘
```

Each node in the DAG represents an operation (creation, vote, policy, etc.) and contains:

- **Header**: Metadata including type, timestamp, parents, scope, and creator
- **Payload**: Operation-specific data
- **Signature**: Cryptographic proof of authenticity

## Lineage Verification Process

When a module is executed, the runtime verifies its complete lineage:

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│ DAG Storage  │     │Lineage Verifier│     │WASM Runtime  │
└──────┬───────┘     └───────┬───────┘     └──────┬───────┘
       │                     │                    │
       │  request node       │                    │
       │◄────────────────────┤                    │
       │                     │                    │
       │  return node        │                    │
       ├────────────────────►│                    │
       │                     │                    │
       │  get parents        │                    │
       │◄────────────────────┤                    │
       │                     │                    │
       │  return parents     │                    │
       ├────────────────────►│                    │
       │                     │                    │
       │     verify          │                    │
       │     signatures      │                    │
       │     and scope       │                    │
       │     authority       │                    │
       │                     │                    │
       │                     │  lineage verified  │
       │                     ├───────────────────►│
       │                     │                    │
       │                     │                    │ execute
       │                     │                    │ module
       │                     │                    │
```

## Scoped Authorization

Scopes are hierarchical and define authorization boundaries:

```
┌───────────────────────────────────────┐
│            Global Scope               │
│                                       │
│  ┌────────────────────────────────┐   │
│  │        Federation Scope        │   │
│  │                                │   │
│  │  ┌─────────────┐  ┌─────────┐  │   │
│  │  │ Cooperative │  │Coopera- │  │   │
│  │  │   Scope 1   │  │tive Sc. 2│  │   │
│  │  └─────────────┘  └─────────┘  │   │
│  │                                │   │
│  └────────────────────────────────┘   │
│                                       │
└───────────────────────────────────────┘
```

Each scope:
- Has its own set of authorized identities
- May have parent scopes (inheriting authority)
- Contains scope-specific operations

## How Verification Works

1. **Signature Verification**: The signature of each node is verified using the creator's public key
2. **Parent Existence**: All parent references must point to existing nodes
3. **Scope Authority**: The creator must be authorized in the node's scope
4. **Recursive Verification**: All ancestors must also be valid

## Code Implementation

The core components:

- **RocksDbDagStore**: Persistent storage for DAG nodes
- **NodeScope**: Authorization context with identity permissions
- **DagVerifiedExecutor**: Runtime integration for lineage validation

Key verification methods:

```rust
// Verify a node's lineage against a scope
async fn verify_lineage(&self, cid: &DAGNodeID, scope: &NodeScope) -> Result<bool, DagStoreError>;

// Execute a WASM module with lineage verification
async fn execute_wasm_module(
    &self,
    cid: &DAGNodeID,
    scope: &NodeScope,
) -> Result<ExecutionResult, RuntimeExecutionError>;
```

## Policy Enforcement Checkpoints

The system verifies lineage at critical points:

1. **When Appending Nodes**: Ensures proper parent references and authorization
2. **During Module Execution**: Requires valid lineage before execution
3. **For Cross-Scope Operations**: Enforces proper scope hierarchy
4. **For Governance Decisions**: Validates voter authorization

## Federation Bootstrap Mechanics

A new federation is established through:

1. **Federation Creation**: Root node establishing the federation scope
2. **Cooperative Joining**: Cooperative nodes referencing the federation
3. **Scope Hierarchy**: Establishing parent-child relationships
4. **Authorization Setup**: Registering identities within appropriate scopes

## Practical Examples

### Example 1: Simple Scoped Authorization

```rust
// 1. Create a federation scope
let federation_scope = "federation:test";
let mut scope = NodeScope::new(federation_scope.to_string());

// 2. Add authorized identities
scope.add_identity(federation_identity_id.clone());

// 3. Create a federation node
let node = create_node(
    DAGNodeType::FederationCreation,
    HashSet::new(), // No parents
    federation_scope,
    &federation_identity,
    federation_payload
);

// 4. Store the node
let node_id = store.append_node(node).await?;

// 5. Verify lineage for execution
let is_valid = store.verify_lineage(&node_id, &scope).await?;
```

### Example 2: Cross-Scope Verification

```rust
// 1. Create a parent-child scope relationship
let mut cooperative_scope = NodeScope::new("cooperative:coop1".to_string());
cooperative_scope.with_parent_scopes(
    HashSet::from([federation_scope.to_string()])
);

// 2. Create a node with a parent in the federation scope
let mut parents = HashSet::new();
parents.insert(federation_node_id.clone());

let coop_node = create_node(
    DAGNodeType::CooperativeCreation,
    parents,
    cooperative_scope_id,
    &cooperative_identity,
    cooperative_payload
);

// 3. The federation can verify the cooperative node
let is_valid = store.verify_lineage(&coop_node_id, &federation_scope).await?;
```

## Security Considerations

- **Node Immutability**: Once written, nodes cannot be modified
- **Cryptographic Proof**: All nodes must have valid signatures
- **Complete Verification**: The entire lineage must be valid
- **Proper Scoping**: Operations must occur in the appropriate scope

## Auditability

The persistent DAG provides:

- **Complete History**: All operations are recorded
- **Verifiable Lineage**: Chain of custody can be validated
- **Scope Boundaries**: Clear authorization contexts
- **Cryptographic Evidence**: Tamper-proof records

By enforcing these mechanisms, ICN v3 ensures that all operations follow proper governance rules and authorization boundaries, creating a reliable foundation for federation activities. 