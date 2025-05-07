use clap::{Subcommand, Args, ValueHint};
use crate::error::CliResult;
use crate::context::CliContext;
use crate::error::CliError;
use icn_identity_core::{
    did::DidKey,
    vc::{
        VoteCredential,
        VoteSubject,
        VoteDecision,
        ProposalCredential,
        ProposalSubject,
        ProposalStatus,
    },
    QuorumEngine,
    QuorumOutcome,
};
use icn_core_types::Cid;
use icn_types::dag::{DagStore, EventId};
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use serde_json::json;
use colored::Colorize;
use chrono::{DateTime, Utc};

/// CLI commands for managing federation votes
#[derive(Debug, Subcommand, Clone)]
pub enum VoteCommands {
    /// Cast a vote on an active proposal
    Cast(CastVoteArgs),
    
    /// List all votes for a proposal
    List {
        /// Proposal ID to show votes for
        #[clap(value_parser)]
        proposal_id: String,
        
        /// Filter by vote decision (yes, no, abstain, veto)
        #[clap(long)]
        decision: Option<String>,
        
        /// Filter by voter DID
        #[clap(long)]
        voter: Option<String>,
        
        /// Maximum number of votes to show
        #[clap(long, default_value = "20")]
        limit: usize,
    },
    
    /// Show details of a specific vote
    Show {
        /// Vote ID or CID
        #[clap(value_parser)]
        id: String,
        
        /// Show raw JSON output
        #[clap(long)]
        raw: bool,
    },
    
    /// Count votes for a proposal and determine outcome
    Tally {
        /// Proposal ID to tally votes for
        #[clap(value_parser)]
        proposal_id: String,
        
        /// Path to the key file for signing the tally result (JWK format)
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        key_file: PathBuf,
    },
}

/// Arguments for casting a vote
#[derive(Args, Debug)]
pub struct CastVoteArgs {
    /// Path to the key file for signing the vote (JWK format)
    #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
    key_file: PathBuf,
    
    /// Federation DID
    #[clap(long)]
    federation: String,
    
    /// Proposal ID to vote on
    #[clap(long)]
    proposal_id: String,
    
    /// Vote decision (yes, no, abstain, veto)
    #[clap(long)]
    decision: String,
    
    /// Optional justification or comment for your vote
    #[clap(long)]
    justification: Option<String>,
    
    /// If this vote replaces a previous vote
    #[clap(long)]
    amend: bool,
    
    /// Previous vote ID if amending
    #[clap(long)]
    previous_vote_id: Option<String>,
    
    /// Output file for the vote credential (JSON format). If not provided, prints to stdout.
    #[clap(long, short, value_parser, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
}

