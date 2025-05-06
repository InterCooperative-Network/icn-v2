use icn_runtime::engine::WasmExecutor;
use icn_runtime::abi::context::HostContext;
use icn_types::Did;
use std::sync::{Arc, Mutex};
use async_trait::async_trait; // Import async_trait
use ed25519_dalek::VerifyingKey; // Use VerifyingKey

// Mock VerifyingKey for creating a Did
const MOCK_VERIFYING_KEY_BYTES: [u8; 32] = [0; 32];

// Use Mutex to track log messages for assertion
struct MockContext {
    logs: Mutex<Vec<String>>,
}

impl MockContext {
    fn new() -> Self {
        MockContext { logs: Mutex::new(Vec::new()) }
    }
}

#[async_trait]
impl HostContext for MockContext {
    fn get_caller_did(&self) -> Did {
        // Create a valid-looking mock Did
        let vk = VerifyingKey::from_bytes(&MOCK_VERIFYING_KEY_BYTES).unwrap();
        Did::new(&vk)
    }

    fn log_message(&self, message: &str) {
        println!("🔧 host_log called: {}", message);
        self.logs.lock().unwrap().push(message.to_string());
    }

    async fn verify_signature(&self, _did: &Did, _msg: &[u8], _sig: &[u8]) -> bool {
        // For testing, always return true or implement mock logic if needed
        true
    }
}

#[test]
fn test_host_log_invocation() {
    // Define the WAT module inline
    let wat_module = r#"
        (module
          ;; Import the host function we want to test
          (import "icn" "host_log" (func $host_log (param i32 i32)))
          
          ;; Define memory and export it so the host can access it
          (memory (export "memory") 1) ;; 1 page = 64KiB
          
          ;; Place the string "Hello ICN!" in memory at address 0
          (data (i32.const 0) "Hello ICN!")
          
          ;; Export a function `_start` that calls the host function
          (func (export "_start")
            ;; Push the memory offset of the string (0)
            i32.const 0
            ;; Push the length of the string (10)
            i32.const 10
            ;; Call the imported host_log function
            call $host_log
          )
        )
    "#;

    // Parse the WAT text into WASM bytes
    let wasm_bytes = wat::parse_str(wat_module).expect("Failed to parse WAT module");

    // Create the mock context
    let mock_ctx = Arc::new(MockContext::new());

    // Create the executor
    let executor = WasmExecutor::new().expect("Failed to create WasmExecutor");

    // Run the module with the context
    executor.run_module(&wasm_bytes, Arc::clone(&mock_ctx))
        .expect("Failed to run WASM module");

    // Assert that the log_message method was called with the correct message
    let logs = mock_ctx.logs.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "Hello ICN!");
} 