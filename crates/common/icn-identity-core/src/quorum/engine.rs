use crate::vc::{
    ProposalCredential, 
    VoteCredential, 
    VoteDecision,
    VotingThreshold
};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use thiserror::Error;
use chrono::Utc;

/// Errors related to quorum evaluation
#[derive(Error, Debug)]
pub enum QuorumEngineError {
    #[error("Invalid proposal state: {0}")]
    InvalidProposalState(String),
    
    #[error("Invalid vote: {0}")]
    InvalidVote(String),
    
    #[error("Voting period not started or already ended")]
    InvalidVotingPeriod,
    
    #[error("Missing required data: {0}")]
    MissingData(String),
    
    #[error("Unable to resolve members: {0}")]
    MemberResolutionError(String),
}

/// The outcome of a quorum evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumOutcome {
    /// Proposal passed the quorum requirements
    Passed,
    
    /// Proposal failed to meet the quorum requirements
    Failed,
    
    /// Voting period still active, outcome inconclusive
    Inconclusive,
    
    /// Proposal cannot be voted on (wrong state, etc.)
    Invalid,
}

/// Detailed results from quorum calculation
#[derive(Debug, Clone)]
pub struct QuorumTally {
    /// Total number of votes cast
    pub total_votes: usize,
    
    /// Number of yes votes
    pub yes_votes: usize,
    
    /// Number of no votes
    pub no_votes: usize,
    
    /// Number of abstain votes
    pub abstain_votes: usize,
    
    /// Number of veto votes (if supported)
    pub veto_votes: usize,
    
    /// Total voting power of yes votes (for weighted voting)
    pub yes_power: u64,
    
    /// Total voting power of no votes (for weighted voting)
    pub no_power: u64,
    
    /// Total voting power of abstain votes (for weighted voting)
    pub abstain_power: u64,
    
    /// Total voting power of veto votes (for weighted voting)
    pub veto_power: u64,
    
    /// Total available voting power
    pub total_power: u64,
    
    /// Unique DIDs who have voted
    pub voters: HashSet<String>,
    
    /// Required threshold to pass (percentage or count, depends on voting rule)
    pub threshold: String,
    
    /// Final outcome
    pub outcome: QuorumOutcome,
}

/// A service that evaluates votes against proposal thresholds
pub struct QuorumEngine {
    /// Optional list of federation members DIDs (used for membership validation)
    members: Option<Vec<String>>,
    
    /// Optional map of member voting power (used for weighted voting)
    member_weights: Option<HashMap<String, u64>>,
}

impl QuorumEngine {
    /// Create a new QuorumEngine with no member restrictions
    pub fn new() -> Self {
        Self {
            members: None,
            member_weights: None,
        }
    }
    
    /// Create a new QuorumEngine with specified members
    pub fn with_members(members: Vec<String>) -> Self {
        Self {
            members: Some(members),
            member_weights: None,
        }
    }
    
    /// Create a new QuorumEngine with weighted members
    pub fn with_weighted_members(member_weights: HashMap<String, u64>) -> Self {
        Self {
            members: Some(member_weights.keys().cloned().collect()),
            member_weights: Some(member_weights),
        }
    }
    
    /// Evaluate a set of votes against a proposal's quorum rules
    pub fn evaluate(
        &self, 
        proposal: &ProposalCredential, 
        votes: &[VoteCredential]
    ) -> Result<QuorumTally, QuorumEngineError> {
        // 1. Validate that voting is allowed for this proposal
        self.validate_proposal_votable(proposal)?;
        
        // 2. Validate each vote and filter out invalid ones
        let valid_votes = self.filter_valid_votes(proposal, votes)?;
        
        // 3. Create a tally of the votes
        let mut tally = QuorumTally {
            total_votes: valid_votes.len(),
            yes_votes: 0,
            no_votes: 0,
            abstain_votes: 0,
            veto_votes: 0,
            yes_power: 0,
            no_power: 0,
            abstain_power: 0,
            veto_power: 0,
            total_power: 0,
            voters: HashSet::new(),
            threshold: self.get_threshold_description(&proposal.credential_subject.voting_threshold),
            outcome: QuorumOutcome::Inconclusive,
        };
        
        // 4. Count votes and tally voting power
        for vote in valid_votes {
            tally.voters.insert(vote.credential_subject.id.clone());
            
            // Get voting power (default to 1 if not specified)
            let voting_power = vote.credential_subject.voting_power.unwrap_or(1);
            
            match vote.credential_subject.decision {
                VoteDecision::Yes => {
                    tally.yes_votes += 1;
                    tally.yes_power += voting_power;
                },
                VoteDecision::No => {
                    tally.no_votes += 1;
                    tally.no_power += voting_power;
                },
                VoteDecision::Abstain => {
                    tally.abstain_votes += 1;
                    tally.abstain_power += voting_power;
                },
                VoteDecision::Veto => {
                    tally.veto_votes += 1;
                    tally.veto_power += voting_power;
                },
            }
        }
        
        // Calculate total voting power
        tally.total_power = tally.yes_power + tally.no_power + tally.abstain_power + tally.veto_power;
        
        // 5. Check if voting period is still active
        if !self.is_voting_ended(proposal) {
            // If not ended, it's inconclusive regardless of current numbers
            tally.outcome = QuorumOutcome::Inconclusive;
            return Ok(tally);
        }
        
        // 6. Apply the threshold rules to determine outcome
        tally.outcome = self.apply_threshold_rules(proposal, &tally)?;
        
        Ok(tally)
    }
    