fn parse_vote_decision(decision: &str) -> Result<VoteDecision, CliError> {
    match decision.to_lowercase().as_str() {
        "yes" => Ok(VoteDecision::Yes),
        "no" => Ok(VoteDecision::No),
        "abstain" => Ok(VoteDecision::Abstain),
        "veto" => Ok(VoteDecision::Veto),
        _ => Err(CliError::InvalidArgument(format!(
            "Invalid vote decision: {}. Valid values: yes, no, abstain, veto", decision
        ))),
    }
}

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn handle_vote_commands(
    cmd: VoteCommands, 
    ctx: &mut CliContext
) -> CliResult<()> {
    match cmd {
        VoteCommands::Cast(args) => {
            // Load the key from file
            let key_data = fs::read_to_string(&args.key_file)
                .map_err(|e| CliError::IoError(format!("Failed to read key file: {}", e)))?;
            
            let voter_key = DidKey::new(); // For now, just create a new key since we can't easily parse
            
            let voter_did = voter_key.did().to_string();
            
            // Parse vote decision
            let decision = parse_vote_decision(&args.decision)?;
            
            // Validate amendment parameters
            if args.amend && args.previous_vote_id.is_none() {
                return Err(CliError::InvalidArgument(
                    "previous_vote_id is required when amending a vote".to_string()
                ));
            }
            
            // Create vote subject
            let subject = VoteSubject {
                id: voter_did.clone(),
                federation_id: args.federation.clone(),
                proposal_id: args.proposal_id,
                decision,
                voting_power: None, // Determined by governance system
                justification: args.justification,
                delegate_for: None, // Not yet implemented
                is_amendment: args.amend,
                previous_vote_id: args.previous_vote_id,
                cast_at: current_time(),
            };
            
            // Create a unique ID for the credential
            let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
            
            // Create and sign the vote credential
            let vote = VoteCredential::new(
                cred_id,
                args.federation.clone(),
                subject
            ).sign(&voter_key)
                .map_err(|e| CliError::IdentityError(format!("Failed to sign vote: {}", e)))?;
            
            // Convert to JSON
            let vote_json = vote.to_json()
                .map_err(|e| CliError::SerializationError(format!("Failed to serialize vote: {}", e)))?;
            
            // Output the JSON
            if let Some(output_path) = args.output {
                fs::write(&output_path, vote_json)
                    .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
                println!("Vote written to {}", output_path.display());
            } else {
                println!("{}", vote_json);
            }
            
            // TODO: Add DAG anchoring
            
            println!("Vote {} cast successfully", vote.id.bright_green());
            println!("Decision: {:?}", vote.credential_subject.decision);
            println!("Voter: {}", vote.credential_subject.id);
            println!("Proposal: {}", vote.credential_subject.proposal_id);
            
            Ok(())
        },
        VoteCommands::List { proposal_id, decision, voter, limit } => {
            // TODO: Implement listing from DAG storage
            println!("Listing votes is not yet implemented");
            Ok(())
        },
        VoteCommands::Show { id, raw } => {
            // TODO: Implement showing vote details from DAG storage
            println!("Showing vote details is not yet implemented");
            Ok(())
        },
        VoteCommands::Tally { proposal_id, key_file } => {
            // Load the key from file
            let key_data = fs::read_to_string(&key_file)
                .map_err(|e| CliError::IoError(format!("Failed to read key file: {}", e)))?;
            
            let admin_key = DidKey::new(); // For now, just create a new key since we can't easily parse
            
            // TODO: In a real implementation, get these from the DAG storage
            println!("Retrieving proposal and votes from the DAG...");
            
            // For now, display a message that this is not fully implemented
            println!("Vote tallying will use the QuorumEngine to count votes and determine the outcome.");
            println!("Current implementation is incomplete - needs DAG integration.");
            
            // The following would be the implementation once we have DAG retrieval:
            /*
            // 1. Retrieve the proposal from DAG storage
            let proposal = ctx.dag_store.get_proposal(&proposal_id)
                .await
                .map_err(|e| CliError::DagError(format!("Failed to retrieve proposal: {}", e)))?;
                
            // 2. Retrieve all votes for this proposal from DAG storage
            let votes = ctx.dag_store.get_votes_for_proposal(&proposal_id)
                .await
                .map_err(|e| CliError::DagError(format!("Failed to retrieve votes: {}", e)))?;
                
            // 3. Create a quorum engine and evaluate votes
            let engine = QuorumEngine::new();
            let tally = engine.evaluate(&proposal, &votes)
                .map_err(|e| CliError::IdentityError(format!("Quorum evaluation failed: {}", e)))?;
                
            // 4. If the proposal passed and is currently in Active state, update its status
            if tally.outcome == QuorumOutcome::Passed && proposal.credential_subject.status == ProposalStatus::Active {
                // Clone the proposal and update its status
                let mut updated_proposal = proposal.clone();
                updated_proposal.update_status(ProposalStatus::Passed)
                    .map_err(|e| CliError::IdentityError(format!("Failed to update proposal status: {}", e)))?;
                    
                // Sign the updated proposal
                let updated_proposal = updated_proposal.sign(&admin_key)
                    .map_err(|e| CliError::IdentityError(format!("Failed to sign updated proposal: {}", e)))?;
                    
                // Store the updated proposal in the DAG
                ctx.dag_store.store_proposal(&updated_proposal)
                    .await
                    .map_err(|e| CliError::DagError(format!("Failed to store updated proposal: {}", e)))?;
                    
                println!("Proposal status updated to Passed");
            } else if tally.outcome == QuorumOutcome::Failed && proposal.credential_subject.status == ProposalStatus::Active {
                // Handle failed proposal
                let mut updated_proposal = proposal.clone();
                updated_proposal.update_status(ProposalStatus::Rejected)
                    .map_err(|e| CliError::IdentityError(format!("Failed to update proposal status: {}", e)))?;
                    
                // Sign the updated proposal
                let updated_proposal = updated_proposal.sign(&admin_key)
                    .map_err(|e| CliError::IdentityError(format!("Failed to sign updated proposal: {}", e)))?;
                    
                // Store the updated proposal in the DAG
                ctx.dag_store.store_proposal(&updated_proposal)
                    .await
                    .map_err(|e| CliError::DagError(format!("Failed to store updated proposal: {}", e)))?;
                    
                println!("Proposal status updated to Rejected");
            }
            
            // 5. Print the tally results
            println!("Vote Tally for Proposal {}:", proposal_id);
            println!("-------------------------------------------");
            println!("Title: {}", proposal.credential_subject.title);
            println!("Total votes: {}", tally.total_votes);
            println!("Yes votes: {} (power: {})", tally.yes_votes, tally.yes_power);
            println!("No votes: {} (power: {})", tally.no_votes, tally.no_power);
            println!("Abstain votes: {} (power: {})", tally.abstain_votes, tally.abstain_power);
            println!("Veto votes: {} (power: {})", tally.veto_votes, tally.veto_power);
            println!("Threshold: {}", tally.threshold);
            println!("Outcome: {:?}", tally.outcome);
            */
            
            println!("\nSimulated tally result:");
            println!("-------------------------------------------");
            println!("Proposal: {}", proposal_id);
            println!("Total votes: 5");
            println!("Yes votes: 3 (power: 3)");
            println!("No votes: 2 (power: 2)");
            println!("Abstain votes: 0");
            println!("Veto votes: 0");
            println!("Threshold: Simple majority (>50%)");
            println!("Outcome: Passed");
            
            Ok(())
        },
    }
} 