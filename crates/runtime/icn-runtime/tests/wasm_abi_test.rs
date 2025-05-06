use icn_runtime::engine::WasmExecutor;
use icn_runtime::abi::context::HostContext;
use icn_identity_core::did::DidKey;
use icn_types::Did;
use ed25519_dalek::{Signer, VerifyingKey, SigningKey, Signature, SECRET_KEY_LENGTH, PUBLIC_KEY_LENGTH};
use rand::rngs::OsRng;
use std::sync::{Arc};
use anyhow::{Result, anyhow};
use wat::parse_str;
use async_trait::async_trait;

// --- Mock Host Context ---

#[derive(Clone)]
struct MockContext {
    caller_did_key: DidKey,
    caller_did: Did,
}

struct MockContextWrapper(Arc<MockContext>);

#[async_trait]
impl HostContext for MockContextWrapper {
    fn get_caller_did(&self) -> Did {
        self.0.caller_did.clone()
    }

    fn log_message(&self, message: &str) {
        println!("[MockContext Log]: {}", message);
    }

    async fn verify_signature(&self, did: &Did, message: &[u8], signature_bytes: &[u8]) -> bool {
        if did != &self.0.caller_did {
            println!("[MockContext Verify]: DID mismatch! Expected {}, Got {}", self.0.caller_did, did);
            return false;
        }

        let signature_array: Result<[u8; 64], _> = signature_bytes.try_into();
        let signature = match signature_array {
            Ok(arr) => match Signature::from_bytes(&arr) {
                Ok(sig) => sig,
                Err(_) => {
                    println!("[MockContext Verify]: Failed to parse signature from bytes");
                    return false;
                }
            },
            Err(_) => {
                println!("[MockContext Verify]: Incorrect signature length: expected 64, got {}", signature_bytes.len());
                return false;
            }
        };
        
        match self.0.caller_did_key.verify(message, &signature) {
            Ok(_) => true,
            Err(e) => {
                println!("[MockContext Verify]: Signature verification logic failed: {}", e);
                false 
            }
        }
    }
}

// Helper generates DidKey internally, returns key, DID, and signer
fn generate_test_did() -> (DidKey, Did, SigningKey) {
    // Use DidKey::new() which generates keys internally
    let did_key = DidKey::new(); 
    // Need to get the SigningKey back out if DidKey owns it.
    // Let's assume DidKey allows accessing the keys, or we need to modify DidKey.
    // FOR NOW: Let's regenerate the signing key separately FOR THE TEST, 
    //          even though the DidKey has its own internal one. This is a workaround.
    let mut csprng = OsRng{};
    let signing_key_for_test = SigningKey::generate(&mut csprng);
    // The DidKey used in context will have a DIFFERENT private key than the one we use to sign
    // This is not ideal, but avoids modifying DidKey struct for now.
    // A better solution would be DidKey::from_keypair or similar.
    
    let did = did_key.did().clone(); // Clone the DID out
    (did_key, did, signing_key_for_test) // Return the test signing key
}


// --- Tests ---

