# ICN Wallet Verification SDK

A self-sovereign verification toolkit for ICN Mesh dispatch credentials. This SDK provides tools to verify that compute tasks were authorized through proper cryptographic governance.

## Features

- ✅ **Cryptographic verification** of dispatch credential signatures
- ✅ **Trust policy validation** to ensure dispatchers are authorized
- ✅ **Revocation checking** for credentials, issuers, and subjects
- ✅ **Policy lineage verification** to trace trust to its roots
- ✅ **Mobile-ready** with Kotlin/Swift bindings via UniFFI

## Usage (Rust)

```rust
use icn_wallet::{verify_dispatch_credential, VerificationReport, TrustPolicyStore};
use icn_types::dag::memory::MemoryDagStore;

// Load credential JSON
let vc_json = r#"{
  "@context": ["https://www.w3.org/2018/credentials/v1"],
  "id": "urn:icn:dispatch:123",
  "type": ["VerifiableCredential", "DispatchReceipt"],
  "issuer": "did:icn:scheduler123",
  "issuanceDate": "2023-07-15T16:34:21Z",
  "credentialSubject": {
    "id": "did:icn:requestor456",
    "selectedNode": "did:icn:worker789",
    "taskRequest": {
      "wasm_hash": "0xabcdef123456",
      "federation_id": "my-federation"
    }
  },
  "proof": {
    "type": "Ed25519Signature2020",
    "verificationMethod": "did:icn:scheduler123#keys-1",
    "created": "2023-07-15T16:34:21Z", 
    "proofValue": "abc123signature"
  }
}"#;

// Create/load DAG store
let dag_store = MemoryDagStore::new();

// Create/load trust policy
let policy_store = TrustPolicyStore {
    federation_id: "my-federation".to_string(),
    trusted_dids: vec![
        // List of trusted DIDs with their trust levels
    ],
    policy_cid: Some("QmTrustPolicyHash".to_string()),
    previous_policy_cid: None,
};

// Verify the credential
let report = verify_dispatch_credential(vc_json, &dag_store, &policy_store)?;

// Check the results
if report.overall_valid {
    println!("Credential is valid!");
    println!("Issued by: {}", report.issuer_did);
} else {
    println!("Invalid credential: {}", report.error.unwrap_or_default());
}
```

## CLI Usage

The crate includes a simple CLI for verification:

```bash
cargo run --example verify_cli -- verify-dispatch ./path/to/credential.json ./dag_dir ./policy.json
```

## Mobile Integration

The crate exposes a UniFFI API for mobile integration:

```kotlin
// Kotlin example
val report = IcnWallet.verifyCredential(credentialJson)
```

```swift
// Swift example
let report = IcnWallet.verifyCredential(credentialJson)
```

## Build with UniFFI Bindings

```bash
cargo build --features uniffi-bindings
```

## License

MIT 