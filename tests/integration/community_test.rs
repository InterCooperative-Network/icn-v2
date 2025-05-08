use anyhow::Result;
use icn_types::{Did, Cid, dag::{DagNodeBuilder, DagPayload, NodeScope}};
use icn_identity_core::did::DidKey;
use icn_types::dag::memory::MemoryDagStore;
use std::sync::Arc;
use tokio;
use rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use chrono::Utc;

/// Setup a test federation with a community
async fn setup_test_federation() -> Result<(
    String, 
    Arc<MemoryDagStore>,
    DidKey
)> {
    // Create a federation ID
    let federation_id = "test-federation".to_string();
    
    // Create a shared DAG store
    let dag_store = Arc::new(MemoryDagStore::new());
    
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
    
    Ok((federation_id, dag_store, did_key))
}

/// Create a community with its own DID
async fn setup_test_community(
    community_id: &str,
    federation_id: &str,
    dag_store: Arc<MemoryDagStore>,
) -> Result<(Did, DidKey)> {
    // Create a community key
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let did_key = DidKey::from_signing_key(&signing_key);
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a community genesis node
    let payload = DagPayload::Json(serde_json::json!({
        "type": "CommunityGenesis",
        "name": community_id,
        "description": format!("Test Community {}", community_id),
        "createdAt": Utc::now().to_rfc3339(),
        "founder": did.to_string(),
    }));
    
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did.clone())
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Community)
        .with_scope_id(community_id.to_string())
        .with_label("CommunityGenesis".to_string())
        .build()?;
    
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let _cid = dag_store.add_node(signed_node).await?;
    
    Ok((did, did_key))
}

/// Create a community charter
async fn create_community_charter(
    community_id: &str,
    federation_id: &str,
    dag_store: Arc<MemoryDagStore>,
    did_key: &DidKey,
) -> Result<Cid> {
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a charter
    let payload = DagPayload::Json(serde_json::json!({
        "type": "CommunityCharter",
        "title": "Test Charter",
        "content": "This is a test charter for the community.",
        "createdAt": Utc::now().to_rfc3339(),
        "author": did.to_string(),
        "status": "Active",
    }));
    
    // Get community genesis node as parent
    let community_nodes = dag_store.get_nodes_by_payload_type("CommunityGenesis").await?;
    let mut parent_cid = None;
    
    for node in community_nodes {
        if let Some(scope) = node.node.metadata.scope_id.as_ref() {
            if scope == community_id {
                let cid = node.ensure_cid()?;
                parent_cid = Some(cid);
                break;
            }
        }
    }
    
    let mut builder = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did.clone())
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Community)
        .with_scope_id(community_id.to_string())
        .with_label("CommunityCharter".to_string());
    
    // Add parent if available
    if let Some(cid) = parent_cid {
        builder = builder.with_parent(cid);
    }
    
    let node = builder.build()?;
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let cid = dag_store.add_node(signed_node).await?;
    
    Ok(cid)
}

/// Create a community proposal
async fn create_community_proposal(
    community_id: &str,
    federation_id: &str,
    title: &str,
    content: &str,
    dag_store: Arc<MemoryDagStore>,
    did_key: &DidKey,
) -> Result<Cid> {
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a proposal
    let payload = DagPayload::Json(serde_json::json!({
        "type": "CommunityProposal",
        "title": title,
        "content": content,
        "proposedAt": Utc::now().to_rfc3339(),
        "proposer": did.to_string(),
        "status": "Open",
    }));
    
    // Get the latest nodes from this community to use as parents
    let community_nodes = dag_store.get_nodes_by_payload_type("CommunityCharter").await?;
    let mut parent_cids = Vec::new();
    
    for node in community_nodes {
        if let Some(scope) = node.node.metadata.scope_id.as_ref() {
            if scope == community_id {
                let cid = node.ensure_cid()?;
                parent_cids.push(cid);
            }
        }
    }
    
    let mut builder = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did)
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Community)
        .with_scope_id(community_id.to_string())
        .with_label("CommunityProposal".to_string());
    
    // Add parents if available
    if !parent_cids.is_empty() {
        builder = builder.with_parents(parent_cids);
    }
    
    let node = builder.build()?;
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let cid = dag_store.add_node(signed_node).await?;
    
    Ok(cid)
}

