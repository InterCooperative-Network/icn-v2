use icn_identity_core::did::DidKey;
use icn_identity_core::vc::execution_receipt::{
    ExecutionReceipt, ExecutionSubject, ExecutionScope, ExecutionStatus
};
use icn_types::dag::EventId;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_execution_receipt_creation() {
    // Create a test subject
    let subject = ExecutionSubject {
        id: "did:key:z6MkpzW2izkFjNz3Vwxgv7TVf4gTbKLRyJtCYSouUpWQReEz".to_string(),
        scope: ExecutionScope::Federation {
            federation_id: "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U".to_string(),
        },
        submitter: Some("did:key:z6MkkqzKJKQZ7e1MX5U3EZ5WUrZ8vi8yXhEAQJMxi9YD5wLA".to_string()),
        module_cid: "bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string(),
        result_cid: "bafy2bzaced7zj7xw2umkpvgyzsj3zkg3qwmgdgpwtlsxfmawleq2tjnfo7fsg".to_string(),
        event_id: Some(EventId([0u8; 32])),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };

    // Create an ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U",
        subject,
    );

    // Check that the receipt has the expected values
    assert_eq!(receipt.id, "urn:uuid:123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(receipt.issuer, "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U");
    assert_eq!(receipt.types, vec!["VerifiableCredential", "ExecutionReceipt"]);
    assert_eq!(receipt.context, vec![
        "https://www.w3.org/2018/credentials/v1",
        "https://schema.intercooperative.network/2023/credentials/execution-receipt/v1"
    ]);
    assert!(receipt.proof.is_none());

    // Verify credential subject
    let subject = &receipt.credential_subject;
    match &subject.scope {
        ExecutionScope::Federation { federation_id } => {
            assert_eq!(federation_id, "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U");
        },
        _ => panic!("Expected Federation scope"),
    }
    assert_eq!(subject.status, ExecutionStatus::Success);
}

#[test]
fn test_execution_receipt_roundtrip() {
    // Generate a test key
    let did_key = DidKey::generate().unwrap();

    // Create a test subject
    let subject = ExecutionSubject {
        id: "did:key:z6MkpzW2izkFjNz3Vwxgv7TVf4gTbKLRyJtCYSouUpWQReEz".to_string(),
        scope: ExecutionScope::MeshCompute {
            task_id: "task-123".to_string(),
            job_id: "job-456".to_string(),
        },
        submitter: None,
        module_cid: "bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string(),
        result_cid: "bafy2bzaced7zj7xw2umkpvgyzsj3zkg3qwmgdgpwtlsxfmawleq2tjnfo7fsg".to_string(),
        event_id: None,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };

    // Create and sign an ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        did_key.did().to_string(),
        subject,
    ).sign(&did_key).unwrap();

    // Verify the proof is present
    assert!(receipt.proof.is_some());

    // Serialize to JSON
    let json = receipt.to_json().unwrap();
    println!("Receipt JSON: {}", json);

    // Deserialize from JSON
    let deserialized = ExecutionReceipt::from_json(&json).unwrap();

    // Verify the deserialized receipt
    assert_eq!(receipt.id, deserialized.id);
    assert_eq!(receipt.issuer, deserialized.issuer);
    assert_eq!(receipt.proof, deserialized.proof);

    // Verify the signature
    assert!(deserialized.verify().unwrap());
}

#[test]
fn test_execution_receipt_cooperative_scope() {
    // Create a test subject with Cooperative scope
    let subject = ExecutionSubject {
        id: "did:key:z6MkpzW2izkFjNz3Vwxgv7TVf4gTbKLRyJtCYSouUpWQReEz".to_string(),
        scope: ExecutionScope::Cooperative {
            coop_id: "did:key:z6MkrfCGsVy3RJWgHKRQRuna7MTLxkCDsQAcGXBsH6K7nD4Y".to_string(),
            module: "governance-vote".to_string(),
        },
        submitter: Some("did:key:z6MkkqzKJKQZ7e1MX5U3EZ5WUrZ8vi8yXhEAQJMxi9YD5wLA".to_string()),
        module_cid: "bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string(),
        result_cid: "bafy2bzaced7zj7xw2umkpvgyzsj3zkg3qwmgdgpwtlsxfmawleq2tjnfo7fsg".to_string(),
        event_id: None,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };

    // Create an ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        "did:key:z6MkrfCGsVy3RJWgHKRQRuna7MTLxkCDsQAcGXBsH6K7nD4Y",
        subject,
    );

    // Serialize to JSON
    let json = receipt.to_json().unwrap();
    println!("Cooperative Receipt JSON: {}", json);

    // Check that the scope is correctly serialized
    assert!(json.contains("Cooperative"));
    assert!(json.contains("governance-vote"));
}

#[test]
fn test_execution_receipt_with_custom_scope() {
    // Create a test subject with Custom scope
    let subject = ExecutionSubject {
        id: "did:key:z6MkpzW2izkFjNz3Vwxgv7TVf4gTbKLRyJtCYSouUpWQReEz".to_string(),
        scope: ExecutionScope::Custom {
            description: "Cross-federation verification".to_string(),
            metadata: serde_json::json!({
                "origin_federation": "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U",
                "target_federation": "did:key:z6MkrfCGsVy3RJWgHKRQRuna7MTLxkCDsQAcGXBsH6K7nD4Y"
            }),
        },
        submitter: None,
        module_cid: "bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string(),
        result_cid: "bafy2bzaced7zj7xw2umkpvgyzsj3zkg3qwmgdgpwtlsxfmawleq2tjnfo7fsg".to_string(),
        event_id: None,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: Some(serde_json::json!({
            "metrics": {
                "execution_time_ms": 1500,
                "memory_usage_mb": 256
            }
        })),
    };

    // Create an ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U",
        subject,
    );

    // Serialize to JSON
    let json = receipt.to_json().unwrap();
    println!("Custom Receipt JSON: {}", json);

    // Check that the custom properties are correctly serialized
    assert!(json.contains("Cross-federation verification"));
    assert!(json.contains("origin_federation"));
    assert!(json.contains("metrics"));
    assert!(json.contains("execution_time_ms"));
}

#[test]
fn test_invalid_signature_verification() {
    // Generate two different keys
    let did_key1 = DidKey::generate().unwrap();
    let did_key2 = DidKey::generate().unwrap();

    // Create a test subject
    let subject = ExecutionSubject {
        id: "did:key:z6MkpzW2izkFjNz3Vwxgv7TVf4gTbKLRyJtCYSouUpWQReEz".to_string(),
        scope: ExecutionScope::Federation {
            federation_id: "did:key:z6MkhaXbRKW4FhKfdpPTVVSRJDfR7UqQSRPPpRJRAxdwPB7U".to_string(),
        },
        submitter: None,
        module_cid: "bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string(),
        result_cid: "bafy2bzaced7zj7xw2umkpvgyzsj3zkg3qwmgdgpwtlsxfmawleq2tjnfo7fsg".to_string(),
        event_id: None,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };

    // Create and sign an ExecutionReceipt with key1
    let receipt = ExecutionReceipt::new(
        "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
        did_key1.did().to_string(),
        subject,
    ).sign(&did_key1).unwrap();

    // Tamper with the receipt by changing the issuer to key2's DID
    let mut tampered_receipt = receipt.clone();
    tampered_receipt.issuer = did_key2.did().to_string();

    // Verification should fail
    assert!(tampered_receipt.verify().is_err());
} 