    /// Verify that a proposal can be voted on
    fn validate_proposal_votable(&self, proposal: &ProposalCredential) -> Result<(), QuorumEngineError> {
        use crate::vc::ProposalStatus;
        
        // Only 'Active' proposals can be voted on
        match &proposal.credential_subject.status {
            ProposalStatus::Active => Ok(()),
            state => Err(QuorumEngineError::InvalidProposalState(
                format!("Proposal must be in 'Active' state for voting, current state: {:?}", state)
            )),
        }
    }
    
    /// Get only valid votes for a proposal
    fn filter_valid_votes<'a>(
        &self,
        proposal: &ProposalCredential,
        votes: &'a [VoteCredential]
    ) -> Result<Vec<&'a VoteCredential>, QuorumEngineError> {
        let mut valid_votes = Vec::new();
        let mut latest_votes: HashMap<String, &'a VoteCredential> = HashMap::new();
        
        // 1. Find the latest vote from each voter
        for vote in votes {
            // Check basic validity
            if !self.is_vote_valid_for_proposal(proposal, vote)? {
                continue;
            }
            
            let voter_id = &vote.credential_subject.id;
            
            // Track the latest vote for each voter by timestamp
            match latest_votes.entry(voter_id.clone()) {
                Entry::Occupied(mut entry) => {
                    if vote.credential_subject.cast_at > entry.get().credential_subject.cast_at {
                        entry.insert(vote);
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(vote);
                }
            }
        }
        
        // 2. Use only the latest vote from each voter
        for vote in latest_votes.values() {
            valid_votes.push(*vote);
        }
        
        Ok(valid_votes)
    }
    
    /// Check if an individual vote is valid for a proposal
    fn is_vote_valid_for_proposal(
        &self,
        proposal: &ProposalCredential,
        vote: &VoteCredential
    ) -> Result<bool, QuorumEngineError> {
        // Check if vote references the proposal correctly
        if vote.credential_subject.proposal_id != proposal.id {
            return Ok(false);
        }
        
        // Check if vote is for the right federation
        if vote.credential_subject.federation_id != proposal.credential_subject.id {
            return Ok(false);
        }
        
        // Check if the vote is from a member (if members list is provided)
        if let Some(members) = &self.members {
            if !members.contains(&vote.credential_subject.id) {
                return Ok(false);
            }
        }
        
        // Verify vote was cast during the voting period
        if !self.is_vote_in_voting_period(proposal, vote)? {
            return Ok(false);
        }
        
        // All checks passed
        Ok(true)
    }
    
    /// Check if a vote was cast during the voting period
    fn is_vote_in_voting_period(
        &self,
        proposal: &ProposalCredential,
        vote: &VoteCredential
    ) -> Result<bool, QuorumEngineError> {
        // Get voting period start/end
        let start_time = proposal.credential_subject.voting_start_time;
        
        // Vote must be cast after voting starts
        if vote.credential_subject.cast_at < start_time {
            return Ok(false);
        }
        
        // Check end time if one is specified
        if let Some(end_time) = proposal.credential_subject.voting_end_time {
            if vote.credential_subject.cast_at > end_time {
                return Ok(false);
            }
        }
        
        // Vote is within the valid period
        Ok(true)
    }
    
    /// Check if voting has ended for a proposal
    fn is_voting_ended(&self, proposal: &ProposalCredential) -> bool {
        let now = Utc::now().timestamp() as u64;
        
        match proposal.credential_subject.voting_end_time {
            // If there's a specific end time, check if we've passed it
            Some(end_time) => now >= end_time,
            
            // For open-ended proposals, voting never automatically ends
            None => false,
        }
    }
    
    /// Apply the threshold rules to determine if quorum was reached
    fn apply_threshold_rules(
        &self,
        proposal: &ProposalCredential,
        tally: &QuorumTally
    ) -> Result<QuorumOutcome, QuorumEngineError> {
        use crate::vc::VotingThreshold;
        
        // If there are any veto votes (and veto is supported in governance), fail
        if tally.veto_votes > 0 {
            return Ok(QuorumOutcome::Failed);
        }
        
        // Apply different rules based on threshold type
        match &proposal.credential_subject.voting_threshold {
            VotingThreshold::Majority => {
                // Simple majority rule: yes votes > no votes
                if tally.yes_power > tally.no_power {
                    // Additional check: ensure there's at least one vote
                    if tally.total_votes > 0 {
                        Ok(QuorumOutcome::Passed)
                    } else {
                        Ok(QuorumOutcome::Inconclusive)
                    }
                } else {
                    Ok(QuorumOutcome::Failed)
                }
            },
            
            VotingThreshold::Percentage(threshold) => {
                // Check if yes votes meet the required percentage threshold
                let total_yes_no = tally.yes_power + tally.no_power;
                
                // Avoid division by zero
                if total_yes_no == 0 {
                    return Ok(QuorumOutcome::Inconclusive);
                }
                
                let yes_percentage = (tally.yes_power * 100) / total_yes_no;
                
                if yes_percentage >= *threshold as u64 {
                    Ok(QuorumOutcome::Passed)
                } else {
                    Ok(QuorumOutcome::Failed)
                }
            },
            
            VotingThreshold::Unanimous => {
                // For unanimous, all votes must be yes and there must be at least one vote
                if tally.total_votes > 0 && tally.no_votes == 0 && tally.veto_votes == 0 {
                    // If members list is provided, check if all members voted yes
                    if let Some(members) = &self.members {
                        if tally.voters.len() == members.len() {
                            Ok(QuorumOutcome::Passed)
                        } else {
                            // Not all members voted
                            Ok(QuorumOutcome::Inconclusive)
                        }
                    } else {
                        // No members list, just check if there are yes votes
                        if tally.yes_votes > 0 {
                            Ok(QuorumOutcome::Passed)
                        } else {
                            Ok(QuorumOutcome::Inconclusive)
                        }
                    }
                } else {
                    Ok(QuorumOutcome::Failed)
                }
            },
            
            VotingThreshold::Weighted { weights, threshold } => {
                // For weighted voting, calculate based on the provided weights
                
                // Use provided weights, or fall back to the engine's weights
                let weights_map: HashMap<String, u64> = if !weights.is_empty() {
                    weights.iter().cloned().collect()
                } else if let Some(member_weights) = &self.member_weights {
                    member_weights.clone()
                } else {
                    return Err(QuorumEngineError::MissingData(
                        "Weighted voting requires weights to be provided either in the proposal or the engine".to_string()
                    ));
                };
                
                // Calculate total possible weight
                let mut total_possible_weight: u64 = weights_map.values().sum();
                if total_possible_weight == 0 {
                    total_possible_weight = 1; // Avoid division by zero
                }
                
                // Calculate weight of yes votes
                let mut yes_weight = 0u64;
                
                // We already know the voting power from the tally
                // No need to check individual votes again
                if tally.yes_power >= *threshold {
                    Ok(QuorumOutcome::Passed)
                } else {
                    Ok(QuorumOutcome::Failed)
                }
            }
        }
    }
    
    /// Get a human-readable description of the threshold
    fn get_threshold_description(&self, threshold: &VotingThreshold) -> String {
        match threshold {
            VotingThreshold::Majority => "Simple majority (>50%)".to_string(),
            VotingThreshold::Percentage(p) => format!("{}% approval", p),
            VotingThreshold::Unanimous => "Unanimous approval".to_string(),
            VotingThreshold::Weighted { threshold, .. } => format!("Weighted voting (threshold: {})", threshold),
        }
    }
} 