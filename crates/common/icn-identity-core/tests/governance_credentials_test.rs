use icn_identity_core::did::DidKey;
use icn_identity_core::vc::{
    // Import the new credential types
    ProposalCredential, 
    ProposalSubject, 
    ProposalType, 
    ProposalStatus,
    VotingThreshold,
    VotingDuration,
    VoteCredential,
    VoteSubject,
    VoteDecision
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// Helper function to get current Unix timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[test]
fn test_proposal_credential_creation_and_verification() {
    // Create DIDs for testing
    let federation_key = DidKey::new();
    let submitter_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    let submitter_did = submitter_key.did().to_string();
    
    // Create a test proposal subject
    let subject = ProposalSubject {
        id: federation_did.clone(),
        title: "Test Proposal".to_string(),
        description: "This is a test proposal for unit tests".to_string(),
        proposal_type: ProposalType::TextProposal,
        status: ProposalStatus::Draft,
        submitter: submitter_did.clone(),
        voting_threshold: VotingThreshold::Majority,
        voting_duration: VotingDuration::TimeBased(86400), // 1 day
        voting_start_time: now(),
        voting_end_time: Some(now() + 86400),
        execution_cid: None,
        thread_cid: Some("bafy2bzacebekzlnhf7hngknfvn4zaokmqrjb6e2jqrr5iqrdj2cytxnv3h6pg".to_string()),
        parameters: None,
        previous_version: None,
        event_id: None,
        created_at: now(),
        updated_at: now(),
    };
    
    // Create a credential ID
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    // Create and sign the proposal credential
    let proposal_cred = ProposalCredential::new(
        cred_id.clone(),
        federation_did.clone(),
        subject
    ).sign(&federation_key).expect("Should sign credential");
    
    // Verify the proposal credential
    assert!(proposal_cred.verify().expect("Should verify"));
    
    // Verify the credential fields
    assert_eq!(proposal_cred.id, cred_id);
    assert_eq!(proposal_cred.issuer, federation_did);
    assert_eq!(proposal_cred.credential_subject.submitter, submitter_did);
    assert_eq!(proposal_cred.credential_subject.status, ProposalStatus::Draft);
    assert!(proposal_cred.proof.is_some());
    
    // Convert to JSON and back
    let json = proposal_cred.to_json().expect("Should serialize to JSON");
    let parsed = ProposalCredential::from_json(&json).expect("Should parse from JSON");
    
    // Verify the roundtrip
    assert_eq!(proposal_cred.id, parsed.id);
    assert_eq!(proposal_cred.credential_subject.title, parsed.credential_subject.title);
    assert!(parsed.verify().expect("Should verify after parsing"));
}

#[test]
fn test_proposal_status_transition() {
    // Create DIDs for testing
    let federation_key = DidKey::new();
    let submitter_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    let submitter_did = submitter_key.did().to_string();
    
    // Create a test proposal subject
    let subject = ProposalSubject {
        id: federation_did.clone(),
        title: "Test Proposal".to_string(),
        description: "This is a test proposal for unit tests".to_string(),
        proposal_type: ProposalType::TextProposal,
        status: ProposalStatus::Draft,
        submitter: submitter_did.clone(),
        voting_threshold: VotingThreshold::Majority,
        voting_duration: VotingDuration::TimeBased(86400), // 1 day
        voting_start_time: now(),
        voting_end_time: Some(now() + 86400),
        execution_cid: None,
        thread_cid: None,
        parameters: None,
        previous_version: None,
        event_id: None,
        created_at: now(),
        updated_at: now(),
    };
    
    // Create a credential
    let mut proposal_cred = ProposalCredential::new(
        format!("urn:uuid:{}", Uuid::new_v4()),
        federation_did.clone(),
        subject
    );
    
    // Test valid transitions
    assert!(proposal_cred.update_status(ProposalStatus::Active).is_ok());
    assert_eq!(proposal_cred.credential_subject.status, ProposalStatus::Active);
    
    assert!(proposal_cred.update_status(ProposalStatus::Passed).is_ok());
    assert_eq!(proposal_cred.credential_subject.status, ProposalStatus::Passed);
    
    assert!(proposal_cred.update_status(ProposalStatus::Executed).is_ok());
    assert_eq!(proposal_cred.credential_subject.status, ProposalStatus::Executed);
    
    // Reset for testing invalid transition
    proposal_cred.credential_subject.status = ProposalStatus::Draft;
    
    // Test invalid transition (Draft -> Executed)
    let result = proposal_cred.update_status(ProposalStatus::Executed);
    assert!(result.is_err());
}

#[test]
fn test_vote_credential_creation_and_verification() {
    // Create DIDs for testing
    let federation_key = DidKey::new();
    let voter_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    let voter_did = voter_key.did().to_string();
    
    // Create a test vote subject
    let subject = VoteSubject {
        id: voter_did.clone(),
        federation_id: federation_did.clone(),
        proposal_id: "urn:uuid:123e4567-e89b-12d3-a456-426614174000".to_string(),
        decision: VoteDecision::Yes,
        voting_power: None,
        justification: Some("I support this proposal".to_string()),
        delegate_for: None,
        is_amendment: false,
        previous_vote_id: None,
        cast_at: now(),
    };
    
    // Create a credential ID
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    // Create and sign the vote credential using the voter's key
    let vote_cred = VoteCredential::new(
        cred_id.clone(),
        federation_did.clone(),
        subject
    ).sign(&voter_key).expect("Should sign credential");
    
    // Verify the vote credential
    assert!(vote_cred.verify().expect("Should verify"));
    
    // Verify the credential fields
    assert_eq!(vote_cred.id, cred_id);
    assert_eq!(vote_cred.issuer, federation_did);
    assert_eq!(vote_cred.credential_subject.id, voter_did);
    assert_eq!(vote_cred.credential_subject.decision, VoteDecision::Yes);
    assert!(vote_cred.proof.is_some());
    
    // Convert to JSON and back
    let json = vote_cred.to_json().expect("Should serialize to JSON");
    let parsed = VoteCredential::from_json(&json).expect("Should parse from JSON");
    
    // Verify the roundtrip
    assert_eq!(vote_cred.id, parsed.id);
    assert_eq!(vote_cred.credential_subject.decision, parsed.credential_subject.decision);
    assert!(parsed.verify().expect("Should verify after parsing"));
}

#[test]
fn test_vote_credential_signer_validation() {
    // Create DIDs for testing
    let federation_key = DidKey::new();
    let voter_key = DidKey::new();
    let different_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    let voter_did = voter_key.did().to_string();
    
    // Create a test vote subject
    let subject = VoteSubject {
        id: voter_did.clone(),
        federation_id: federation_did.clone(),
        proposal_id: "urn:uuid:123e4567-e89b-12d3-a456-426614174000".to_string(),
        decision: VoteDecision::No,
        voting_power: None,
        justification: None,
        delegate_for: None,
        is_amendment: false,
        previous_vote_id: None,
        cast_at: now(),
    };
    
    // Create a credential ID
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    // Create a vote credential
    let vote_cred = VoteCredential::new(
        cred_id.clone(),
        federation_did.clone(),
        subject
    );
    
    // Try to sign with a different key than the voter - should fail
    let result = vote_cred.sign(&different_key);
    assert!(result.is_err());
    
    // Sign with the correct voter key - should succeed
    let result = vote_cred.sign(&voter_key);
    assert!(result.is_ok());
} 