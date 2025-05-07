use anyhow::Result;
use icn_types::{Did, Cid, dag::{DagNodeBuilder, DagPayload, NodeScope}};
use icn_identity_core::did::DidKey;
use icn_types::dag::memory::MemoryDagStore;
use icn_economics::storage::InMemoryTokenStore;
use planetary_mesh::scheduler::{Scheduler, CapabilityIndex, TaskRequest};
use icn_economics::token::ResourceType;
use std::sync::Arc;
use tokio;
use std::time::Duration;
use rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use chrono::Utc;

/// Setup a test federation with two cooperatives
async fn setup_test_federation() -> Result<(
    String, 
    Arc<InMemoryTokenStore>, 
    Arc<MemoryDagStore>,
    DidKey
)> {
    // Create a federation ID
    let federation_id = "test-federation".to_string();
    
    // Create a shared DAG store
    let dag_store = Arc::new(MemoryDagStore::new());
    
    // Create a token store
    let token_store = Arc::new(InMemoryTokenStore::new());
    
    // Create a federation key
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::from_signing_key(&signing_key);
    let fed_did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a federation genesis node
    let payload = DagPayload::Json(serde_json::json!({
        "type": "FederationGenesis",
        "name": federation_id,
        "description": "Test Federation",
        "createdAt": Utc::now().to_rfc3339(),
        "founder": fed_did.to_string(),
    }));
    
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(fed_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("FederationGenesis".to_string())
        .build()?;
    
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let _cid = dag_store.add_node(signed_node).await?;
    
    Ok((federation_id, token_store, dag_store, did_key))
}

/// Create a cooperative with its own DID
async fn setup_test_cooperative(
    coop_id: &str,
    federation_id: &str,
    dag_store: Arc<MemoryDagStore>,
    token_store: Arc<InMemoryTokenStore>,
    initial_tokens: u64,
) -> Result<(Did, DidKey)> {
    // Create a cooperative key
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::from_signing_key(&signing_key);
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a cooperative genesis node
    let payload = DagPayload::Json(serde_json::json!({
        "type": "CooperativeGenesis",
        "name": coop_id,
        "description": format!("Test Cooperative {}", coop_id),
        "createdAt": Utc::now().to_rfc3339(),
        "founder": did.to_string(),
    }));
    
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did.clone())
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Cooperative)
        .with_scope_id(coop_id.to_string())
        .with_label("CooperativeGenesis".to_string())
        .build()?;
    
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let _cid = dag_store.add_node(signed_node).await?;
    
    // Credit initial tokens to the cooperative
    token_store.credit(
        coop_id, 
        ResourceType::ComputeUnit, 
        initial_tokens
    ).await?;
    
    Ok((did, did_key))
}

/// Register a node with capabilities in a cooperative
async fn register_test_node(
    node_id: &str,
    coop_id: &str,
    federation_id: &str,
    dag_store: Arc<MemoryDagStore>,
    capabilities: Vec<(String, u64)>, // (capability_type, value)
    did_key: &DidKey,
) -> Result<Did> {
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create capability data
    let mut capability_data = Vec::new();
    for (cap_type, value) in capabilities {
        capability_data.push(serde_json::json!({
            "type": cap_type,
            "value": value,
            "available": true,
        }));
    }
    
    // Create a node manifest
    let payload = DagPayload::Json(serde_json::json!({
        "type": "NodeManifest",
        "node_id": node_id,
        "coop_id": coop_id,
        "capabilities": capability_data,
        "last_seen": Utc::now().to_rfc3339(),
        "metadata": {
            "coop_id": coop_id,
        }
    }));
    
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did.clone())
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Cooperative)
        .with_scope_id(coop_id.to_string())
        .with_label("NodeManifest".to_string())
        .build()?;
    
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let _cid = dag_store.add_node(signed_node).await?;
    
    Ok(did)
}

#[tokio::test]
async fn test_cross_coop_job_execution() -> Result<()> {
    // Set up a federation with a DAG store and token store
    let (federation_id, token_store, dag_store, federation_key) = setup_test_federation().await?;
    
    // Set up Cooperative A (job submitter) with 100 tokens
    let coop_a_id = "coop-a";
    let (coop_a_did, coop_a_key) = setup_test_cooperative(
        coop_a_id,
        &federation_id,
        dag_store.clone(),
        token_store.clone(),
        100
    ).await?;
    
    // Set up Cooperative B (job executor) with 0 tokens initially
    let coop_b_id = "coop-b";
    let (coop_b_did, coop_b_key) = setup_test_cooperative(
        coop_b_id,
        &federation_id,
        dag_store.clone(),
        token_store.clone(),
        0
    ).await?;
    
    // Register a node in Cooperative B with CPU and RAM capabilities
    let node_b_did = register_test_node(
        "node-b",
        coop_b_id,
        &federation_id,
        dag_store.clone(),
        vec![
            ("RamMb".to_string(), 2048),
            ("CpuCores".to_string(), 4),
        ],
        &coop_b_key
    ).await?;
    
    // Create capability index and scheduler
    let cap_index = Arc::new(CapabilityIndex::new(dag_store.clone()));
    let scheduler_did = Did::from_string("did:icn:scheduler")?;
    let mut scheduler = Scheduler::new(
        federation_id.clone(),
        cap_index.clone(),
        dag_store.clone(),
        scheduler_did.clone()
    );
    
    // Set the token store for the scheduler
    scheduler.set_token_store(token_store.clone());
    
    // Create a task request
    let task_request = TaskRequest {
        requestor: coop_a_did.clone(),
        wasm_hash: "test-hash".to_string(),
        wasm_size: 1024,
        inputs: vec!["test-input".to_string()],
        max_latency_ms: 1000,
        memory_mb: 1024,
        cores: 2,
        priority: 10,
        timestamp: Utc::now(),
        federation_id: federation_id.clone(),
    };
    
    // Dispatch the task across cooperatives
    let result = scheduler.dispatch_cross_coop(task_request, coop_a_id.to_string()).await?;
    
    // Verify the result
    assert_eq!(result.origin_coop, coop_a_id);
    assert_eq!(result.executor_coop, coop_b_id);
    assert!(result.transaction_cids.len() >= 3); // We should have at least 3 transaction CIDs
    
    // Check token balances
    let coop_a_balance = token_store.get_balance(coop_a_id, &ResourceType::ComputeUnit).await?;
    let coop_b_balance = token_store.get_balance(coop_b_id, &ResourceType::ComputeUnit).await?;
    
    // Cooperative A should have less tokens now
    assert!(coop_a_balance < 100);
    
    // Cooperative B should have some tokens now
    assert!(coop_b_balance > 0);
    
    // Verify DAG records
    let debit_nodes = dag_store.get_nodes_by_payload_type("ResourceDebit").await?;
    let credit_nodes = dag_store.get_nodes_by_payload_type("ResourceCredit").await?;
    let transfer_nodes = dag_store.get_nodes_by_payload_type("CrossCoopTransaction").await?;
    
    assert!(!debit_nodes.is_empty());
    assert!(!credit_nodes.is_empty());
    assert!(!transfer_nodes.is_empty());
    
    Ok(())
} 