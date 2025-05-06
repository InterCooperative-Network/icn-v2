use icn_identity_core::{
    did::DidKey,
    QuorumEngine,
    QuorumOutcome,
    ProposalCredential,
    ProposalSubject,
    ProposalStatus,
    ProposalType,
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

// Helper function to create a test proposal
fn create_test_proposal(
    federation_key: &DidKey, 
    status: ProposalStatus,
    threshold: VotingThreshold,
    duration: VotingDuration,
) -> ProposalCredential {
    let federation_did = federation_key.did().to_string();
    let submitter_did = federation_did.clone(); // Using federation as submitter for simplicity
    
    let start_time = now();
    let end_time = match duration {
        VotingDuration::TimeBased(secs) => Some(start_time + secs),
        _ => None,
    };
    
    let subject = ProposalSubject {
        id: federation_did.clone(),
        title: "Test Proposal".to_string(),
        description: "This is a test proposal for quorum engine".to_string(),
        proposal_type: ProposalType::TextProposal,
        status,
        submitter: submitter_did,
        voting_threshold: threshold,
        voting_duration: duration,
        voting_start_time: start_time,
        voting_end_time: end_time,
        execution_cid: None,
        thread_cid: None,
        parameters: None,
        previous_version: None,
        event_id: None,
        created_at: start_time,
        updated_at: start_time,
    };
    
    let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    let proposal = ProposalCredential::new(
        cred_id,
        federation_did,
        subject
    ).sign(federation_key).expect("Should sign credential");
    
    proposal
}

// Helper function to create a test vote
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
    
    let vote = VoteCredential::new(
        cred_id,
        federation_did.to_string(),
        subject
    ).sign(voter_key).expect("Should sign vote");
    
    vote
}

#[test]
fn test_majority_voting() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter1_key = DidKey::new();
    let voter2_key = DidKey::new();
    let voter3_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create an active proposal with majority threshold
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Majority,
        VotingDuration::TimeBased(86400), // 1 day
    );
    
    // Create votes (2 yes, 1 no)
    let vote1 = create_test_vote(&voter1_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote2 = create_test_vote(&voter2_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote3 = create_test_vote(&voter3_key, &federation_did, &proposal.id, VoteDecision::No);
    
    let votes = vec![vote1, vote2, vote3];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // Since voting period hasn't ended (it's 1 day), outcome should be Inconclusive
    assert_eq!(tally.outcome, QuorumOutcome::Inconclusive);
    
    // Verify vote counts
    assert_eq!(tally.yes_votes, 2);
    assert_eq!(tally.no_votes, 1);
    assert_eq!(tally.yes_power, 2);
    assert_eq!(tally.no_power, 1);
    
    // Create a proposal with elapsed voting period (end time in the past)
    let current_time = now();
    let mut proposal_ended = proposal.clone();
    proposal_ended.credential_subject.voting_start_time = current_time - 86401; // Started yesterday
    proposal_ended.credential_subject.voting_end_time = Some(current_time - 1); // Ended 1 second ago
    
    // Evaluate again
    let tally_ended = engine.evaluate(&proposal_ended, &votes).expect("Should evaluate votes");
    
    // Now we should get a definitive outcome (Passed)
    assert_eq!(tally_ended.outcome, QuorumOutcome::Passed);
}

#[test]
fn test_percentage_voting() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter1_key = DidKey::new();
    let voter2_key = DidKey::new();
    let voter3_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create an active proposal with 67% threshold (need 2/3 yes votes)
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Percentage(67),
        VotingDuration::TimeBased(0), // Instant voting period (will end immediately)
    );
    
    // Create votes (2 yes, 1 no)
    let vote1 = create_test_vote(&voter1_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote2 = create_test_vote(&voter2_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote3 = create_test_vote(&voter3_key, &federation_did, &proposal.id, VoteDecision::No);
    
    let votes = vec![vote1, vote2, vote3];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // 2/3 = 66.66% which is less than 67%, so it should fail
    assert_eq!(tally.outcome, QuorumOutcome::Failed);
    
    // Create another proposal with 66% threshold (should pass with 2/3 votes)
    let proposal66 = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Percentage(66),
        VotingDuration::TimeBased(0),
    );
    
    let tally66 = engine.evaluate(&proposal66, &votes).expect("Should evaluate votes");
    
    // Now 2/3 = 66.66% which is > 66%, so it should pass
    assert_eq!(tally66.outcome, QuorumOutcome::Passed);
}

