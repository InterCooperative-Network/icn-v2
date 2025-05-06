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
    }
};
use icn_types::dag::{Cid, DagStore, EventId};
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use serde_json::{json, Value};
use colored::Colorize;

/// CLI commands for managing federation proposals
#[derive(Subcommand, Debug)]
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
            
            let submitter_key = DidKey::from_jwk(&key_data)
                .map_err(|e| CliError::IdentityError(format!("Failed to parse key: {}", e)))?;
            
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
    }
} 