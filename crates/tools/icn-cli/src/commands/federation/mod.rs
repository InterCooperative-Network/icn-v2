use crate::context::CliContext;
use crate::error::CliError;

pub mod bootstrap;

#[derive(clap::Subcommand, Debug)]
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
        FederationCommands::Verify { bundle_path } => {
            println!("Verifying federation bundle at {}", bundle_path);
            todo!("Implement federation bundle verification");
        }
    }
    
    Ok(())
} 