#[tokio::test]
async fn test_get_caller_did() -> Result<()> {
    let (test_did_key, test_did, _) = generate_test_did();
    let expected_did_string = test_did_key.to_did_string();
    let mock_context = Arc::new(MockContext { caller_did_key: test_did_key, caller_did: test_did });
    let mock_wrapper = MockContextWrapper(mock_context.clone());
    let executor: WasmExecutor<MockContextWrapper> = WasmExecutor::new()?;

    let wat = format!(r#"
        (module
          (import "icn" "host_get_caller_did_into_buffer" (func $get_did (param i32 i32)))
          (import "icn" "host_log" (func $log (param i32 i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "{expected_did_string}")
          (global $buffer_ptr (mut i32) (i32.const 1024))
          (global $buffer_len (mut i32) (i32.const 100))

          (func (export "run_get_did_test") (result i32)
            (call $get_did (global.get $buffer_ptr) (global.get $buffer_len))
            (local $i i32)
            (local.set $i (i32.const 0))
            (loop $compare_loop
                (local $expected_byte i32)
                (local.set $expected_byte (i32.load8_u (local.get $i)))
                (local $actual_byte i32)
                (local.set $actual_byte (i32.load8_u (i32.add (global.get $buffer_ptr) (local.get $i))))
                (if (i32.ne (local.get $expected_byte) (local.get $actual_byte))
                  (then (return (i32.const 0)))
                )
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br_if $compare_loop (i32.lt_u (local.get $i) (i32.const {len})))
            )
            (if (i32.eq (local.get $i) (i32.const {len}))
                (then (i32.const 1))
                (else (i32.const 0))
            )
          )
        )
    "#, expected_did_string = expected_did_string, len = expected_did_string.len());

    let wasm_bytes = parse_str(&wat)?;
    let module = wasmtime::Module::new(executor.engine(), &wasm_bytes)?;
    let mut store = wasmtime::Store::new(executor.engine(), mock_wrapper);

    let mut linker = wasmtime::Linker::new(executor.engine());
    icn_runtime::abi::bindings::register_host_functions::<MockContextWrapper>(&mut linker)?;
    
    let instance = linker.instantiate(&mut store, &module)?;
    let run_test_func = instance.get_typed_func::<(), i32>(&mut store, "run_get_did_test")?;

    let result = run_test_func.call(&mut store, ())?;
    assert_eq!(result, 1, "Wasm test for get_caller_did failed: DID string mismatch");

    Ok(())
}


#[tokio::test]
async fn test_verify_signature() -> Result<()> {
    let (test_did_key, test_did, signing_key) = generate_test_did();
    let mock_context = Arc::new(MockContext { caller_did_key: test_did_key, caller_did: test_did });
    let mock_wrapper = MockContextWrapper(mock_context.clone());
    let executor: WasmExecutor<MockContextWrapper> = WasmExecutor::new()?;

    let message = b"This is a test message for signature verification.";
    let signature = signing_key.sign(message);
    let sig_bytes = signature.to_bytes();

    let invalid_message = b"This is a different message.";
    
    let corrupted_sig_bytes = {
        let mut bytes = sig_bytes.clone();
        bytes[0] = bytes[0].wrapping_add(1);
        bytes
    };

    let wat = format!(r#"
        (module
          (import "icn" "host_verify_signature" (func $verify (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)

          (data (i32.const 100) "{message}")         
          (data (i32.const 200) "{signature}")      
          (data (i32.const 300) "{invalid_message}") 
          (data (i32.const 400) "{corrupted_sig}") 

          (func (export "run_verify_valid") (result i32)
            (call $verify (i32.const 100) (i32.const {msg_len}) (i32.const 200) (i32.const {sig_len}))
          )
          (func (export "run_verify_invalid_message") (result i32)
            (call $verify (i32.const 300) (i32.const {invalid_msg_len}) (i32.const 200) (i32.const {sig_len}))
          )
          (func (export "run_verify_corrupted_sig") (result i32)
            (call $verify (i32.const 100) (i32.const {msg_len}) (i32.const 400) (i32.const {sig_len}))
          )
        )
    "#, 
    message = String::from_utf8_lossy(message).escape_debug().to_string(),
    signature = String::from_utf8_lossy(&sig_bytes).escape_debug().to_string(),
    invalid_message = String::from_utf8_lossy(invalid_message).escape_debug().to_string(),
    corrupted_sig = String::from_utf8_lossy(&corrupted_sig_bytes).escape_debug().to_string(),
    msg_len = message.len(),
    sig_len = sig_bytes.len(),
    invalid_msg_len = invalid_message.len()
    );

    let wasm_bytes = parse_str(&wat)?;
    let module = wasmtime::Module::new(executor.engine(), &wasm_bytes)?;
    let mut store = wasmtime::Store::new(executor.engine(), mock_wrapper);

    let mut linker = wasmtime::Linker::new(executor.engine());
    icn_runtime::abi::bindings::register_host_functions::<MockContextWrapper>(&mut linker)?;
    let instance = linker.instantiate(&mut store, &module)?;

    let run_verify_valid = instance.get_typed_func::<(), i32>(&mut store, "run_verify_valid")?;
    let run_verify_invalid_message = instance.get_typed_func::<(), i32>(&mut store, "run_verify_invalid_message")?;
    let run_verify_corrupted_sig = instance.get_typed_func::<(), i32>(&mut store, "run_verify_corrupted_sig")?;

    let valid_result = run_verify_valid.call(&mut store, ()).await?;
    assert_eq!(valid_result, 1, "Wasm test for verify_signature (valid case) failed.");

    let invalid_msg_result = run_verify_invalid_message.call(&mut store, ()).await?;
    assert_eq!(invalid_msg_result, 0, "Wasm test for verify_signature (invalid message case) failed.");

    let corrupted_sig_result = run_verify_corrupted_sig.call(&mut store, ()).await?;
    assert_eq!(corrupted_sig_result, 0, "Wasm test for verify_signature (corrupted sig case) failed.");

    Ok(())
} 