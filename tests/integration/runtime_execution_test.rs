// Integration test for actual WASM execution via icn-runtime

// Placeholder for necessary imports - to be expanded
use icn_identity_core::{
    did::DidKey,
    vc::{
        ProposalCredential,
        ProposalSubject,
        ProposalType,
        ProposalStatus,
        VotingThreshold,
        VotingDuration,
        VoteCredential,
        VoteSubject,
        VoteDecision,
        execution_receipt::{
            ExecutionReceipt,
            ExecutionSubject,
            ExecutionScope,
            ExecutionStatus,
        },
    },
    QuorumEngine,
    QuorumOutcome,
};
use icn_types::{
    dag::{
        memory::MemoryDagStore, DagError, DagEvent, DagStore, EventId, EventPayload, EventType,
    },
    Cid, Did,
};
use icn_runtime::{
    abi::context::HostContext,
    config::ExecutionConfig,
    engine::{ContextExtension, ModernWasmExecutor, ExecutionResult},
    policy::{MembershipIndex, PolicyLoader},
};

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use wasmtime;
use multihash::{Code, MultihashDigest};

/// Mock execution context for testing WASM module behavior.
#[derive(Clone)]
struct TestContext {
    logs: Arc<StdRwLock<Vec<String>>>,
    config: ExecutionConfig,
    dag_store: MemoryDagStore,
    federation_did: Did,
    federation_key: DidKey,
    caller_did: Did,
    node_did: Did,
    policy_loader_mock: Option<Arc<dyn PolicyLoader + Send + Sync>>,
    membership_index_mock: Option<Arc<dyn MembershipIndex + Send + Sync>>,
    last_error: Arc<StdRwLock<Option<String>>>,
}

