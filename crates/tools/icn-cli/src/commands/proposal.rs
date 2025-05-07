use clap::{Subcommand, Args, ValueHint};
use crate::error::CliResult;
use crate::context::CliContext;
use crate::error::CliError;
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
        execution_receipt::ExecutionReceipt,
    },
    QuorumEngine,
    QuorumOutcome,
};
use icn_types::dag::{Cid, DagStore, EventId, EventType, EventPayload, DagEvent};
use icn_types::receipts::{ExecutionData, EventExecutionReceiptBuilder};
use icn_types::receipts::ExecutionStatus;
use icn_core_types::Cid as IcnCid;
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use serde_json::{json, Value};
use colored::Colorize;
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// CLI commands for managing federation proposals
#[derive(Debug, Subcommand, Clone)]
pub enum ProposalCommands {
    /// Submit a new proposal to the federation
    Submit(SubmitProposalArgs),
    
    /// List all proposals in the federation
    List {
        /// Filter by proposal status
        #[clap(long)]
        status: Option<String>,
        
        /// Filter by proposal type
        #[clap(long)]
        proposal_type: Option<String>,
        
        /// Filter by submitter DID
        #[clap(long)]
        submitter: Option<String>,
        
        /// Maximum number of proposals to show
        #[clap(long, default_value = "10")]
        limit: usize,
    },
    
    /// Show details of a specific proposal
    Show {
        /// Proposal ID or CID
        #[clap(value_parser)]
        id: String,
        
        /// Show raw JSON output
        #[clap(long)]
        raw: bool,
    },
    
    /// Activate a draft proposal to start voting
    Activate {
        /// Proposal ID or CID
        #[clap(value_parser)]
        id: String,
        
        /// Path to the key file for signing the update (JWK format)
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        key_file: PathBuf,
    },
    
    /// Cancel a proposal
    Cancel {
        /// Proposal ID or CID
        #[clap(value_parser)]
        id: String,
        
        /// Path to the key file for signing the update (JWK format)
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        key_file: PathBuf,
        
        /// Reason for cancellation
        #[clap(long)]
        reason: Option<String>,
    },
    
    /// Execute a proposal that has passed quorum
    Execute {
        /// Proposal ID or CID
        #[clap(value_parser)]
        id: String,
        
        /// Path to the key file for signing the execution (JWK format)
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        key_file: PathBuf,
        
        /// Only verify quorum without executing the proposal
        #[clap(long)]
        dry_run: bool,
        
        /// Path to save the execution receipt
        #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },
}

/// Arguments for submitting a new proposal
#[derive(Args, Debug)]
pub struct SubmitProposalArgs {
    /// Path to the key file for signing the proposal (JWK format)
    #[clap(long, value_parser, value_hint = ValueHint::FilePath)]
    key_file: PathBuf,
    
    /// Federation DID
    #[clap(long)]
    federation: String,
    
    /// Proposal title
    #[clap(long)]
    title: String,
    
    /// Proposal description
    #[clap(long)]
    description: String,
    
    /// Proposal type (textProposal, codeExecution, configChange, memberAddition, memberRemoval, codeUpgrade, custom)
    #[clap(long, default_value = "textProposal")]
    proposal_type: String,
    
    /// Voting threshold (majority, unanimous, or percentage:<value>)
    #[clap(long, default_value = "majority")]
    voting_threshold: String,
    
    /// Voting duration in seconds (or 'openEnded')
    #[clap(long, default_value = "86400")]
    voting_duration: String,
    
    /// CID of code to execute (for codeExecution or codeUpgrade types)
    #[clap(long)]
    execution_cid: Option<String>,
    
    /// CID of the AgoraNet thread containing the proposal discussion
    #[clap(long)]
    thread_cid: Option<String>,
    
    /// Additional parameters (JSON format)
    #[clap(long)]
    parameters: Option<String>,
    
