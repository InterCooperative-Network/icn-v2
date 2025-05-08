# ICN Persistent DAG Storage

This crate provides a persistent, RocksDB-based DAG (Directed Acyclic Graph) storage implementation for the Inter-Cooperative Network (ICN). It ensures reliable storage, secure cryptographic validation, and efficient lineage traversal for scoped policy verification.

## Features

- **Persistent Storage**: Stores DAG nodes using RocksDB for durability
- **Cryptographic Validation**: Verifies signatures and integrity of all nodes
- **Lineage Verification**: Validates node ancestry for proper scoped policy enforcement
- **Efficient Indexing**: Uses prefix-based keys for optimized lookups
- **Runtime Integration**: Enforces policy at the execution layer

## Architecture

The storage layer consists of several key components:

### `RocksDbDagStore`

The main storage implementation that handles:
- Node serialization and persistence
- Cryptographic validation of nodes
- Lineage tracking and verification
- Scope-based authorization

### `NodeScope`

Represents an authorization scope with:
- Identifiers for the scope
- Authorized identities for the scope
- Parent-child scope relationships
- Additional constraints for authorization

### `DagVerifiedExecutor`

Runtime integration that:
- Verifies node lineage before execution
- Ensures that only properly authorized operations are executed
- Provides a bridge between storage and runtime execution

## Usage Example

```rust
use icn_storage::{
    RocksDbDagStore, 
    DagStore, 
    NodeScope, 
    ConnectionConfig,
    DagVerifiedExecutor
};
use std::sync::Arc;

async fn example() -> Result<(), Box<dyn std::error::Error>> {
    // Create a configuration
    let config = ConnectionConfig {
        path: "./dag_storage".into(),
        write_buffer_size: Some(64 * 1024 * 1024), // 64MB
        max_open_files: Some(1000),
        create_if_missing: true,
    };
    
    // Initialize the store
    let store = Arc::new(RocksDbDagStore::new(config));
    store.init().await?;
    
    // Define a scope for authorization
    let mut scope = NodeScope::new("federation:test".to_string());
    scope.add_identity("federation_identity123".to_string());
    store.register_scope(scope.clone()).await?;
    
    // Create and add a node
    // ... (see test cases for complete examples)
    
    // Create an executor for runtime integration
    let executor = DagVerifiedExecutor::new(store.clone());
    
    // Execute a module with lineage verification
    let result = executor.execute_wasm_module(&module_id, &scope).await?;
    println!("Execution result: {:?}", result);
    
    Ok(())
}
```

## Integration with Runtime

This storage layer integrates with the ICN runtime by enforcing scoped policy checks:

1. Before any WASM module execution, the lineage of the module is verified
2. Verification ensures the node and all its ancestors have valid signatures
3. Node creators must be authorized within the appropriate scope
4. Cross-scope references must follow proper parent-child relationships

This ensures that only properly authorized operations can be executed, creating a reliable trust anchor for federation activities.

## Testing

The crate includes comprehensive tests for all functionality:
- Basic storage operations
- Node lineage verification
- Runtime integration
- Authorization enforcement

Run the tests with:

```bash
cargo test -p icn-storage -- --nocapture
``` 