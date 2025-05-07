# SharedDagStore Usage Guide

## Overview

The `SharedDagStore` is a wrapper around the `DagStore` trait that provides thread-safe, shared access to a DAG store implementation. It resolves the common problem of needing to share mutable access to a DAG store across multiple components or services while maintaining proper synchronization and avoiding Rust's ownership conflicts.

## Key Features

- Thread-safe access to a `DagStore` implementation
- Proper locking semantics for concurrent access
- Easy cloning and sharing between components
- Compatible with all `DagStore` implementations
- Maintains the same interface as `DagStore` with async methods

## When to Use SharedDagStore

You should use `SharedDagStore` when:

1. Multiple components need access to the same DAG store
2. Components might need to mutate the store (e.g., add nodes)
3. You need to pass the store reference across thread boundaries
4. You need to store the DAG store in a struct with a longer lifetime

## Basic Usage

### Creating a SharedDagStore

```rust
use icn_types::dag::{DagStore, SharedDagStore, memory::MemoryDagStore};

// Create a backing store implementation
let memory_store = MemoryDagStore::new();

// Wrap it in a SharedDagStore
let dag_store = SharedDagStore::new(
    Box::new(memory_store) as Box<dyn DagStore + Send + Sync>
);

// The dag_store can now be safely cloned and shared
let dag_store_clone = dag_store.clone();
```

### Using SharedDagStore in a Component

```rust
struct MyComponent {
    dag_store: SharedDagStore,
    // other fields...
}

impl MyComponent {
    pub fn new(dag_store: SharedDagStore) -> Self {
        Self {
            dag_store,
            // initialize other fields...
        }
    }
    
    pub async fn add_node(&self, node: SignedDagNode) -> Result<Cid, DagError> {
        // No mut reference needed here since SharedDagStore handles locking internally
        self.dag_store.add_node(node).await
    }
    
    pub async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, DagError> {
        self.dag_store.get_node(cid).await
    }
}
```

### Passing to Multiple Components

```rust
async fn setup_components() -> Result<(), Error> {
    let memory_store = MemoryDagStore::new();
    let dag_store = SharedDagStore::new(
        Box::new(memory_store) as Box<dyn DagStore + Send + Sync>
    );
    
    // Create multiple components that share the same store
    let component1 = MyComponent::new(dag_store.clone());
    let component2 = AnotherComponent::new(dag_store.clone());
    let component3 = YetAnotherComponent::new(dag_store);
    
    // All components can now read from and write to the same store
    // with proper synchronization
    
    Ok(())
}
```

## Advanced Usage

### From Existing Arc<Box<dyn DagStore>>

If you have an existing `Arc<Box<dyn DagStore>>` (e.g., from legacy code), you can convert it to a `SharedDagStore`:

```rust
let old_style_store: Arc<Box<dyn DagStore + Send + Sync>> = get_legacy_store();
let shared_store = SharedDagStore::from_arc(old_style_store);
```

### Using in Tokio Tasks

`SharedDagStore` works well with Tokio tasks:

```rust
let dag_store = shared_dag_store.clone();
tokio::spawn(async move {
    // Use dag_store in a new task
    let node = create_some_node();
    let cid = dag_store.add_node(node).await?;
    // Do something with cid...
});
```

### Using with CapabilityIndex and Scheduler

The `SharedDagStore` is designed to work well with components like the `CapabilityIndex` and `Scheduler`:

```rust
// Create a shared store
let dag_store = SharedDagStore::new(
    Box::new(MemoryDagStore::new()) as Box<dyn DagStore + Send + Sync>
);

// Use it with CapabilityIndex
let cap_index = Arc::new(CapabilityIndex::new(dag_store.clone()));

// Use it with Scheduler
let scheduler = Scheduler::new(
    "federation-id".to_string(),
    cap_index.clone(),
    dag_store.clone(),
    "scheduler-did".parse().unwrap(),
);
```

## Common Patterns

### Signing and Adding DAG Nodes

```rust
async fn create_and_add_node(
    dag_store: &SharedDagStore,
    did_key: &DidKey,
    payload: DagPayload,
    federation_id: &str,
) -> Result<Cid, anyhow::Error> {
    // Build the node
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did_key.did().clone())
        .with_federation_id(federation_id.to_string())
        .with_label("Example".to_string())
        .build()?;
    
    // Serialize for signing
    let node_bytes = serde_json::to_vec(&node)?;
    
    // Sign the node
    let signature = did_key.sign(&node_bytes);
    
    // Create a signed node
    let signed_node = SignedDagNode {
        node,
        signature,
        cid: None, // Will be computed when added
    };
    
    // Add to DAG and get CID
    let cid = dag_store.add_node(signed_node).await?;
    
    Ok(cid)
}
```

### Traversing the DAG

```rust
async fn walk_dag_from_tip(
    dag_store: &SharedDagStore,
    tip_cid: &Cid,
) -> Result<Vec<SignedDagNode>, DagError> {
    let mut nodes = Vec::new();
    let mut queue = vec![tip_cid.clone()];
    let mut visited = std::collections::HashSet::new();
    
    while let Some(cid) = queue.pop() {
        if visited.contains(&cid) {
            continue;
        }
        
        let node = dag_store.get_node(&cid).await?;
        visited.insert(cid);
        
        // Add parents to queue
        for parent in &node.node.parents {
            if !visited.contains(parent) {
                queue.push(parent.clone());
            }
        }
        
        nodes.push(node);
    }
    
    Ok(nodes)
}
```

## Best Practices

1. **Clone Sparingly**: While `SharedDagStore` is designed for cloning, minimize unnecessary clones to reduce overhead.

2. **Avoid Long-Held Locks**: The internal mutex should not be held for long periods. Complete your operations quickly and release the lock.

3. **Error Handling**: Always properly handle errors from DAG operations and propagate them appropriately.

4. **Verification**: When retrieving nodes, verify their signatures and CIDs if they came from untrusted sources.

5. **Background Tasks**: For long-running background tasks that monitor or process the DAG, use a clone of the store rather than trying to hold a reference.

## FAQ

### Q: Will SharedDagStore work with any DagStore implementation?

A: Yes, as long as the implementation is Send + Sync, it can be wrapped in a SharedDagStore.

### Q: Is there a performance penalty for using SharedDagStore?

A: There is a small overhead from the Mutex locking, but it's generally negligible compared to the actual storage operations.

### Q: Can I use SharedDagStore with types other than DagStore?

A: The pattern can be adapted for other traits that have similar mutability requirements, but this specific implementation is for DagStore.

### Q: Is SharedDagStore safe for concurrent use across threads?

A: Yes, that's its primary purpose. The internal tokio::sync::Mutex ensures thread-safety. 