use anyhow::Result;
use icn_types::{Did, Cid, dag::{DagNodeBuilder, DagPayload, NodeScope}};
use icn_identity_core::did::DidKey;
use icn_types::dag::memory::MemoryDagStore;
use icn_types::attestation::{QuorumProof, FederationMembershipAttestation, LineageAttestation, ScopeSignature};
use std::sync::Arc;
use tokio;
use chrono::Utc;
use rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use serde_json::json;

#[tokio::test]
async fn test_community_federation_join_flow() -> Result<()> {
    // Create test objects and DIDs
    let federation_id = "test-federation".to_string();
    let community_id = "test-community".to_string();
    let dag_store = Arc::new(MemoryDagStore::new());
    
    // Create DIDs for testing
    let mut csprng = OsRng;
    let federation_admin_key = SigningKey::generate(&mut csprng);
    let federation_admin_did_key = DidKey::from_signing_key(&federation_admin_key);
    let federation_admin_did = Did::from_string(&federation_admin_did_key.to_did_string())?;
    
    let community_admin_key = SigningKey::generate(&mut csprng);
    let community_admin_did_key = DidKey::from_signing_key(&community_admin_key);
    let community_admin_did = Did::from_string(&community_admin_did_key.to_did_string())?;
    
    // Create 5 federation members for voting
    let mut member_dids = Vec::new();
    let mut member_keys = Vec::new();
    
    for i in 0..5 {
        let key = SigningKey::generate(&mut csprng);
        let did_key = DidKey::from_signing_key(&key);
        let did = Did::from_string(&did_key.to_did_string())?;
        
        member_dids.push(did);
        member_keys.push(did_key);
    }
    
    // Step 1: Create a federation genesis node
    let fed_payload = DagPayload::Json(json!({
        "type": "FederationGenesis",
        "name": federation_id,
        "description": "Test Federation",
        "createdAt": Utc::now().to_rfc3339(),
        "founder": federation_admin_did.to_string(),
        "members": member_dids.iter().map(|d| d.to_string()).collect::<Vec<_>>(),
        "quorumThreshold": 3
    }));
    
    let fed_node = DagNodeBuilder::new()
        .with_payload(fed_payload)
        .with_author(federation_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("FederationGenesis".to_string())
        .build()?;
    
    let signature = federation_admin_did_key.sign(&fed_node.canonical_bytes()?);
    let signed_fed_node = fed_node.sign(signature.to_bytes().to_vec())?;
    let federation_genesis_cid = dag_store.add_node(signed_fed_node).await?;
    println!("Created federation with genesis CID: {}", federation_genesis_cid);
    
    // Step 2: Create a community genesis node
    let comm_payload = DagPayload::Json(json!({
        "type": "CommunityGenesis",
        "name": community_id,
        "federationId": federation_id,
        "description": "Test Community",
        "createdAt": Utc::now().to_rfc3339(),
        "founder": community_admin_did.to_string(),
    }));
    
    let comm_node = DagNodeBuilder::new()
        .with_payload(comm_payload)
        .with_author(community_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Community)
        .with_scope_id(community_id.clone())
        .with_label("CommunityGenesis".to_string())
        .build()?;
    
    let signature = community_admin_did_key.sign(&comm_node.canonical_bytes()?);
    let signed_comm_node = comm_node.sign(signature.to_bytes().to_vec())?;
    let community_genesis_cid = dag_store.add_node(signed_comm_node).await?;
    println!("Created community with genesis CID: {}", community_genesis_cid);
    
    // Step 3: Community submits join request to federation
    let join_request_payload = DagPayload::Json(json!({
        "type": "CommunityJoinRequest",
        "communityId": community_id,
        "federationId": federation_id,
        "communityGenesisCid": community_genesis_cid.to_string(),
        "federationGenesisCid": federation_genesis_cid.to_string(),
        "requestedAt": Utc::now().to_rfc3339(),
        "requester": community_admin_did.to_string(),
    }));
    
    let join_request_node = DagNodeBuilder::new()
        .with_payload(join_request_payload)
        .with_author(community_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("CommunityJoinRequest".to_string())
        .with_parent(federation_genesis_cid)
        .with_parent(community_genesis_cid)
        .build()?;
    
    let signature = community_admin_did_key.sign(&join_request_node.canonical_bytes()?);
    let signed_join_request = join_request_node.sign(signature.to_bytes().to_vec())?;
    let join_request_cid = dag_store.add_node(signed_join_request).await?;
    println!("Submitted join request with CID: {}", join_request_cid);
    
    // Step 4: Federation members vote on the join request
    let mut vote_cids = Vec::new();
    
    // Process votes from all 5 members (3 yes, 2 no)
    for i in 0..5 {
        let vote = i < 3; // First 3 vote yes, last 2 vote no
        let voter_did = &member_dids[i];
        let voter_key = &member_keys[i];
        
        let vote_payload = DagPayload::Json(json!({
            "type": "FederationJoinVote",
            "vote": if vote { "yes" } else { "no" },
            "reason": format!("Member {} vote", i),
            "votedAt": Utc::now().to_rfc3339(),
            "voter": voter_did.to_string(),
            "requestCid": join_request_cid.to_string(),
        }));
        
        let vote_node = DagNodeBuilder::new()
            .with_payload(vote_payload)
            .with_author(voter_did.clone())
            .with_federation_id(federation_id.clone())
            .with_scope(NodeScope::Federation)
            .with_label("FederationJoinVote".to_string())
            .with_parent(join_request_cid)
            .build()?;
        
        let signature = voter_key.sign(&vote_node.canonical_bytes()?);
        let signed_vote = vote_node.sign(signature.to_bytes().to_vec())?;
        let vote_cid = dag_store.add_node(signed_vote).await?;
        vote_cids.push(vote_cid);
        
        println!("Member {} voted {}: {}", i, if vote { "yes" } else { "no" }, vote_cid);
    }
    
    // Step 5: Create QuorumProof from votes
    let total_members = 5;
    let threshold = 3;
    
    let mut quorum_proof = QuorumProof::new(
        total_members,
        threshold,
        member_dids.clone(),
    );
    
    // Add votes to the quorum proof
    for i in 0..5 {
        let vote = i < 3; // First 3 vote yes, last 2 vote no
        quorum_proof.add_vote(member_dids[i].clone(), vote)?;
    }
    
    // Verify quorum is reached and approved
    assert!(quorum_proof.is_quorum_reached());
    assert!(quorum_proof.is_approved());
    assert_eq!(quorum_proof.yes_votes, 3);
    assert_eq!(quorum_proof.no_votes, 2);
    
    // Step 6: Create FederationMembershipAttestation
    let membership_attestation = FederationMembershipAttestation::new(
        NodeScope::Community,
        &community_id,
        community_genesis_cid,
        &federation_id,
        federation_genesis_cid,
        join_request_cid,
        vote_cids,
        quorum_proof,
        Some(format!("Approved join request for {} to federation {}", community_id, federation_id)),
    );
    
    // Step 7: Sign the attestation with federation admin key
    let federation_sig = ScopeSignature {
        signer: federation_admin_did.clone(),
        scope: NodeScope::Federation,
        scope_id: Some(federation_id.clone()),
        signature: federation_admin_did_key.sign(&membership_attestation.canonical_bytes()?).to_bytes().to_vec(),
        timestamp: Utc::now(),
    };
    
    let mut signed_attestation = membership_attestation;
    signed_attestation.add_signature(federation_sig);
    
    // Step 8: Sign the attestation with community admin key
    let community_sig = ScopeSignature {
        signer: community_admin_did.clone(),
        scope: NodeScope::Community,
        scope_id: Some(community_id.clone()),
        signature: community_admin_did_key.sign(&signed_attestation.canonical_bytes()?).to_bytes().to_vec(),
        timestamp: Utc::now(),
    };
    
    signed_attestation.add_signature(community_sig);
    
    // Verify the attestation is complete
    assert!(signed_attestation.is_complete());
    assert!(signed_attestation.verify()?);
    
    // Step 9: Create a DAG node for the attestation
    let attestation_json = serde_json::to_value(&signed_attestation)?;
    let attestation_payload = DagPayload::Json(attestation_json);
    
    let attestation_node = DagNodeBuilder::new()
        .with_payload(attestation_payload)
        .with_author(federation_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("FederationMembershipAttestation".to_string())
        .with_parent(join_request_cid)
        .build()?;
    
    let signature = federation_admin_did_key.sign(&attestation_node.canonical_bytes()?);
    let signed_attestation_node = attestation_node.sign(signature.to_bytes().to_vec())?;
    let attestation_cid = dag_store.add_node(signed_attestation_node).await?;
    println!("Created membership attestation with CID: {}", attestation_cid);
    
    // Step 10: Create a LineageAttestation to link the federation and community DAGs
    let lineage_attestation = LineageAttestation::new_join_attestation(
        &federation_id,
        federation_genesis_cid,
        NodeScope::Community,
        &community_id,
        community_genesis_cid,
        attestation_cid,
    );
    
    // Step 11: Sign the lineage attestation with both federation and community keys
    let lineage_federation_sig = ScopeSignature {
        signer: federation_admin_did.clone(),
        scope: NodeScope::Federation,
        scope_id: Some(federation_id.clone()),
        signature: federation_admin_did_key.sign(&lineage_attestation.canonical_bytes()?).to_bytes().to_vec(),
        timestamp: Utc::now(),
    };
    
    let mut signed_lineage = lineage_attestation;
    signed_lineage.add_signature(lineage_federation_sig);
    
    let lineage_community_sig = ScopeSignature {
        signer: community_admin_did.clone(),
        scope: NodeScope::Community,
        scope_id: Some(community_id.clone()),
        signature: community_admin_did_key.sign(&signed_lineage.canonical_bytes()?).to_bytes().to_vec(),
        timestamp: Utc::now(),
    };
    
    signed_lineage.add_signature(lineage_community_sig);
    
    // Verify the lineage attestation is complete
    assert!(signed_lineage.is_complete());
    
    // Step 12: Create a DAG node for the lineage attestation
    let lineage_json = serde_json::to_value(&signed_lineage)?;
    let lineage_payload = DagPayload::Json(lineage_json);
    
    let lineage_node = DagNodeBuilder::new()
        .with_payload(lineage_payload)
        .with_author(federation_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("LineageAttestation".to_string())
        .with_parent(join_request_cid)
        .with_parent(attestation_cid)
        .build()?;
    
    let signature = federation_admin_did_key.sign(&lineage_node.canonical_bytes()?);
    let signed_lineage_node = lineage_node.sign(signature.to_bytes().to_vec())?;
    let lineage_cid = dag_store.add_node(signed_lineage_node).await?;
    println!("Created lineage attestation with CID: {}", lineage_cid);
    
    // Step 13: Create approval node to finalize the join process
    let approval_payload = DagPayload::Json(json!({
        "type": "FederationJoinApproval",
        "scopeType": "Community",
        "scopeId": community_id,
        "federationId": federation_id,
        "requestCid": join_request_cid.to_string(),
        "attestationCid": attestation_cid.to_string(),
        "lineageCid": lineage_cid.to_string(),
        "approvedAt": Utc::now().to_rfc3339(),
        "approver": federation_admin_did.to_string(),
    }));
    
    let approval_node = DagNodeBuilder::new()
        .with_payload(approval_payload)
        .with_author(federation_admin_did.clone())
        .with_federation_id(federation_id.clone())
        .with_scope(NodeScope::Federation)
        .with_label("FederationJoinApproval".to_string())
        .with_parent(join_request_cid)
        .with_parent(attestation_cid)
        .with_parent(lineage_cid)
        .build()?;
    
    let signature = federation_admin_did_key.sign(&approval_node.canonical_bytes()?);
    let signed_approval_node = approval_node.sign(signature.to_bytes().to_vec())?;
    let approval_cid = dag_store.add_node(signed_approval_node).await?;
    println!("Created join approval with CID: {}", approval_cid);
    
    // Final verification: ensure the community is now part of the federation
    // This would typically involve querying the DAG to find attestations
    let join_approval_nodes = dag_store.get_nodes_by_payload_type("FederationJoinApproval").await?;
    assert!(!join_approval_nodes.is_empty());
    
    // Find the attestation for our community
    let mut found_approval = false;
    for node in &join_approval_nodes {
        if let DagPayload::Json(json_data) = &node.node.payload {
            if let Some(scope_id) = json_data.get("scopeId").and_then(|v| v.as_str()) {
                if scope_id == community_id {
                    found_approval = true;
                    break;
                }
            }
        }
    }
    assert!(found_approval, "Community join approval not found in DAG");
    
    println!("Community federation join test passed successfully!");
    Ok(())
} 