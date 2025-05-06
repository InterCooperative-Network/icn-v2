// Integration test for the complete governance flow from proposal to execution receipt
//
// This test demonstrates the end-to-end process of:
// 1. Setting up a federation with participants
// 2. Creating and submitting a proposal to the DAG
// 3. Submitting votes from participants
// 4. Executing the proposal if quorum is reached
// 5. Verifying the execution receipt

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
use icn_types::dag::{
    DagEvent,
    EventType,
    EventPayload,
    EventId,
    DagStore,
    DagError,
    memory::MemoryDagStore
};
use icn_types::Cid;

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use async_trait::async_trait;

/// Simple wrapper around the in-memory DAG store with extensions for governance testing
struct TestDagStore {
    store: Arc<RwLock<MemoryDagStore>>,
    // Storage for proposal and vote content (simulating IPLD)
    content_by_cid: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl TestDagStore {
    fn new() -> Self {
        TestDagStore {
            store: Arc::new(RwLock::new(MemoryDagStore::new())),
            content_by_cid: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Store content by CID
    async fn store_content(&self, cid: String, content: String) -> Result<(), DagError> {
        let mut content_store = self.content_by_cid.write().await;
        content_store.insert(cid, content);
        Ok(())
    }
    
    /// Get content by CID
    async fn get_content(&self, cid: &str) -> Result<Option<String>, DagError> {
        let content_store = self.content_by_cid.read().await;
        Ok(content_store.get(cid).cloned())
    }
    
    /// Add a proposal to the DAG
    async fn add_proposal(&self, proposal: &ProposalCredential) -> Result<EventId, DagError> {
        // 1. Serialize the proposal
        let proposal_json = proposal.to_json()
            .map_err(|e| DagError::InvalidNodeData(format!("Failed to serialize proposal: {}", e)))?;
        
        // 2. Generate a CID for the proposal content (simplified)
        let content_cid = format!("proposal-cid-{}", Uuid::new_v4());
        
        // 3. Store the content
        self.store_content(content_cid.clone(), proposal_json).await?;
        
        // 4. Create a new DAG event for the proposal
        let event = DagEvent::new(
            EventType::Proposal,
            proposal.credential_subject.submitter.clone(),
            Vec::new(), // No parents for simplicity
            EventPayload::proposal(proposal.id.clone(), content_cid),
        );
        
        // 5. Add the event to the DAG
        let dag_store = self.store.write().await;
        let node = dag_store.create_node(event, 0)?;
        
        Ok(node.id().clone())
    }
    
    /// Add a vote to the DAG
    async fn add_vote(&self, vote: &VoteCredential) -> Result<EventId, DagError> {
        // 1. Serialize the vote
        let vote_json = vote.to_json()
            .map_err(|e| DagError::InvalidNodeData(format!("Failed to serialize vote: {}", e)))?;
        
        // 2. Generate a CID for the vote content
        let content_cid = format!("vote-cid-{}", Uuid::new_v4());
        
        // 3. Store the content
        self.store_content(content_cid.clone(), vote_json).await?;
        
        // 4. Create a DAG event for the vote
        let event = DagEvent::new(
            EventType::Vote,
            vote.credential_subject.id.clone(),
            Vec::new(), // No parents for simplicity
            EventPayload::vote(
                vote.credential_subject.proposal_id.clone(),
                format!("{:?}", vote.credential_subject.decision)
            ),
        );
        
        // Add metadata to store the content CID
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("content_cid".to_string(), content_cid);
        
        // 5. Add the event to the DAG
        let dag_store = self.store.write().await;
        let node = dag_store.create_node_with_metadata(event, 0, metadata)?;
        
        Ok(node.id().clone())
    }
    
    /// Find all proposal events
    async fn find_proposal_events(&self) -> Result<Vec<(DagEvent, std::collections::HashMap<String, String>)>, DagError> {
        let dag_store = self.store.read().await;
        
        // In a real implementation, this would efficiently query the DAG
        // Here we'll just scan all nodes
        let mut result = Vec::new();
        
        for node in dag_store.get_nodes().await? {
            if let EventType::Proposal = node.event().event_type {
                result.push((node.event().clone(), node.metadata().clone()));
            }
        }
        
        Ok(result)
    }
    
    /// Find vote events for a specific proposal
    async fn find_vote_events_for_proposal(&self, proposal_id: &str) -> Result<Vec<(DagEvent, std::collections::HashMap<String, String>)>, DagError> {
        let dag_store = self.store.read().await;
        
        let mut result = Vec::new();
        
        for node in dag_store.get_nodes().await? {
            if let EventType::Vote = node.event().event_type {
                if let EventPayload::Vote { proposal_id: event_proposal_id, .. } = &node.event().payload {
                    if event_proposal_id == proposal_id {
                        result.push((node.event().clone(), node.metadata().clone()));
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    /// Add an execution receipt to the DAG
    async fn add_receipt(&self, receipt: &ExecutionReceipt) -> Result<EventId, DagError> {
        // 1. Serialize the receipt
        let receipt_json = receipt.to_json()
            .map_err(|e| DagError::InvalidNodeData(format!("Failed to serialize receipt: {}", e)))?;
        
        // 2. Generate a CID for the receipt
        let content_cid = format!("receipt-cid-{}", Uuid::new_v4());
        
        // 3. Store the content
        self.store_content(content_cid.clone(), receipt_json).await?;
        
        // 4. Create a DAG event for the receipt
        let event = DagEvent::new(
            EventType::Receipt,
            receipt.issuer.clone(),
            Vec::new(), // No parents for simplicity
            EventPayload::receipt(Cid::from(content_cid.clone())),
        );
        
        // 5. Add the event to the DAG
        let dag_store = self.store.write().await;
        let node = dag_store.create_node(event, 0)?;
        
        Ok(node.id().clone())
    }
}

/// Helper functions for the test
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Create a test proposal
fn create_test_proposal(
    federation_key: &DidKey,
    title: &str,
    description: &str,
    proposal_type: ProposalType,
    threshold: VotingThreshold,
    execution_cid: Option<String>,
) -> ProposalCredential {
    let federation_did = federation_key.did().to_string();
    
    let now = now();
    
    let subject = ProposalSubject {
        id: federation_did.clone(),
        title: title.to_string(),
        description: description.to_string(),
        proposal_type,
        status: ProposalStatus::Active,
        submitter: federation_did.clone(),
        voting_threshold: threshold,
        voting_duration: VotingDuration::TimeBased(3600), // 1 hour voting window
        voting_start_time: now,
        voting_end_time: Some(now + 3600),
        execution_cid,
        thread_cid: None,
        parameters: None,
        previous_version: None,
        event_id: None,
        created_at: now,
        updated_at: now,
    };
    
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    ProposalCredential::new(
        cred_id,
        federation_did,
        subject
    ).sign(federation_key).expect("Should sign credential")
}

/// Create a test vote
fn create_test_vote(
    voter_key: &DidKey,
    federation_did: &str,
    proposal_id: &str,
    decision: VoteDecision,
) -> VoteCredential {
    let voter_did = voter_key.did().to_string();
    
    let subject = VoteSubject {
        id: voter_did.clone(),
        federation_id: federation_did.to_string(),
        proposal_id: proposal_id.to_string(),
        decision,
        voting_power: None,
        justification: None,
        delegate_for: None,
        is_amendment: false,
        previous_vote_id: None,
        cast_at: now(),
    };
    
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    VoteCredential::new(
        cred_id,
        federation_did.to_string(),
        subject
    ).sign(voter_key).expect("Should sign vote")
}

/// Execute a proposal (stub for WASM execution)
fn execute_proposal_stub(execution_cid: &str) -> String {
    // In a real implementation, this would load and execute the WASM module
    format!("result-{}", Uuid::new_v4())
}

/// Create an execution receipt
fn create_execution_receipt(
    executor_key: &DidKey,
    federation_did: &str,
    module_cid: &str,
    result_cid: &str,
) -> ExecutionReceipt {
    let executor_did = executor_key.did().to_string();
    
    let subject = ExecutionSubject {
        id: executor_did.clone(),
        scope: ExecutionScope::Federation { federation_id: federation_did.to_string() },
        submitter: Some(executor_did.clone()),
        module_cid: module_cid.to_string(),
        result_cid: result_cid.to_string(),
        event_id: None,
        timestamp: now(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };
    
    let receipt_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    ExecutionReceipt::new(
        receipt_id,
        federation_did.to_string(),
        subject
    ).sign(executor_key).unwrap()
}

/// Load a proposal from the DAG
async fn load_proposal_from_dag(
    proposal_id: &str,
    dag_store: &TestDagStore,
) -> Result<Option<ProposalCredential>, DagError> {
    // 1. Find all proposal events
    let proposal_events = dag_store.find_proposal_events().await?;
    
    // 2. Find the event with matching proposal ID
    for (event, _metadata) in proposal_events {
        if let EventPayload::Proposal { proposal_id: event_proposal_id, content_cid } = &event.payload {
            if event_proposal_id == proposal_id {
                // 3. Get the proposal content
                if let Some(proposal_json) = dag_store.get_content(&content_cid).await? {
                    // 4. Deserialize the proposal
                    let proposal = ProposalCredential::from_json(&proposal_json)
                        .map_err(|e| DagError::InvalidNodeData(format!("Failed to deserialize proposal: {}", e)))?;
                    
                    return Ok(Some(proposal));
                }
            }
        }
    }
    
    Ok(None)
}

/// Load votes for a proposal from the DAG
async fn load_votes_for_proposal(
    proposal_id: &str,
    dag_store: &TestDagStore,
) -> Result<Vec<VoteCredential>, DagError> {
    // 1. Find all vote events for the proposal
    let vote_events = dag_store.find_vote_events_for_proposal(proposal_id).await?;
    
    let mut votes = Vec::new();
    
    // 2. Process each vote event
    for (event, metadata) in vote_events {
        // 3. Get the content CID from metadata
        if let Some(content_cid) = metadata.get("content_cid") {
            // 4. Get the vote content
            if let Some(vote_json) = dag_store.get_content(content_cid).await? {
                // 5. Deserialize the vote
                let vote = VoteCredential::from_json(&vote_json)
                    .map_err(|e| DagError::InvalidNodeData(format!("Failed to deserialize vote: {}", e)))?;
                
                votes.push(vote);
            }
        }
    }
    
    Ok(votes)
}

/// Main test for the end-to-end governance flow
#[tokio::test]
async fn test_governance_proposal_flow() {
    // 1. Setup federation and participants
    println!("🔑 Setting up federation and participants...");
    let federation_key = DidKey::new();
    let voter1_key = DidKey::new();
    let voter2_key = DidKey::new();
    let voter3_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    println!("   Federation DID: {}", federation_did);
    
    // Create a test DAG store
    let dag_store = TestDagStore::new();
    
    // 2. Create a proposal with execution code
    println!("\n📝 Creating a new proposal...");
    let execution_cid = Some("bafyreihgmyh2srmmyiw7fdihrc2lw2bqdyxagrpvt2zk3aitq4hdxrhzzz".to_string());
    
    let proposal = create_test_proposal(
        &federation_key,
        "Test Governance Proposal",
        "This is a test proposal for demonstrating the complete governance flow",
        ProposalType::CodeExecution,
        VotingThreshold::Percentage(67), // Require 67% approval
        execution_cid.clone(),
    );
    
    println!("   Proposal ID: {}", proposal.id);
    
    // 3. Submit the proposal to the DAG
    println!("\n🔗 Anchoring proposal to DAG...");
    let proposal_event_id = dag_store.add_proposal(&proposal).await.expect("Failed to add proposal to DAG");
    println!("   Proposal event ID: {}", proposal_event_id);
    
    // 4. Submit votes from participants
    println!("\n🗳️ Participants casting votes...");
    
    // Voter 1 votes YES
    let vote1 = create_test_vote(&voter1_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote1_event_id = dag_store.add_vote(&vote1).await.expect("Failed to add vote to DAG");
    println!("   Voter 1 ({}) voted YES - Event ID: {}", voter1_key.did().to_string(), vote1_event_id);
    
    // Voter 2 votes YES
    let vote2 = create_test_vote(&voter2_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote2_event_id = dag_store.add_vote(&vote2).await.expect("Failed to add vote to DAG");
    println!("   Voter 2 ({}) voted YES - Event ID: {}", voter2_key.did().to_string(), vote2_event_id);
    
    // Voter 3 votes NO
    let vote3 = create_test_vote(&voter3_key, &federation_did, &proposal.id, VoteDecision::No);
    let vote3_event_id = dag_store.add_vote(&vote3).await.expect("Failed to add vote to DAG");
    println!("   Voter 3 ({}) voted NO - Event ID: {}", voter3_key.did().to_string(), vote3_event_id);
    
    // 5. Load the proposal and votes from the DAG
    println!("\n📊 Loading proposal and votes from DAG...");
    let loaded_proposal = load_proposal_from_dag(&proposal.id, &dag_store).await
        .expect("Failed to search for proposal")
        .expect("Proposal not found in DAG");
    
    let loaded_votes = load_votes_for_proposal(&proposal.id, &dag_store).await
        .expect("Failed to search for votes");
    
    println!("   Found proposal: {}", loaded_proposal.id);
    println!("   Found {} votes", loaded_votes.len());
    
    // 6. Evaluate votes using the QuorumEngine
    println!("\n🧮 Evaluating votes with QuorumEngine...");
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&loaded_proposal, &loaded_votes).expect("Failed to evaluate votes");
    
    println!("   Vote tally:");
    println!("   - Yes votes: {} (power: {})", tally.yes_votes, tally.yes_power);
    println!("   - No votes: {} (power: {})", tally.no_votes, tally.no_power);
    println!("   - Abstain votes: {} (power: {})", tally.abstain_votes, tally.abstain_power);
    println!("   - Veto votes: {} (power: {})", tally.veto_votes, tally.veto_power);
    println!("   - Threshold: {}", tally.threshold);
    println!("   - Outcome: {:?}", tally.outcome);
    
    // 7. Execute the proposal if quorum is reached
    assert_eq!(tally.outcome, QuorumOutcome::Passed, "Proposal should pass with 2/3 yes votes");
    
    println!("\n⚙️ Executing proposal...");
    let execution_cid = execution_cid.unwrap();
    let result_cid = execute_proposal_stub(&execution_cid);
    println!("   Execution complete. Result CID: {}", result_cid);
    
    // 8. Create and anchor execution receipt
    println!("\n📜 Generating execution receipt...");
    let receipt = create_execution_receipt(
        &federation_key,
        &federation_did,
        &execution_cid,
        &result_cid,
    );
    
    println!("   Receipt ID: {}", receipt.id);
    println!("   Anchoring receipt to DAG...");
    
    let receipt_event_id = dag_store.add_receipt(&receipt).await.expect("Failed to add receipt to DAG");
    println!("   Receipt event ID: {}", receipt_event_id);
    
    // 9. Verify the receipt
    println!("\n✅ Verifying execution receipt...");
    let valid = receipt.verify().expect("Failed to verify receipt");
    assert!(valid, "Receipt verification should succeed");
    println!("   Receipt verification: {}", if valid { "VALID ✓" } else { "INVALID ✗" });
    
    // 10. Final assertions for the complete flow
    println!("\n🎉 Governance flow test completed successfully!");
    
    // Additional test assertions
    assert_eq!(loaded_votes.len(), 3, "Should have loaded 3 votes from DAG");
    assert_eq!(tally.yes_votes, 2, "Should have 2 yes votes");
    assert_eq!(tally.no_votes, 1, "Should have 1 no vote");
    assert_eq!(receipt.credential_subject.module_cid, execution_cid, "Receipt should reference correct module CID");
    assert_eq!(receipt.credential_subject.result_cid, result_cid, "Receipt should reference correct result CID");
    assert_eq!(receipt.credential_subject.status, ExecutionStatus::Success, "Execution status should be Success");
}

/// Extension trait for MemoryDagStore to make it easier to work with in tests
#[async_trait]
trait MemoryDagStoreExt {
    async fn get_nodes(&self) -> Result<Vec<DagNodeWithMetadata>, DagError>;
    fn create_node(&self, event: DagEvent, height: u64) -> Result<DagNodeWithMetadata, DagError>;
    fn create_node_with_metadata(&self, event: DagEvent, height: u64, metadata: std::collections::HashMap<String, String>) -> Result<DagNodeWithMetadata, DagError>;
}

/// Helper struct that includes metadata with the DAG node
struct DagNodeWithMetadata {
    event: DagEvent,
    id: EventId,
    metadata: std::collections::HashMap<String, String>,
}

impl DagNodeWithMetadata {
    fn event(&self) -> &DagEvent {
        &self.event
    }
    
    fn id(&self) -> &EventId {
        &self.id
    }
    
    fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }
}

#[async_trait]
impl MemoryDagStoreExt for MemoryDagStore {
    async fn get_nodes(&self) -> Result<Vec<DagNodeWithMetadata>, DagError> {
        // This would be implemented to get all nodes from the MemoryDagStore
        // For the test stub, we'll just return an empty vector
        Ok(Vec::new())
    }
    
    fn create_node(&self, event: DagEvent, height: u64) -> Result<DagNodeWithMetadata, DagError> {
        // Create a node with default metadata
        self.create_node_with_metadata(event, height, std::collections::HashMap::new())
    }
    
    fn create_node_with_metadata(&self, event: DagEvent, height: u64, metadata: std::collections::HashMap<String, String>) -> Result<DagNodeWithMetadata, DagError> {
        // In a real implementation, this would add the node to the store
        // For the test stub, we'll just return a node with generated ID
        let id = EventId::new(format!("event-{}", Uuid::new_v4()).as_bytes());
        
        Ok(DagNodeWithMetadata {
            event,
            id,
            metadata,
        })
    }
} 