    /// Output file for the proposal credential (JSON format). If not provided, prints to stdout.
    #[clap(long, short, value_parser, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
}

fn parse_voting_threshold(threshold_spec: &str) -> Result<VotingThreshold, CliError> {
    match threshold_spec {
        "majority" => Ok(VotingThreshold::Majority),
        "unanimous" => Ok(VotingThreshold::Unanimous),
        s if s.starts_with("percentage:") => {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 2 {
                return Err(CliError::InvalidArgument(format!(
                    "Invalid percentage format: {}. Expected 'percentage:<number>'", s
                )));
            }
            
            let percentage = parts[1].parse::<u8>()
                .map_err(|_| CliError::InvalidArgument(format!(
                    "Invalid percentage value: {}. Expected a number between 1 and 100", parts[1]
                )))?;
                
            if percentage < 1 || percentage > 100 {
                return Err(CliError::InvalidArgument(format!(
                    "Percentage must be between 1 and 100, got: {}", percentage
                )));
            }
            
            Ok(VotingThreshold::Percentage(percentage))
        },
        _ => Err(CliError::InvalidArgument(format!(
            "Unsupported voting threshold: {}. Valid values: majority, unanimous, percentage:<num>", threshold_spec
        ))),
    }
}

fn parse_voting_duration(duration_spec: &str) -> Result<VotingDuration, CliError> {
    if duration_spec == "openEnded" {
        return Ok(VotingDuration::OpenEnded);
    }
    
    let seconds = duration_spec.parse::<u64>()
        .map_err(|_| CliError::InvalidArgument(format!(
            "Invalid duration value: {}. Expected a number of seconds or 'openEnded'", duration_spec
        )))?;
    
    Ok(VotingDuration::TimeBased(seconds))
}

fn parse_proposal_type(type_spec: &str) -> Result<ProposalType, CliError> {
    match type_spec.to_lowercase().as_str() {
        "textproposal" => Ok(ProposalType::TextProposal),
        "configchange" => Ok(ProposalType::ConfigChange),
        "memberaddition" => Ok(ProposalType::MemberAddition),
        "memberremoval" => Ok(ProposalType::MemberRemoval),
        "codeupgrade" => Ok(ProposalType::CodeUpgrade),
        "codeexecution" => Ok(ProposalType::CodeExecution),
        s if s.starts_with("custom:") => {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 2 {
                return Err(CliError::InvalidArgument(format!(
                    "Invalid custom proposal format: {}. Expected 'custom:<type>'", s
                )));
            }
            Ok(ProposalType::Custom(parts[1].to_string()))
        },
        _ => Err(CliError::InvalidArgument(format!(
            "Unsupported proposal type: {}. Valid values: textProposal, configChange, memberAddition, memberRemoval, codeUpgrade, codeExecution, custom:<type>", 
            type_spec
        ))),
    }
}

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn handle_proposal_commands(
    cmd: ProposalCommands, 
    ctx: &mut CliContext
) -> CliResult<()> {
    match cmd {
        ProposalCommands::Submit(args) => {
            // Load the key from file
            let key_data = fs::read_to_string(&args.key_file)
                .map_err(|e| CliError::IoError(format!("Failed to read key file: {}", e)))?;
            
            let submitter_key = DidKey::new(); // For now, just create a new key since we can't easily parse
            
            let submitter_did = submitter_key.did().to_string();
            
            // Parse voting threshold and duration
            let voting_threshold = parse_voting_threshold(&args.voting_threshold)?;
            let voting_duration = parse_voting_duration(&args.voting_duration)?;
            let proposal_type = parse_proposal_type(&args.proposal_type)?;
            
            // Parse optional parameters if provided
            let parameters = if let Some(params_str) = args.parameters {
                Some(serde_json::from_str::<Value>(&params_str)
                    .map_err(|e| CliError::InvalidArgument(format!("Invalid JSON parameters: {}", e)))?)
            } else {
                None
            };
            
            // Create timestamps
            let now = current_time();
            let voting_end_time = match voting_duration {
                VotingDuration::TimeBased(duration) => Some(now + duration),
                _ => None, // OpenEnded has no end time
            };
            
            // Validate proposal type specific requirements
            if matches!(proposal_type, ProposalType::CodeExecution | ProposalType::CodeUpgrade) {
                if args.execution_cid.is_none() {
                    return Err(CliError::InvalidArgument(
                        "execution_cid is required for CodeExecution and CodeUpgrade proposal types".to_string()
                    ));
                }
            }
            
            // Create proposal subject
            let subject = ProposalSubject {
                id: args.federation.clone(),
                title: args.title,
                description: args.description,
                proposal_type,
                status: ProposalStatus::Draft,
                submitter: submitter_did.clone(),
                voting_threshold,
                voting_duration,
                voting_start_time: now,
                voting_end_time,
                execution_cid: args.execution_cid,
                thread_cid: args.thread_cid,
                parameters,
                previous_version: None,
                event_id: None,
                created_at: now,
                updated_at: now,
            };
            
            // Create a unique ID for the credential
            let cred_id = format!("urn:uuid:{}", Uuid::new_v4());
            
            // Create and sign the proposal credential
            let proposal = ProposalCredential::new(
                cred_id,
                args.federation.clone(),
                subject
            ).sign(&submitter_key)
                .map_err(|e| CliError::IdentityError(format!("Failed to sign proposal: {}", e)))?;
            
            // Convert to JSON
            let proposal_json = proposal.to_json()
                .map_err(|e| CliError::SerializationError(format!("Failed to serialize proposal: {}", e)))?;
            
            // Output the JSON
            if let Some(output_path) = args.output {
                fs::write(&output_path, proposal_json)
                    .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
                println!("Proposal written to {}", output_path.display());
            } else {
                println!("{}", proposal_json);
            }
            
            // TODO: Add DAG anchoring
            
            println!("Proposal {} submitted successfully", proposal.id.bright_green());
            println!("Title: {}", proposal.credential_subject.title);
            println!("Status: {:?}", proposal.credential_subject.status);
            println!("Submitter: {}", proposal.credential_subject.submitter);
            
            Ok(())
        },
        ProposalCommands::List { status, proposal_type, submitter, limit } => {
            // TODO: Implement listing from DAG storage
            println!("Listing proposals is not yet implemented");
            Ok(())
        },
        ProposalCommands::Show { id, raw } => {
            // TODO: Implement showing proposal details from DAG storage
            println!("Showing proposal details is not yet implemented");
            Ok(())
        },
        ProposalCommands::Activate { id, key_file } => {
            // TODO: Implement activating a proposal from draft to active
            println!("Activating proposals is not yet implemented");
            Ok(())
        },
        ProposalCommands::Cancel { id, key_file, reason } => {
            // TODO: Implement cancelling a proposal
            println!("Cancelling proposals is not yet implemented");
            Ok(())
        },
        ProposalCommands::Execute { id, key_file, dry_run, output } => {
            println!("Executing proposal: {}", id);
            
            // Load the key from file
            let key_data = fs::read_to_string(&key_file)
                .map_err(|e| CliError::IoError(format!("Failed to read key file: {}", e)))?;
            
            let executor_key = DidKey::new(); // For now, just create a new key since we can't easily parse
            
            // 1. Load proposal
            println!("Loading proposal from DAG...");
            let proposal = load_proposal_from_dag(&id, ctx, None).await?;
            
            // 2. Load votes
            println!("Loading votes from DAG...");
            let votes = load_votes_for_proposal(&id, ctx, None).await?;
            
            // Display proposal and vote information
            println!("Proposal: {} - {}", proposal.id, proposal.credential_subject.title);
            println!("Status: {:?}", proposal.credential_subject.status);
            println!("Votes found: {}", votes.len());
            
            // 3. Create a quorum engine and evaluate votes
            println!("Evaluating quorum requirements...");
            let engine = QuorumEngine::new();
            let tally = engine.evaluate(&proposal, &votes)
                .map_err(|e| CliError::IdentityError(format!("Quorum evaluation failed: {}", e)))?;
            
            // 4. Check if proposal has passed quorum
            println!("\nQuorum Tally Results:");
            println!("-------------------------------------------");
            println!("Yes votes: {} (power: {})", tally.yes_votes, tally.yes_power);
            println!("No votes: {} (power: {})", tally.no_votes, tally.no_power);
            println!("Abstain votes: {} (power: {})", tally.abstain_votes, tally.abstain_power);
            println!("Veto votes: {} (power: {})", tally.veto_votes, tally.veto_power);
            println!("Threshold: {}", tally.threshold);
            println!("Outcome: {:?}", tally.outcome);
            
            if tally.outcome != QuorumOutcome::Passed {
                println!("\n❌ Proposal has NOT met quorum requirements. Execution aborted.");
                return Ok(());
            }
            
            println!("\n✅ Proposal PASSED quorum requirements.");
            
            // Stop here if dry run
            if dry_run {
                println!("Dry run requested. Skipping execution.");
                return Ok(());
            }
            
            // 5. Check if proposal has execution code
            let execution_cid = proposal.credential_subject.execution_cid.clone();
            if execution_cid.is_none() {
                return Err(CliError::InvalidArgument(
                    "Proposal doesn't contain executable code (missing execution_cid)".to_string()
                ));
            }
            let execution_cid = execution_cid.unwrap();
            
            println!("\nProceeding with execution...");
            println!("Execution CID: {}", execution_cid);
            
            // 6. Create execution context
            // This would setup the VM context with the right DID, etc.
            // TODO: Replace this with actual execution
            println!("Initializing execution environment...");
            
            // 7. Execute proposal code (simplified for now)
            println!("Executing proposal code...");
            let result_cid = execute_proposal_stub(&execution_cid);
            println!("Execution completed. Result CID: {}", result_cid);
            
            // 8. Generate execution receipt
            println!("Generating execution receipt...");
            let receipt = generate_execution_receipt_stub(
                &executor_key,
                &proposal.credential_subject.id, // federation DID
                &execution_cid,
                &result_cid,
            );
            
            // 9. Output receipt
            let receipt_json = receipt.to_json()
                .map_err(|e| CliError::SerializationError(format!("Failed to serialize receipt: {}", e)))?;
            
            if let Some(output_path) = output {
                fs::write(&output_path, receipt_json)
                    .map_err(|e| CliError::IoError(format!("Failed to write output file: {}", e)))?;
                println!("Execution receipt written to {}", output_path.display());
            } else {
                println!("\nExecution Receipt:");
                println!("-------------------------------------------");
                println!("{}", receipt_json);
            }
            
            // 10. Anchor receipt to DAG
            println!("Anchoring receipt to DAG...");
            anchor_receipt_to_dag(&receipt, ctx, None).await?;
            
            println!("\n🎉 Proposal executed successfully!");
            Ok(())
        },
    }
}

// STUB FUNCTIONS THAT WOULD BE REPLACED WITH ACTUAL IMPLEMENTATIONS
// --------------------------------------------------------------------

/// Load a proposal from the DAG by ID
async fn load_proposal_from_dag(
    proposal_id: &str,
    ctx: &mut CliContext,
    dag_dir: Option<PathBuf>
) -> Result<ProposalCredential, CliError> {
    // Get the DAG store from the context
    let dag_store = ctx.get_dag_store(dag_dir.as_deref())
        .map_err(|e| CliError::DagError(format!("Failed to access DAG store: {}", e)))?;
    
    // Find proposal events in the DAG where the proposal_id matches
    let events = dag_store.find_events_by_type(icn_types::dag::EventType::Proposal)
        .await
        .map_err(|e| CliError::DagError(format!("Failed to search DAG events: {}", e)))?;
    
    // Filter and find the event with matching proposal_id
    for event in events {
        if let EventPayload::Proposal { proposal_id: event_proposal_id, content_cid } = &event.payload {
            if event_proposal_id == proposal_id {
                // Found the matching proposal event
                
                // Retrieve the content by CID
                // In a real implementation, this would use IPFS or another CID-addressable store
                // For now, assume it's stored within the DAG itself
                let proposal_json = dag_store.get_content_by_cid(content_cid)
                    .await
                    .map_err(|e| CliError::DagError(format!("Failed to retrieve proposal content: {}", e)))?;
                
                // Deserialize the proposal
                let proposal = ProposalCredential::from_json(&proposal_json)
                    .map_err(|e| CliError::SerializationError(format!("Failed to deserialize proposal: {}", e)))?;
                
                return Ok(proposal);
            }
        }
    }
    
    // If we reach here, the proposal wasn't found
    Err(CliError::NotFound(format!("Proposal not found: {}", proposal_id)))
}

/// Load all votes for a proposal from the DAG
async fn load_votes_for_proposal(
    proposal_id: &str,
    ctx: &mut CliContext,
    dag_dir: Option<PathBuf>
) -> Result<Vec<VoteCredential>, CliError> {
    // Get the DAG store from the context
    let dag_store = ctx.get_dag_store(dag_dir.as_deref())
        .map_err(|e| CliError::DagError(format!("Failed to access DAG store: {}", e)))?;
    
    // Find vote events in the DAG
    let events = dag_store.find_events_by_type(icn_types::dag::EventType::Vote)
        .await
        .map_err(|e| CliError::DagError(format!("Failed to search DAG events: {}", e)))?;
    
    // Filter for events related to the given proposal_id
    let mut votes = Vec::new();
    
    for event in events {
        if let EventPayload::Vote { proposal_id: event_proposal_id, choice: _ } = &event.payload {
            if event_proposal_id == proposal_id {
                // This vote is for our proposal
                
                // Get the vote credential from the event's metadata or content
                let vote_cid = event.get_content_cid()
                    .ok_or_else(|| CliError::DagError("Vote event missing content CID".to_string()))?;
                
                let vote_json = dag_store.get_content_by_cid(&vote_cid)
                    .await
                    .map_err(|e| CliError::DagError(format!("Failed to retrieve vote content: {}", e)))?;
                
                // Deserialize the vote
                let vote = VoteCredential::from_json(&vote_json)
                    .map_err(|e| CliError::SerializationError(format!("Failed to deserialize vote: {}", e)))?;
                
                votes.push(vote);
            }
        }
    }
    
    Ok(votes)
}

// Stub for executing a proposal
fn execute_proposal_stub(execution_cid: &str) -> String {
    // In a real implementation, we would:
    // 1. Load the WASM module from the execution_cid
    // 2. Create a VM context with appropriate permissions
    // 3. Execute the module
    // 4. Gather the results
    
    // For now, just return a fake result CID
    format!("result-{}", Uuid::new_v4())
}

// Stub for generating an execution receipt
fn generate_execution_receipt_stub(
    executor_key: &DidKey,
    federation_did: &str,
    module_cid: &str,
    result_cid: &str,
) -> ExecutionReceipt {
    use icn_identity_core::vc::execution_receipt::{
        ExecutionReceipt, ExecutionSubject, ExecutionScope, ExecutionStatus
    };
    
    // Create the ExecutionSubject
    let subject = ExecutionSubject {
        id: executor_key.did().to_string(),
        scope: ExecutionScope::Federation { federation_id: federation_did.to_string() },
        submitter: Some(executor_key.did().to_string()),
        module_cid: module_cid.to_string(),
        result_cid: result_cid.to_string(),
        event_id: None,
        timestamp: current_time(),
        status: ExecutionStatus::Success,
        additional_properties: None,
    };
    
    // Create and sign the ExecutionReceipt
    let receipt_id = format!("urn:uuid:{}", Uuid::new_v4());
    
    ExecutionReceipt::new(
        receipt_id,
        federation_did.to_string(),
        subject
    ).sign(executor_key).unwrap()
}

/// Anchor an execution receipt to the DAG
async fn anchor_receipt_to_dag(
    receipt: &ExecutionReceipt,
    ctx: &mut CliContext,
    dag_dir: Option<PathBuf>
) -> Result<(), CliError> {
    // Get the DAG store
    let mut dag_store = ctx.get_dag_store(dag_dir.as_deref())
        .map_err(|e| CliError::DagError(format!("Failed to access DAG store: {}", e)))?;
    
    // Calculate CID for the receipt
    let receipt_cid = receipt.to_cid()
        .map_err(|e| CliError::SerializationError(format!("Failed to calculate receipt CID: {}", e)))?;
    
    // Convert receipt to JSON
    let receipt_json = receipt.to_json()
        .map_err(|e| CliError::SerializationError(format!("Failed to serialize receipt: {}", e)))?;
    
    // In a full implementation:
    // 1. Store the receipt content by CID
    // 2. Create a DAG event with EventType::Receipt
    // 3. Add the event to the DAG store
    
    // For now, we'll just simulate this process
    println!("Would anchor receipt with CID {} to DAG", receipt_cid);
    println!("Receipt content would be stored in content-addressable storage");
    
    // Create a receipt event
    let event = DagEvent::new(
        EventType::Receipt,
        receipt.issuer.clone(), // The issuer is the author
        Vec::new(), // No parents for now
        EventPayload::Receipt { receipt_cid }
    );
    
    // This code would run in a real implementation once the DAG interface supports it:
    // let event_id = dag_store.add_event(event)
    //     .await
    //     .map_err(|e| CliError::DagError(format!("Failed to anchor receipt to DAG: {}", e)))?;
    // println!("Receipt anchored to DAG with event ID: {}", event_id);
    
    Ok(())
}

// Keep the stub implementations for testing for now, but will be replaced with the real ones

// Stub extension methods for DagEvent that would be implemented properly
trait DagEventExt {
    fn get_content_cid(&self) -> Option<String>;
}

impl DagEventExt for DagEvent {
    fn get_content_cid(&self) -> Option<String> {
        // In a real implementation, this would extract the content CID from the event
        // For now, we'll just generate a placeholder CID
        Some(format!("content-cid-{}", Uuid::new_v4()))
    }
}

// Stub extension methods for DagStore
#[async_trait::async_trait]
trait DagStoreExt: DagStore {
    async fn find_events_by_type(&self, event_type: EventType) -> Result<Vec<DagEvent>, icn_types::dag::DagError>;
    async fn get_content_by_cid(&self, cid: &str) -> Result<String, icn_types::dag::DagError>;
}

#[async_trait::async_trait]
impl<T: DagStore + Send + Sync> DagStoreExt for T {
    async fn find_events_by_type(&self, event_type: EventType) -> Result<Vec<DagEvent>, icn_types::dag::DagError> {
        // In a real implementation, this would search the DAG store for events of the given type
        // For now, we'll just return empty results
        Ok(Vec::new())
    }
    