impl TestContext {
    fn new() -> Self {
        let fed_key = DidKey::new();
        let call_key = DidKey::new();
        let node_key = fed_key.clone();

        let mut config = ExecutionConfig::default();
        config.auto_issue_receipts = true;
        config.anchor_receipts = true;
        config.receipt_export_dir = None;

        Self {
            logs: Arc::new(StdRwLock::new(vec![])),
            config,
            dag_store: MemoryDagStore::default(),
            federation_key: fed_key.clone(),
            federation_did: fed_key.did(),
            caller_did: call_key.did(),
            node_did: node_key.did(),
            policy_loader_mock: None,
            membership_index_mock: None,
            last_error: Arc::new(StdRwLock::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl HostContext for TestContext {
    fn read_string(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32, _len: i32) -> Result<String> {
        unimplemented!("TestContext::read_string")
    }

    fn write_string(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32, _max_len: i32, _s: &str) -> Result<i32> {
        unimplemented!("TestContext::write_string")
    }

    fn malloc(&self, _caller: &mut impl wasmtime::AsContextMut, _size: i32) -> Result<i32> {
        unimplemented!("TestContext::malloc")
    }

    fn free(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32) -> Result<()> {
        unimplemented!("TestContext::free")
    }

    fn get_caller_did(&self) -> Did {
        self.caller_did.clone()
    }

    fn log_message(&self, message: &str) {
        self.logs.write().unwrap().push(message.to_string());
    }

    async fn verify_signature(&self, _did: &Did, _message: &[u8], _signature: &[u8]) -> bool {
        true
    }
    
    fn set_error(&self, message: String) {
        *self.last_error.write().unwrap() = Some(message);
    }

    fn get_error(&self) -> Option<String> {
        self.last_error.read().unwrap().clone()
    }
    
    fn clear_error(&self) {
        *self.last_error.write().unwrap() = None;
    }

    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>> {
        self.policy_loader_mock.clone()
    }

    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>> {
        self.membership_index_mock.clone()
    }
}

impl ContextExtension for TestContext {
    fn get_execution_config(&self) -> &ExecutionConfig {
        &self.config
    }

    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn DagStore + Send + Sync)> {
        Some(&mut self.dag_store)
    }

    fn node_did(&self) -> Option<&Did> {
        Some(&self.node_did)
    }

    fn federation_did(&self) -> Option<&Did> {
        Some(&self.federation_did)
    }

    fn caller_did(&self) -> Option<&Did> {
        Some(&self.caller_did)
    }

    fn federation_keypair(&self) -> Option<DidKey> {
        Some(self.federation_key.clone())
    }

    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>> {
        self.membership_index_mock.clone()
    }

    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>> {
        self.policy_loader_mock.clone()
    }
}

#[tokio::test]
async fn test_runtime_execution_and_receipt_issuance() -> Result<()> {
    // Initialize logging for the test, if not already globally initialized.
    // This helps in seeing logs from the executor itself if needed.
    let _ = env_logger::builder().is_test(true).try_init();

    // 1. Load WASM module
    // Ensure `tests/fixtures/` directory exists and `log_test.wasm` is inside.
    // The path is relative to the Cargo.toml of the `icn-integration-tests-dag` package.
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/log_test.wasm");
    if !wasm_path.exists() {
        panic!("WASM test file not found at {:?}. Please compile and place log_test.wasm.", wasm_path);
    }
    let wasm_bytes = fs::read(&wasm_path)?;

    // 2. Calculate CID for the module
    let hash = Code::Sha2_256.digest(&wasm_bytes);
    let module_cid = Cid::new_v1(0x55, hash); // Codec 0x55 = raw binary (standard for WASM content)

    // 3. Create test execution context
    // TestContext needs to be mutable to allow get_dag_store_mut to work as expected internally
    // when called by the executor. However, the executor takes Arc<TestContext>.
    // The `ContextExtension::get_dag_store_mut` takes `&mut self` on the *trait*,
    // but wasmtime::Store stores `Arc<T>`, so actual mutation of T through Arc::get_mut
    // happens inside the executor if the Arc is uniquely held at that point.
    // For our direct call to ctx.get_dag_store_mut() later, we pass a mutable reference to `ctx_instance`.
    let mut ctx_instance = TestContext::new();
    let ctx_arc_for_executor = Arc::new(ctx_instance.clone()); // Clone for the executor

    // 4. Initialize the executor
    let executor = ModernWasmExecutor::new()?;

    // 5. Execute
    println!("Executing WASM module: {}", module_cid);
    let execution_result = executor
        .execute(
            &wasm_bytes,
            ctx_arc_for_executor, // Pass the Arc'd context
            module_cid,
            None,           // No EventId for this direct execution test yet
            None,           // No specific input data for log_test.wasm
            Some(10_000_000),   // Fuel limit (increased slightly from original example)
        )
        .await?;
    
    println!("Execution result: {:?}", execution_result);

    // 6. Assert the log was captured
    let logs = ctx_instance.logs.read().unwrap();
    println!("Captured logs: {:?}", logs);
    assert!(
        logs.iter().any(|msg| msg.contains("hello from wasm")),
        "Log message from WASM not found"
    );

    // 7. Check that receipt is anchored in the context's DAG store
    // We use ctx_instance here to access the dag_store mutably if needed,
    // or just read from it.
    let dag_store_ref = &mut ctx_instance.dag_store; // Get a mutable ref to the owned store
    
    let all_events = dag_store_ref.get_all_events().await?;
    let receipts: Vec<_> = all_events
        .into_iter()
        .filter(|event_with_meta| matches!(event_with_meta.event.payload, EventPayload::ExecutionReceipt(_)))
        .collect();

    assert_eq!(receipts.len(), 1, "Expected exactly one execution receipt to be anchored");
    
    if let EventPayload::ExecutionReceipt(receipt_payload_cid) = &receipts[0].event.payload {
        println!("ExecutionReceipt found in DAG with payload CID: {}", receipt_payload_cid);
        // Further checks can be done on the receipt content if we fetch it by its CID
        // For now, presence is the main check.
    } else {
        panic!("Found event was not an ExecutionReceipt as expected.");
    }
    
    println!("Runtime WASM execution and receipt anchoring test completed successfully!");

    Ok(())
}

// TODO: Consider adding helper functions for:
// - Loading/compiling WASM
// - Setting up TestDagStore (if not using a shared utility)
// - Simplified proposal/vote creation if needed 