/// Vote on a community proposal
async fn vote_on_proposal(
    community_id: &str,
    federation_id: &str,
    proposal_cid: &Cid,
    vote: &str,
    comment: Option<&str>,
    dag_store: Arc<MemoryDagStore>,
    did_key: &DidKey,
) -> Result<Cid> {
    let did = Did::from_string(&did_key.to_did_string())?;
    
    // Create a vote
    let payload = DagPayload::Json(serde_json::json!({
        "type": "CommunityVote",
        "vote": vote,
        "comment": comment,
        "votedAt": Utc::now().to_rfc3339(),
        "voter": did.to_string(),
        "proposalCid": proposal_cid.to_string(),
    }));
    
    let node = DagNodeBuilder::new()
        .with_payload(payload)
        .with_author(did)
        .with_federation_id(federation_id.to_string())
        .with_scope(NodeScope::Community)
        .with_scope_id(community_id.to_string())
        .with_label("CommunityVote".to_string())
        .with_parent(proposal_cid.clone())
        .build()?;
    
    let signature = did_key.sign(&node.canonical_bytes()?);
    let signed_node = node.sign(signature.to_bytes().to_vec())?;
    
    let cid = dag_store.add_node(signed_node).await?;
    
    Ok(cid)
}

#[tokio::test]
async fn test_community_governance() -> Result<()> {
    // Set up a federation with a DAG store
    let (federation_id, dag_store, federation_key) = setup_test_federation().await?;
    
    // Set up a community
    let community_id = "test-community";
    let (community_did, community_key) = setup_test_community(
        community_id,
        &federation_id,
        dag_store.clone(),
    ).await?;
    
    // Create a community charter
    let charter_cid = create_community_charter(
        community_id,
        &federation_id,
        dag_store.clone(),
        &community_key,
    ).await?;
    
    // Create a community proposal
    let proposal_title = "Community Garden Initiative";
    let proposal_content = "Let's create a community garden in the vacant lot on Main Street.";
    let proposal_cid = create_community_proposal(
        community_id,
        &federation_id,
        proposal_title,
        proposal_content,
        dag_store.clone(),
        &community_key,
    ).await?;
    
    // Create member keys for voting
    let mut csprng = OsRng;
    let member1_signing_key = SigningKey::generate(&mut csprng);
    let member1_key = DidKey::from_signing_key(&member1_signing_key);
    
    let member2_signing_key = SigningKey::generate(&mut csprng);
    let member2_key = DidKey::from_signing_key(&member2_signing_key);
    
    // Vote on the proposal
    let vote1_cid = vote_on_proposal(
        community_id,
        &federation_id,
        &proposal_cid,
        "yes",
        Some("Great idea!"),
        dag_store.clone(),
        &member1_key,
    ).await?;
    
    let vote2_cid = vote_on_proposal(
        community_id,
        &federation_id,
        &proposal_cid,
        "yes",
        Some("I fully support this."),
        dag_store.clone(),
        &member2_key,
    ).await?;
    
    // Verify that all nodes are in the DAG
    let all_nodes = dag_store.get_ordered_nodes().await?;
    let mut community_nodes = Vec::new();
    
    for node in all_nodes {
        if let Some(scope_id) = &node.node.metadata.scope_id {
            if scope_id == community_id && node.node.metadata.scope == NodeScope::Community {
                community_nodes.push(node);
            }
        }
    }
    
    // Verify that we have the expected number of nodes
    // 1 genesis + 1 charter + 1 proposal + 2 votes = 5
    assert_eq!(community_nodes.len(), 5);
    
    // Verify that we can find the charter
    let charter_nodes = dag_store.get_nodes_by_payload_type("CommunityCharter").await?;
    assert_eq!(charter_nodes.len(), 1);
    
    // Verify that we can find the proposal
    let proposal_nodes = dag_store.get_nodes_by_payload_type("CommunityProposal").await?;
    assert_eq!(proposal_nodes.len(), 1);
    
    // Verify that we can find the votes
    let vote_nodes = dag_store.get_nodes_by_payload_type("CommunityVote").await?;
    assert_eq!(vote_nodes.len(), 2);
    
    Ok(())
} 