use icn_runtime::engine::ModernWasmExecutor;
use icn_runtime::abi::context::HostContext;
use icn_runtime::policy::{PolicyLoader, MembershipIndex};
use icn_types::Did;
use icn_types::Cid;
use std::sync::{Arc, Mutex};
use async_trait::async_trait; // Import async_trait
use ed25519_dalek::VerifyingKey; // Use VerifyingKey
use tokio; // Import tokio for async test
use icn_runtime::config::ExecutionConfig; // Import ExecutionConfig
use icn_runtime::engine::ContextExtension; // Import ContextExtension
use icn_types::dag::DagStore; // Import DagStore
use icn_identity_core::did::DidKey; // Import DidKey
use once_cell::sync::Lazy; // Import Lazy

// Mock VerifyingKey for creating a Did
const MOCK_VERIFYING_KEY_BYTES: [u8; 32] = [0; 32];

static DEFAULT_EXEC_CONFIG: Lazy<ExecutionConfig> = Lazy::new(ExecutionConfig::default);

// Use Mutex to track log messages for assertion
#[derive(Clone)]
struct MockContext {
    logged: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    // Add fields for other HostContext methods if needed for tests, e.g., for error handling
    error_message: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    // Mock DID for get_caller_did
    caller_did: Did,
}

impl MockContext {
    fn new() -> Self {
        MockContext {
            logged: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            error_message: std::sync::Arc::new(std::sync::Mutex::new(None)),
            caller_did: Did::from_string("did:example:mockcaller").unwrap(), // Example DID
        }
    }
}

#[async_trait::async_trait]
impl HostContext for MockContext {
    fn read_string(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32, _len: i32) -> anyhow::Result<String> {
        unimplemented!("MockContext::read_string")
    }

    fn write_string(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32, _max_len: i32, _s: &str) -> anyhow::Result<i32> {
        unimplemented!("MockContext::write_string")
    }

    fn malloc(&self, _caller: &mut impl wasmtime::AsContextMut, _size: i32) -> anyhow::Result<i32> {
        unimplemented!("MockContext::malloc")
    }

    fn free(&self, _caller: &mut impl wasmtime::AsContextMut, _ptr: i32) -> anyhow::Result<()> {
        unimplemented!("MockContext::free")
    }

    fn get_caller_did(&self) -> Did {
        self.caller_did.clone()
    }

    fn log_message(&self, message: &str) { // Changed return type to ()
        self.logged.lock().unwrap().push(message.to_string());
        // Ok(()) removed
    }

    async fn verify_signature(&self, _did: &Did, _message: &[u8], _signature: &[u8]) -> bool {
        unimplemented!("MockContext::verify_signature")
    }

    fn set_error(&self, message: String) {
        *self.error_message.lock().unwrap() = Some(message);
    }

    fn get_error(&self) -> Option<String> {
        self.error_message.lock().unwrap().clone()
    }

    fn clear_error(&self) {
        *self.error_message.lock().unwrap() = None;
    }

    fn policy_loader(&self) -> Option<Arc<dyn PolicyLoader + Send + Sync>> {
        // Return a mock policy loader if needed for tests, otherwise None
        None
    }

    fn membership_index(&self) -> Option<Arc<dyn MembershipIndex + Send + Sync>> {
        // Return a mock membership index if needed for tests, otherwise None
        None
    }
}

impl ContextExtension for MockContext {
    fn get_execution_config(&self) -> &ExecutionConfig {
        &DEFAULT_EXEC_CONFIG
    }

    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn DagStore + Send + Sync)> {
        // If MockContext needs to provide a DagStore for some tests, implement here
        None
    }

    // node_did, federation_did have default impls (Option<&Did> { None })
    // If specific tests need them, they can be overridden.

    fn caller_did(&self) -> Option<&Did> { // Matches ContextExtension signature
        Some(&self.caller_did) 
    }

    // federation_keypair has default impl (Option<DidKey> { None })

    // membership_index and policy_loader have default impls (Option<Arc<...>> { None })
    // These are also in HostContext, HostContext impl already returns None for these.
    // Default impls from ContextExtension trait will be used if not overridden here.
}

#[tokio::test]
async fn test_host_log_invocation() {
    // Define the WAT module inline
    let wat_module = r#"
        (module
          (import "env" "log" (func $log (param i32 i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "Hello ICN!")
          (func (export "_start")
            (call $log (i32.const 0) (i32.const 10)) ;; "Hello ICN!" is 10 bytes
          )
        )
    "#;
    let wasm_bytes = wat::parse_str(wat_module).expect("Failed to parse WAT");

    let executor = ModernWasmExecutor::new().expect("Failed to create executor");
    let mock_ctx = std::sync::Arc::new(MockContext::new());

    let dummy_module_cid = Cid::from_bytes(b"test_module_cid_0123456789abcdef").unwrap(); // Dummy CID

    // Run the module with the context
    executor.execute(&wasm_bytes, mock_ctx.clone(), dummy_module_cid, None, None, None).await
        .expect("Failed to run WASM module");

    // Assert that the log_message method was called with the correct message
    let logs = mock_ctx.logged.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "Hello ICN!");
} 