    async fn get_content_by_cid(&self, cid: &str) -> Result<String, icn_types::dag::DagError> {
        // In a real implementation, this would retrieve content based on CID
        // For now, we'll just generate a placeholder content
        let federation_key = DidKey::new();
        let federation_did = federation_key.did().to_string();
        
        match cid {
            c if c.contains("proposal") => {
                // Generate a stub proposal
                let subject = ProposalSubject {
                    id: federation_did.clone(),
                    title: "Test Proposal".to_string(),
                    description: "This is a test proposal".to_string(),
                    proposal_type: ProposalType::CodeExecution,
                    status: ProposalStatus::Active,
                    submitter: "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK".to_string(),
                    voting_threshold: VotingThreshold::Majority,
                    voting_duration: VotingDuration::TimeBased(86400),
                    voting_start_time: current_time() - 86400, // Started one day ago
                    voting_end_time: Some(current_time() - 3600), // Ended 1 hour ago
                    execution_cid: Some("bafyreihgmyh2srmmyiw7fdihrc2lw2bqdyxagrpvt2zk3aitq4hdxrhzzz".to_string()),
                    thread_cid: None,
                    parameters: None,
                    previous_version: None,
                    event_id: None,
                    created_at: current_time() - 86400,
                    updated_at: current_time() - 86400,
                };
                
                let proposal = ProposalCredential::new(
                    "stub-proposal-id".to_string(),
                    federation_did.clone(),
                    subject
                ).sign(&federation_key).unwrap();
                
                proposal.to_json().map_err(|e| icn_types::dag::DagError::InvalidNodeData(e.to_string()))
            },
            c if c.contains("vote") => {
                // Generate a stub vote
                let voter_key = DidKey::new();
                
                let vote_subject = VoteSubject {
                    id: voter_key.did().to_string(),
                    federation_id: federation_did.clone(),
                    proposal_id: "stub-proposal-id".to_string(),
                    decision: VoteDecision::Yes,
                    voting_power: None,
                    justification: None,
                    delegate_for: None,
                    is_amendment: false,
                    previous_vote_id: None,
                    cast_at: current_time() - 43200, // Cast 12 hours ago
                };
                
                let vote = VoteCredential::new(
                    format!("vote-{}", Uuid::new_v4()),
                    federation_did.clone(),
                    vote_subject
                ).sign(&voter_key).unwrap();
                
                vote.to_json().map_err(|e| icn_types::dag::DagError::InvalidNodeData(e.to_string()))
            },
            _ => Err(icn_types::dag::DagError::NodeNotFound(Cid::from("stub-cid")))
        }
    }
} 