use crate::context::CliContext;
use crate::error::CliError;
use std::path::PathBuf;
use clap::ValueHint;
use clap::{Args, Subcommand};

pub mod bootstrap;
pub mod verify;
pub mod export;
pub mod import;
pub mod proposal;

#[derive(Subcommand, Debug, Clone)]
pub enum FederationCommands {
    /// Bootstrap a new federation with a genesis TrustBundle
    Init {
        /// Name of the federation
        #[clap(long)]
        name: String,
        
        /// Directory to output federation files
        #[clap(long)]
        output_dir: Option<String>,
        
        /// Run in dry-run mode without writing files
        #[clap(long)]
        dry_run: bool,
        
        /// Paths to participant key files (JSON format)
        /// If not provided, a single federation key will be generated
        #[clap(long = "participant", value_name = "KEY_FILE")]
        participants: Vec<String>,
        
        /// Quorum type to use for federation governance
        /// Valid values: all, majority, threshold:<num> (e.g., threshold:67 for 67%)
        #[clap(long, default_value = "all")]
        quorum: String,
        
        /// Export the federation keys to a file
        #[clap(long, default_value = "true")]
        export_keys: bool,
        
        /// Key format for exported keys (jwk or base58)
        #[clap(long, default_value = "jwk")]
        key_format: String,
    },
    
    /// Verify a federation TrustBundle
    Verify {
        /// Path to the federation bundle file
        #[clap(long)]
        bundle_path: String,
        
        /// Path to the referenced event(s) for verification
        /// If not provided, will try to find events in the same directory
        #[clap(long)]
        events_path: Option<String>,
        
        /// Directory containing participant key files for verification
        /// If not provided, will try to find keys in the same directory
        #[clap(long)]
        keys_dir: Option<String>,
        
        /// Print detailed verification information
        #[clap(long, default_value = "false")]
        verbose: bool,
    },
    
    /// Export a federation to a CAR archive for cold-sync
    Export {
        /// Path to the federation directory
        #[clap(long)]
        federation_dir: String,
        
        /// Output path for the CAR archive
        /// If not provided, will use <federation_name>.car in the current directory
        #[clap(long)]
        output: Option<String>,
        
        /// Include keys in the export (warning: contains private keys)
        #[clap(long, default_value = "false")]
        include_keys: bool,
        
        /// Include additional files or directories in the export
        #[clap(long = "include", value_name = "PATH")]
        include_paths: Vec<String>,
    },

    /// Import a federation from a CAR archive
    Import {
        /// Path to the CAR archive file
        #[clap(long)]
        archive_path: String,

        /// Directory to output the imported federation files
        #[clap(long)]
        output_dir: Option<String>,

        /// Perform verification only without writing files
        #[clap(long, default_value = "false")]
        verify_only: bool,

        /// Override existing federation with the same name
        #[clap(long, default_value = "false")]
        override_existing: bool,
        
        /// Skip importing federation keys
        #[clap(long, default_value = "false")]
        no_keys: bool,
    },
    
    /// Submit a new proposal to a federation
    #[clap(name = "submit-proposal")]
    SubmitProposal {
        /// File containing the proposal in TOML format
        #[clap(long, value_hint = ValueHint::FilePath)]
        file: PathBuf,
        
        /// Federation node URL to submit the proposal to
        #[clap(long)]
        to: String,
        
        /// Path to the key file for signing the proposal
        #[clap(long, value_hint = ValueHint::FilePath)]
        key: Option<PathBuf>,
        
        /// Output file to save the proposal details
        #[clap(long, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },
    
    /// Vote on an existing federation proposal
    #[clap(name = "vote")]
    Vote {
        /// ID of the proposal to vote on
        #[clap(long)]
        proposal_id: String,
        
        /// Vote decision (approve/reject)
        #[clap(long, default_value = "approve")]
        decision: String,
        
        /// Reason for the vote
        #[clap(long)]
        reason: Option<String>,
        
        /// Path to the key file for signing the vote
        #[clap(long, value_hint = ValueHint::FilePath)]
        key: Option<PathBuf>,
        
        /// Federation node URL to submit the vote to
        #[clap(long)]
        to: Option<String>,
    },
    
    /// Execute an approved proposal
    #[clap(name = "execute")]
    Execute {
        /// ID of the proposal to execute
        #[clap(long)]
        proposal_id: String,
        
        /// Path to the key file for signing the execution
        #[clap(long, value_hint = ValueHint::FilePath)]
        key: Option<PathBuf>,
        
        /// Federation node URL to execute the proposal on
        #[clap(long)]
        to: Option<String>,
        
        /// Output file to save the execution receipt
        #[clap(long, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },
}

pub async fn handle_federation_command(
    context: &mut CliContext,
    cmd: &FederationCommands,
) -> Result<(), CliError> {
    match cmd {
        FederationCommands::Init { 
            name, 
            output_dir, 
            dry_run,
            participants,
            quorum,
            export_keys,
            key_format,
        } => {
            bootstrap::run_init(
                context, 
                name, 
                output_dir.as_deref(), 
                *dry_run,
                participants,
                quorum,
                *export_keys,
                key_format,
            ).await?;
        }
        FederationCommands::Verify { 
            bundle_path,
            events_path,
            keys_dir,
            verbose,
        } => {
            verify::run_verify(
                context,
                bundle_path,
                events_path.as_deref(),
                keys_dir.as_deref(),
                *verbose,
            ).await?;
        }
        FederationCommands::Export {
            federation_dir,
            output,
            include_keys,
            include_paths,
        } => {
            export::run_export(
                context,
                federation_dir,
                output.as_deref(),
                *include_keys,
                include_paths,
            ).await?;
        }
        FederationCommands::Import {
            archive_path,
            output_dir,
            verify_only,
            override_existing,
            no_keys,
        } => {
            import::run_import(
                context,
                archive_path,
                output_dir.as_deref(),
                *verify_only,
                *override_existing,
                *no_keys,
            ).await?;
        }
        FederationCommands::SubmitProposal {
            file,
            to,
            key,
            output,
        } => {
            proposal::submit_proposal(
                context,
                file,
                to,
                key.as_deref(),
                output.as_deref(),
            ).await?;
        }
        FederationCommands::Vote {
            proposal_id,
            decision,
            reason,
            key,
            to,
        } => {
            proposal::vote_on_proposal(
                context,
                proposal_id,
                decision,
                reason.as_deref(),
                key.as_deref(),
                to.as_deref(),
            ).await?;
        }
        FederationCommands::Execute {
            proposal_id,
            key,
            to,
            output,
        } => {
            proposal::execute_proposal(
                context,
                proposal_id,
                key.as_deref(),
                to.as_deref(),
                output.as_deref(),
            ).await?;
        }
    }
    
    Ok(())
} 