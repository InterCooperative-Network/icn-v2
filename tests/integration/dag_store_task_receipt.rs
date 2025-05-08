#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::Utc;
    use ed25519_dalek::Signature;
    use icn_core_types::Did;
    use icn_identity_core::{
        did::DidKey,
        vc::execution_receipt::{ExecutionReceipt, ExecutionScope, ExecutionStatus, ExecutionSubject, Proof},
    };
    use icn_types::dag::{
        DagNodeBuilder, DagPayload, DagStore, SharedDagStore, SignedDagNode,
        memory::MemoryDagStore,
    };
    use icn_wallet::receipt_store::{ReceiptFilter, StoredReceipt, WalletReceiptStore};
    use serde_json::json;
    
    /// Creates a DidKey for testing
    fn create_test_did_key() -> DidKey {
        // Note: In a real test, we'd use a known seed or mnemonic
        // For example: DidKey::from_seed([1u8; 32]) or similar
        // This example uses the simple new() constructor which generates a random key
        DidKey::new()
    }
    
    /// Creates a task DAG node
    async fn create_task_node(
        federation_did: &Did,
        requestor_did_key: &DidKey,
        task_id: &str,
        wasm_hash: &str,
        federation_id: &str,
    ) -> Result<(SignedDagNode, String)> {
        // Create task payload
        let task_payload = json!({
            "type": "Task",
            "task_id": task_id,
            "wasm_hash": wasm_hash,
            "inputs": [],
            "max_latency_ms": 1000,
            "memory_mb": 128,
            "cores": 1,
            "priority": 5,
            "timestamp": Utc::now().to_rfc3339(),
            "federation_id": federation_id
        });
        
        // Create DAG node for task
        let task_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(task_payload))
            .with_author(requestor_did_key.did().clone())
            .with_federation_id(federation_id.to_string())
            .with_label("Task".to_string())
            .build()?;
        
        // Serialize for signing
        let node_bytes = serde_json::to_vec(&task_node)?;
        
        // Sign the node
        let signature = requestor_did_key.sign(&node_bytes);
        
        // Create signed node
        let signed_task_node = SignedDagNode {
            node: task_node,
            signature,
            cid: None,
        };
        
        Ok((signed_task_node, task_id.to_string()))
    }
    
    /// Creates a receipt for task execution
    async fn create_execution_receipt(
        federation_did_key: &DidKey,
        requestor_did_key: &DidKey,
        task_id: &str,
        module_cid: &str,
        result_cid: &str,
        status: ExecutionStatus,
        federation_id: &str,
    ) -> Result<ExecutionReceipt> {
        // Create unique credential ID
        let credential_id = format!("urn:icn:receipt:{}", uuid::Uuid::new_v4());
        let now = Utc::now();
        
        // Create the subject
        let subject = ExecutionSubject {
            id: requestor_did_key.did().to_string(),
            scope: ExecutionScope::MeshCompute {
                task_id: task_id.to_string(),
                job_id: format!("job-{}", uuid::Uuid::new_v4()),
            },
            submitter: Some(requestor_did_key.did().to_string()),
            module_cid: module_cid.to_string(),
            result_cid: result_cid.to_string(),
            event_id: None,
            timestamp: now.timestamp() as u64,
            status,
            additional_properties: Some(json!({
                "result_summary": "Successful execution",
                "metrics": {
                    "execution_time_ms": 350,
                    "memory_used_mb": 64
                },
                "federation_id": federation_id
            })),
        };
        
        // Create the unsigned receipt
        let mut receipt = ExecutionReceipt {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://icn.network/2023/credentials/execution/v1".to_string(),
            ],
            id: credential_id.clone(),
            types: vec![
                "VerifiableCredential".to_string(),
                "ExecutionReceipt".to_string(),
            ],
            issuer: federation_did_key.did().to_string(),
            issuance_date: now,
            credential_subject: subject,
            proof: None,
        };
        
        // Sign the receipt
        let receipt_json = serde_json::to_vec(&receipt)?;
        let signature = federation_did_key.sign(&receipt_json);
        
        // Add the proof
        receipt.proof = Some(Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: now,
            proof_purpose: "assertionMethod".to_string(),
            verification_method: format!("{}#keys-1", federation_did_key.did()),
            proof_value: hex::encode(signature.to_bytes()),
        });
        
        Ok(receipt)
    }
    
    /// Stores a receipt in the DAG
    async fn store_receipt_in_dag(
        dag_store: &SharedDagStore,
        federation_did_key: &DidKey,
        receipt: &ExecutionReceipt,
        federation_id: &str,
    ) -> Result<String> {
        // Create payload with the receipt
        let receipt_payload = json!({
            "type": "ExecutionReceipt",
            "receipt": receipt,
        });
        
        // Create DAG node for receipt
        let receipt_node = DagNodeBuilder::new()
            .with_payload(DagPayload::Json(receipt_payload))
            .with_author(federation_did_key.did().clone())
            .with_federation_id(federation_id.to_string())
            .with_label("ExecutionReceipt".to_string())
            .build()?;
            
        // Serialize for signing
        let node_bytes = serde_json::to_vec(&receipt_node)?;
        
        // Sign the node
        let signature = federation_did_key.sign(&node_bytes);
        
        // Create signed node
        let signed_receipt_node = SignedDagNode {
            node: receipt_node,
            signature,
            cid: None,
        };
        
        // Add to DAG
        let cid = dag_store.add_node(signed_receipt_node).await?;
        
        Ok(cid.to_string())
    }
    
    /// Verifies a receipt's signature
    fn verify_receipt_signature(receipt: &ExecutionReceipt, federation_did_key: &DidKey) -> Result<bool> {
        if let Some(proof) = &receipt.proof {
            // Get the signature bytes
            let signature_bytes = hex::decode(&proof.proof_value)?;
            let signature = Signature::try_from(signature_bytes.as_slice())?;
            
            // Create a copy without the proof for verification
            let mut receipt_for_verification = receipt.clone();
            receipt_for_verification.proof = None;
            
            // Serialize for verification
            let receipt_bytes = serde_json::to_vec(&receipt_for_verification)?;
            
            // Verify signature
            let verification_result = federation_did_key.verify(&receipt_bytes, &signature);
            
            Ok(verification_result.is_ok())
        } else {
            // No proof to verify
            Ok(false)
        }
    }
    
    #[tokio::test]
    async fn test_task_receipt_flow() -> Result<()> {
        // Create DIDs for federation and requestor
        let federation_did_key = create_test_did_key();
        let requestor_did_key = create_test_did_key();
        
        // Federation ID
        let federation_id = "test-federation";
        
        // Create a shared DAG store
        let memory_store = MemoryDagStore::new();
        let dag_store = SharedDagStore::new(
            Box::new(memory_store) as Box<dyn DagStore + Send + Sync>
        );
        
        // Step 1: Create and store a task
        let task_id = uuid::Uuid::new_v4().to_string();
        let wasm_hash = "abcdef1234567890";
        let (task_node, task_id) = create_task_node(
            federation_did_key.did(),
            &requestor_did_key,
            &task_id,
            wasm_hash,
            federation_id,
        ).await?;
        
        // Add task to DAG
        let task_cid = dag_store.add_node(task_node).await?;
        println!("Task stored with CID: {}", task_cid);
        
        // Step 2: Create execution receipt
        let module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let result_cid = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjm";
        
        let receipt = create_execution_receipt(
            &federation_did_key,
            &requestor_did_key,
            &task_id,
            module_cid,
            result_cid,
            ExecutionStatus::Success,
            federation_id,
        ).await?;
        
        // Step 3: Store the receipt in the DAG
        let receipt_cid = store_receipt_in_dag(
            &dag_store,
            &federation_did_key,
            &receipt,
            federation_id,
        ).await?;
        println!("Receipt stored with CID: {}", receipt_cid);
        
        // Step 4: Verify the receipt
        let is_valid = verify_receipt_signature(&receipt, &federation_did_key)?;
        assert!(is_valid, "Receipt signature verification failed");
        
        // Step 5: Store receipt in wallet
        let stored_receipt = StoredReceipt {
            id: receipt.id.clone(),
            cid: receipt_cid.parse()?,
            federation_did: federation_did_key.did().clone(),
            subject: receipt.credential_subject.clone(),
            execution_timestamp: receipt.credential_subject.timestamp,
            raw_vc: receipt.clone(),
            source_event_id: None,
            wallet_stored_at: receipt.credential_subject.timestamp,
        };
        
        // Create wallet store (in-memory for testing)
        let mut wallet_store = icn_wallet::receipt_store::InMemoryWalletReceiptStore::new();
        wallet_store.save_receipt(stored_receipt)?;
        
        // Step 6: Query the wallet store for receipts
        let filter = ReceiptFilter {
            submitter_did: Some(requestor_did_key.did().clone()),
            ..Default::default()
        };
        
        let wallet_receipts = wallet_store.list_receipts(filter)?;
        assert_eq!(wallet_receipts.len(), 1, "Expected 1 receipt in wallet");
        assert_eq!(wallet_receipts[0].subject.module_cid, module_cid);
        
        // Verify that all steps in the flow worked as expected
        Ok(())
    }

    #[tokio::test]
    async fn test_multi_actor_dag_and_receipt_flow() -> Result<()> {
        // Create DIDs for two federations and two requestors
        let federation1_did_key = create_test_did_key();
        let federation2_did_key = create_test_did_key();
        let requestor1_did_key = create_test_did_key();
        let requestor2_did_key = create_test_did_key();

        let federation1_id = "federation-1";
        let federation2_id = "federation-2";

        // Shared DAG store for all actors
        let memory_store = MemoryDagStore::new();
        let dag_store = SharedDagStore::new(
            Box::new(memory_store) as Box<dyn DagStore + Send + Sync>
        );

        // Step 1: Each requestor submits a task to the DAG (anchored by their federation)
        let task1_id = uuid::Uuid::new_v4().to_string();
        let task2_id = uuid::Uuid::new_v4().to_string();
        let wasm_hash1 = "wasmhash1";
        let wasm_hash2 = "wasmhash2";

        let (task1_node, task1_id) = create_task_node(
            federation1_did_key.did(),
            &requestor1_did_key,
            &task1_id,
            wasm_hash1,
            federation1_id,
        ).await?;
        let (task2_node, task2_id) = create_task_node(
            federation2_did_key.did(),
            &requestor2_did_key,
            &task2_id,
            wasm_hash2,
            federation2_id,
        ).await?;

        // Add both tasks to DAG (simulate concurrent writes)
        let dag_store1 = dag_store.clone();
        let dag_store2 = dag_store.clone();
        let task1_fut = tokio::spawn(async move {
            dag_store1.add_node(task1_node).await
        });
        let task2_fut = tokio::spawn(async move {
            dag_store2.add_node(task2_node).await
        });
        let task1_cid = task1_fut.await??;
        let task2_cid = task2_fut.await??;

        // Step 2: Each federation issues a receipt for their respective requestor's task
        let module_cid1 = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let result_cid1 = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjm";
        let module_cid2 = "bafybeibwzif2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5";
        let result_cid2 = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjn";

        let receipt1 = create_execution_receipt(
            &federation1_did_key,
            &requestor1_did_key,
            &task1_id,
            module_cid1,
            result_cid1,
            ExecutionStatus::Success,
            federation1_id,
        ).await?;
        let receipt2 = create_execution_receipt(
            &federation2_did_key,
            &requestor2_did_key,
            &task2_id,
            module_cid2,
            result_cid2,
            ExecutionStatus::Success,
            federation2_id,
        ).await?;

        // Step 3: Store both receipts in the DAG (simulate concurrent writes)
        let dag_store1 = dag_store.clone();
        let dag_store2 = dag_store.clone();
        let federation1_did_key_clone = federation1_did_key.clone();
        let federation2_did_key_clone = federation2_did_key.clone();
        let receipt1_clone = receipt1.clone();
        let receipt2_clone = receipt2.clone();
        let store1 = tokio::spawn(async move {
            store_receipt_in_dag(&dag_store1, &federation1_did_key_clone, &receipt1_clone, federation1_id).await
        });
        let store2 = tokio::spawn(async move {
            store_receipt_in_dag(&dag_store2, &federation2_did_key_clone, &receipt2_clone, federation2_id).await
        });
        let receipt1_cid = store1.await??;
        let receipt2_cid = store2.await??;

        // Step 4: Each wallet stores and verifies its own receipt, and attempts to verify the other wallet's receipt
        let mut wallet1 = icn_wallet::receipt_store::InMemoryWalletReceiptStore::new();
        let mut wallet2 = icn_wallet::receipt_store::InMemoryWalletReceiptStore::new();

        let stored_receipt1 = StoredReceipt {
            id: receipt1.id.clone(),
            cid: receipt1_cid.parse()?,
            federation_did: federation1_did_key.did().clone(),
            subject: receipt1.credential_subject.clone(),
            execution_timestamp: receipt1.credential_subject.timestamp,
            raw_vc: receipt1.clone(),
            source_event_id: None,
            wallet_stored_at: receipt1.credential_subject.timestamp,
        };
        let stored_receipt2 = StoredReceipt {
            id: receipt2.id.clone(),
            cid: receipt2_cid.parse()?,
            federation_did: federation2_did_key.did().clone(),
            subject: receipt2.credential_subject.clone(),
            execution_timestamp: receipt2.credential_subject.timestamp,
            raw_vc: receipt2.clone(),
            source_event_id: None,
            wallet_stored_at: receipt2.credential_subject.timestamp,
        };
        wallet1.save_receipt(stored_receipt1.clone())?;
        wallet2.save_receipt(stored_receipt2.clone())?;

        // Each wallet verifies its own receipt
        let is_valid1 = verify_receipt_signature(&receipt1, &federation1_did_key)?;
        let is_valid2 = verify_receipt_signature(&receipt2, &federation2_did_key)?;
        assert!(is_valid1, "Wallet1's receipt signature verification failed");
        assert!(is_valid2, "Wallet2's receipt signature verification failed");

        // Each wallet attempts to verify the other wallet's receipt
        let is_valid1_other = verify_receipt_signature(&receipt2, &federation2_did_key)?;
        let is_valid2_other = verify_receipt_signature(&receipt1, &federation1_did_key)?;
        assert!(is_valid1_other, "Wallet1 failed to verify Wallet2's receipt");
        assert!(is_valid2_other, "Wallet2 failed to verify Wallet1's receipt");

        // Both receipts should be present in the DAG (by CID)
        let dag_store_read = dag_store.clone();
        let node1 = dag_store_read.get_node(&stored_receipt1.cid).await?;
        let node2 = dag_store_read.get_node(&stored_receipt2.cid).await?;
        if let DagPayload::Json(ref json1) = node1.node.payload {
            assert_eq!(json1["type"], "ExecutionReceipt");
        } else {
            panic!("Node1 payload is not JSON");
        }
        if let DagPayload::Json(ref json2) = node2.node.payload {
            assert_eq!(json2["type"], "ExecutionReceipt");
        } else {
            panic!("Node2 payload is not JSON");
        }

        // Both wallets can query their own receipts
        let filter1 = ReceiptFilter {
            submitter_did: Some(requestor1_did_key.did().clone()),
            ..Default::default()
        };
        let filter2 = ReceiptFilter {
            submitter_did: Some(requestor2_did_key.did().clone()),
            ..Default::default()
        };
        let wallet1_receipts = wallet1.list_receipts(filter1)?;
        let wallet2_receipts = wallet2.list_receipts(filter2)?;
        assert_eq!(wallet1_receipts.len(), 1);
        assert_eq!(wallet2_receipts.len(), 1);
        assert_eq!(wallet1_receipts[0].id, receipt1.id);
        assert_eq!(wallet2_receipts[0].id, receipt2.id);

        // Both wallets can attempt to query the other wallet's receipt (should be empty)
        let wallet1_other = wallet1.list_receipts(ReceiptFilter {
            submitter_did: Some(requestor2_did_key.did().clone()),
            ..Default::default()
        })?;
        let wallet2_other = wallet2.list_receipts(ReceiptFilter {
            submitter_did: Some(requestor1_did_key.did().clone()),
            ..Default::default()
        })?;
        assert!(wallet1_other.is_empty());
        assert!(wallet2_other.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_receipt_with_tampered_signature_fails_verification() -> Result<()> {
        let federation_did_key = create_test_did_key();
        let requestor_did_key = create_test_did_key();
        let federation_id = "test-federation";
        let task_id = uuid::Uuid::new_v4().to_string();
        let module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let result_cid = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjm";
        let receipt = create_execution_receipt(
            &federation_did_key,
            &requestor_did_key,
            &task_id,
            module_cid,
            result_cid,
            ExecutionStatus::Success,
            federation_id,
        ).await?;
        // Tamper with the proof value
        let mut tampered_receipt = receipt.clone();
        if let Some(proof) = &mut tampered_receipt.proof {
            // Flip a bit in the proof value
            let mut bytes = hex::decode(&proof.proof_value)?;
            if !bytes.is_empty() {
                bytes[0] ^= 0xFF;
            }
            proof.proof_value = hex::encode(bytes);
        }
        let is_valid = verify_receipt_signature(&tampered_receipt, &federation_did_key)?;
        assert!(!is_valid, "Tampered receipt should fail signature verification");
        Ok(())
    }

    #[tokio::test]
    async fn test_query_nonexistent_dag_node_returns_error() -> Result<()> {
        let memory_store = MemoryDagStore::new();
        let dag_store = SharedDagStore::new(
            Box::new(memory_store) as Box<dyn DagStore + Send + Sync>
        );
        // Use a random CID that is not present
        let random_cid: icn_types::Cid = "bafybeibwzif2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5w2k7w3v5".parse()?;
        let result = dag_store.get_node(&random_cid).await;
        assert!(result.is_err(), "Querying a non-existent DAG node should return an error");
        Ok(())
    }

    #[tokio::test]
    async fn test_overwrite_existing_receipt_in_wallet() -> Result<()> {
        let federation_did_key = create_test_did_key();
        let requestor_did_key = create_test_did_key();
        let federation_id = "test-federation";
        let task_id = uuid::Uuid::new_v4().to_string();
        let module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let result_cid = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjm";
        let receipt = create_execution_receipt(
            &federation_did_key,
            &requestor_did_key,
            &task_id,
            module_cid,
            result_cid,
            ExecutionStatus::Success,
            federation_id,
        ).await?;
        let mut wallet = icn_wallet::receipt_store::InMemoryWalletReceiptStore::new();
        let stored_receipt = StoredReceipt {
            id: receipt.id.clone(),
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".parse()?,
            federation_did: federation_did_key.did().clone(),
            subject: receipt.credential_subject.clone(),
            execution_timestamp: receipt.credential_subject.timestamp,
            raw_vc: receipt.clone(),
            source_event_id: None,
            wallet_stored_at: receipt.credential_subject.timestamp,
        };
        wallet.save_receipt(stored_receipt.clone())?;
        // Attempt to overwrite with a different execution_timestamp
        let mut overwritten_receipt = stored_receipt.clone();
        overwritten_receipt.execution_timestamp += 1;
        let result = wallet.save_receipt(overwritten_receipt.clone());
        // Document observed behavior: InMemoryWalletReceiptStore allows overwrite (HashMap::insert)
        assert!(result.is_ok(), "Overwriting an existing receipt should succeed (current policy: replace)");
        let fetched = wallet.get_receipt_by_id(&overwritten_receipt.id)?;
        assert_eq!(fetched.unwrap().execution_timestamp, overwritten_receipt.execution_timestamp);
        Ok(())
    }

    #[tokio::test]
    async fn test_receipt_with_mismatched_federation_fails_verification() -> Result<()> {
        let federation_did_key = create_test_did_key();
        let wrong_federation_did_key = create_test_did_key();
        let requestor_did_key = create_test_did_key();
        let federation_id = "test-federation";
        let task_id = uuid::Uuid::new_v4().to_string();
        let module_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let result_cid = "bafybeihykxkacdsxnplp7exwspy2h7jxgomgepstqgciufpnxkaexitpjm";
        // Create a receipt signed by the wrong federation
        let receipt = create_execution_receipt(
            &wrong_federation_did_key,
            &requestor_did_key,
            &task_id,
            module_cid,
            result_cid,
            ExecutionStatus::Success,
            federation_id,
        ).await?;
        // Attempt to verify with the correct federation's key (should fail)
        let is_valid = verify_receipt_signature(&receipt, &federation_did_key)?;
        assert!(!is_valid, "Receipt signed by wrong federation should fail verification");
        Ok(())
    }
} 