#[test]
fn test_unanimous_voting() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter1_key = DidKey::new();
    let voter2_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create an active proposal with unanimous threshold
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Unanimous,
        VotingDuration::TimeBased(0), // Instant voting period
    );
    
    // Create votes (all yes)
    let vote1 = create_test_vote(&voter1_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote2 = create_test_vote(&voter2_key, &federation_did, &proposal.id, VoteDecision::Yes);
    
    let votes = vec![vote1, vote2];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // All votes are yes, so it should pass
    assert_eq!(tally.outcome, QuorumOutcome::Passed);
    
    // Now add a no vote
    let voter3_key = DidKey::new();
    let vote3 = create_test_vote(&voter3_key, &federation_did, &proposal.id, VoteDecision::No);
    
    let votes_with_no = vec![vote1, vote2, vote3];
    
    let tally_with_no = engine.evaluate(&proposal, &votes_with_no).expect("Should evaluate votes");
    
    // With a single no vote, unanimous should fail
    assert_eq!(tally_with_no.outcome, QuorumOutcome::Failed);
}

#[test]
fn test_invalid_proposal_state() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create a draft proposal (not in Active state)
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Draft,
        VotingThreshold::Majority,
        VotingDuration::TimeBased(86400),
    );
    
    // Create a vote
    let vote = create_test_vote(&voter_key, &federation_did, &proposal.id, VoteDecision::Yes);
    
    let votes = vec![vote];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let result = engine.evaluate(&proposal, &votes);
    
    // Should get error because proposal is not in Active state
    assert!(result.is_err());
}

#[test]
fn test_veto_vote() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter1_key = DidKey::new();
    let voter2_key = DidKey::new();
    let voter3_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create an active proposal with majority threshold
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Majority,
        VotingDuration::TimeBased(0), // Instant voting period
    );
    
    // Create votes (2 yes, 1 veto)
    let vote1 = create_test_vote(&voter1_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote2 = create_test_vote(&voter2_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let vote3 = create_test_vote(&voter3_key, &federation_did, &proposal.id, VoteDecision::Veto);
    
    let votes = vec![vote1, vote2, vote3];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // Any veto should cause the proposal to fail
    assert_eq!(tally.outcome, QuorumOutcome::Failed);
    assert_eq!(tally.veto_votes, 1);
}

#[test]
fn test_vote_amendments() {
    // Setup keys
    let federation_key = DidKey::new();
    let voter_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    
    // Create an active proposal with majority threshold
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Majority,
        VotingDuration::TimeBased(0), // Instant voting period
    );
    
    // Create an initial "No" vote
    let vote1 = create_test_vote(&voter_key, &federation_did, &proposal.id, VoteDecision::No);
    
    // Create an amended "Yes" vote (cast later)
    let mut vote2 = create_test_vote(&voter_key, &federation_did, &proposal.id, VoteDecision::Yes);
    // Make sure the timestamp is later
    vote2.credential_subject.cast_at = vote1.credential_subject.cast_at + 10;
    vote2.credential_subject.is_amendment = true;
    vote2.credential_subject.previous_vote_id = Some(vote1.id.clone());
    
    let votes = vec![vote1, vote2];
    
    // Create quorum engine and evaluate
    let engine = QuorumEngine::new();
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // Only the amended vote (Yes) should count
    assert_eq!(tally.yes_votes, 1);
    assert_eq!(tally.no_votes, 0);
    assert_eq!(tally.outcome, QuorumOutcome::Passed);
}

#[test]
fn test_member_restricted_voting() {
    // Setup keys
    let federation_key = DidKey::new();
    let member_key = DidKey::new();
    let nonmember_key = DidKey::new();
    
    let federation_did = federation_key.did().to_string();
    let member_did = member_key.did().to_string();
    
    // Create an active proposal with majority threshold
    let proposal = create_test_proposal(
        &federation_key,
        ProposalStatus::Active,
        VotingThreshold::Majority,
        VotingDuration::TimeBased(0), // Instant voting period
    );
    
    // Create votes from member and non-member
    let member_vote = create_test_vote(&member_key, &federation_did, &proposal.id, VoteDecision::Yes);
    let nonmember_vote = create_test_vote(&nonmember_key, &federation_did, &proposal.id, VoteDecision::No);
    
    let votes = vec![member_vote, nonmember_vote];
    
    // Create quorum engine with restricted members list
    let engine = QuorumEngine::with_members(vec![member_did]);
    let tally = engine.evaluate(&proposal, &votes).expect("Should evaluate votes");
    
    // Only the member's vote should count
    assert_eq!(tally.total_votes, 1);
    assert_eq!(tally.yes_votes, 1);
    assert_eq!(tally.no_votes, 0);
    assert_eq!(tally.outcome, QuorumOutcome::Passed);
} 