use criterion::{criterion_group, criterion_main, Criterion, black_box};
use icn_runtime::dag_indexing::{DagIndex, SledDagIndex};
use icn_core_types::Cid;
use icn_identity_core::Did;
use icn_types::dag::{DagNode, DagNodeMetadata, NodeScope, SignedDagNode, DagPayload};
use icn_types::dag::memory::MemoryDagStore; // Adjusted path
use icn_types::dag::DagStore;
use multihash::{Code, MultihashDigest};
use rand::{rngs::StdRng, Rng, SeedableRng};
use tempfile::tempdir;
use std::sync::Arc;
use std::str::FromStr; // Needed for Did::from_str
use ed25519_dalek::Signature; // Needed for SignedDagNode

/// Generates a deterministic mock DID
fn mock_did(index: u32) -> Did {
    // Assuming Did::from_str exists and works like this
    Did::from_str(&format!("did:icn:benchmark-{}", index % 10)).unwrap()
}

/// Generates a deterministic CID from data
fn mock_cid(data: &[u8]) -> Cid {
    // Assuming Cid::new_v1 and multihash usage is correct based on icn-core-types
    Cid::new_v1(0x55, Code::Sha2_256.digest(data))
}

/// Generates a DAG node with mock metadata
fn make_signed_node(index: u32) -> SignedDagNode {
    let author = mock_did(index);
    let metadata = DagNodeMetadata {
        federation_id: "test-fed".into(),
        timestamp: chrono::Utc::now(),
        label: Some(format!("node-{}", index)),
        scope: if index % 2 == 0 {
            // Using Cooperative variant as Community requires scope_id
            NodeScope::Cooperative
        } else {
            NodeScope::Cooperative // Both need scope_id, let's use Coop for simplicity
        },
        scope_id: Some(format!("scope-{}", index % 5)), // Added scope_id
    };

    let node = DagNode {
        author: author.clone(),
        metadata,
        // Adjusted payload to fit DagNode definition
        payload: DagPayload::Raw(format!("payload-{}", index).into_bytes()),
        parents: Vec::new(), // Keep parents empty for simplicity in this benchmark setup
    };

    // Create a placeholder Signature (64 bytes of zeros)
    let empty_sig_bytes = [0u8; 64];
    let signature = Signature::from_bytes(&empty_sig_bytes); // Assuming Signature::from_bytes exists

    SignedDagNode {
        node,
        signature, // Use the actual Signature type
        cid: None, // CID will be calculated by add_node
    }
}

/// Sets up both the DagStore and SledDagIndex
async fn setup_index_and_store(count: usize) -> (MemoryDagStore, SledDagIndex, Did, NodeScope) {
    let mut dag_store = MemoryDagStore::new(); // Use new() instead of default()
    let temp_dir = tempdir().unwrap();
    let index = SledDagIndex::new(temp_dir.path().to_str().unwrap()).unwrap();

    let mut rng = StdRng::seed_from_u64(42); // For potential future random elements
    let mut target_did = None;
    let mut target_scope = None;

    for i in 0..count as u32 {
        let node = make_signed_node(i);
        let node_for_index = node.node.clone(); // Clone the inner DagNode for indexing
        // Use the async version of add_node
        let cid = dag_store.add_node(node).await.unwrap();

        // Index it - requires the inner DagNode
        index.add_node_to_index(&cid, &node_for_index).unwrap();

        if i == count as u32 / 2 {
            target_did = Some(node_for_index.author.clone());
            target_scope = Some(node_for_index.metadata.scope.clone());
        }
    }

    (dag_store, index, target_did.unwrap(), target_scope.unwrap())
}

/// Benchmark querying by DID
fn bench_query_by_did(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap(); // Create a tokio runtime
    let (dag_store, index, target_did, _) = rt.block_on(setup_index_and_store(10_000));

    let mut group = c.benchmark_group("query_by_did_10k");

    group.bench_function("index", |b| {
        b.iter(|| {
            // Use black_box to prevent optimizations
            let result = index.nodes_by_did(black_box(&target_did)).unwrap();
            black_box(result);
        })
    });

    group.bench_function("linear_scan", |b| {
        b.iter(|| {
            // Use black_box
            let target = black_box(&target_did);
            // Need to run the async get_ordered_nodes within the runtime
            let nodes = rt.block_on(dag_store.get_ordered_nodes()).unwrap();
            let result = nodes
                .into_iter()
                .filter(|n| n.node.author == *target)
                .collect::<Vec<_>>();
            black_box(result);
        })
    });

    group.finish();
}

/// Benchmark querying by scope
fn bench_query_by_scope(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap(); // Create a tokio runtime
    let (dag_store, index, _, target_scope) = rt.block_on(setup_index_and_store(10_000));

    let mut group = c.benchmark_group("query_by_scope_10k");

    group.bench_function("index", |b| {
        b.iter(|| {
            // Use black_box
            let result = index.nodes_by_scope(black_box(&target_scope)).unwrap();
            black_box(result);
        })
    });

    group.bench_function("linear_scan", |b| {
        b.iter(|| {
            // Use black_box
            let target = black_box(&target_scope);
            // Need to run the async get_ordered_nodes within the runtime
            let nodes = rt.block_on(dag_store.get_ordered_nodes()).unwrap();
            let result = nodes
                .into_iter()
                .filter(|n| n.node.metadata.scope == *target)
                .collect::<Vec<_>>();
            black_box(result);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_query_by_did, bench_query_by_scope);
criterion_main!(benches); 