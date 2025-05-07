use clap::{Args, Subcommand};
use crate::{CliContext, error::{CliError, CliResult}};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};

/// Key generation and management for Decentralized Identifiers (DIDs)
#[derive(Subcommand, Debug, Clone)]
pub enum KeygenCommands {
    /// Generate a new DID key
    Generate(GenerateKeyArgs),
    
    /// Import an existing DID key
    Import(ImportKeyArgs),
    
    /// Display information about a DID key
    Info(KeyInfoArgs),
}

#[derive(Args, Debug, Clone)]
pub struct GenerateKeyArgs {
    /// Output file to save the key (defaults to ~/.icn/key.json)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    
    /// Key type (ed25519, secp256k1, etc.)
    #[arg(long, default_value = "ed25519")]
    pub key_type: String,
    
    /// Force overwrite if file exists
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ImportKeyArgs {
    /// Path to key file to import
    #[arg(long)]
    pub file: PathBuf,
    
    /// Output file (defaults to ~/.icn/key.json)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    
    /// Force overwrite if file exists
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct KeyInfoArgs {
    /// Path to key file (defaults to ~/.icn/key.json)
    #[arg(long)]
    pub file: Option<PathBuf>,
}

/// Handle key generation commands
pub async fn handle_keygen_command(context: &mut CliContext, cmd: &KeygenCommands) -> CliResult {
    if context.verbose { println!("Handling Keygen command: {:?}", cmd); }
    
    match cmd {
        KeygenCommands::Generate(args) => handle_generate_key(context, args).await,
        KeygenCommands::Import(args) => handle_import_key(context, args).await,
        KeygenCommands::Info(args) => handle_key_info(context, args).await,
    }
}

/// Handle for top-level key-gen command
pub async fn handle_key_gen(context: &mut CliContext, output: &Option<PathBuf>) -> CliResult {
    // Convert the simple output path to GenerateKeyArgs
    let args = GenerateKeyArgs {
        output: output.clone(),
        key_type: "ed25519".to_string(),
        force: false,
    };
    
    handle_generate_key(context, &args).await
}

async fn handle_generate_key(context: &mut CliContext, args: &GenerateKeyArgs) -> CliResult {
    let output_path = get_output_path(&args.output)?;
    
    // Check if file exists and handle force flag
    if output_path.exists() && !args.force {
        return Err(CliError::IoError(format!(
            "Output file '{}' already exists. Use --force to overwrite.", 
            output_path.display()
        )));
    }
    
    if context.verbose {
        println!("Generating {} key and saving to: {}", args.key_type, output_path.display());
    } else {
        println!("Generating key: {}", output_path.display());
    }
    
    // TODO: Generate the actual key based on key_type
    // For now, create a placeholder JSON structure
    let did_key_json = format!(r#"{{
  "id": "did:icn:example",
  "type": "{}",
  "created": "{}",
  "privateKeyBase64": "PLACEHOLDER_FOR_PRIVATE_KEY",
  "publicKeyBase64": "PLACEHOLDER_FOR_PUBLIC_KEY"
}}"#, args.key_type, chrono::Utc::now().to_rfc3339());
    
    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Write to file
    fs::write(&output_path, did_key_json)?;
    
    println!("DID key successfully generated and written to: {}", output_path.display());
    println!("DID: did:icn:example");
    
    Err(CliError::Unimplemented("Actual key generation not implemented yet".to_string()))
}

async fn handle_import_key(context: &mut CliContext, args: &ImportKeyArgs) -> CliResult {
    if !args.file.exists() {
        return Err(CliError::IoError(format!("Input file not found: {}", args.file.display())));
    }
    
    let output_path = get_output_path(&args.output)?;
    
    if output_path.exists() && !args.force {
        return Err(CliError::IoError(format!(
            "Output file '{}' already exists. Use --force to overwrite.", 
            output_path.display()
        )));
    }
    
    println!("Importing key from {} to {}", args.file.display(), output_path.display());
    
    // TODO: Validate the key file format and import properly
    // For now, just copy the file
    if context.verbose {
        println!("Copying key file content");
    }
    
    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Copy the file
    fs::copy(&args.file, &output_path)?;
    
    println!("Key imported successfully");
    
    Err(CliError::Unimplemented("Complete key import validation not implemented".to_string()))
}

async fn handle_key_info(context: &mut CliContext, args: &KeyInfoArgs) -> CliResult {
    let key_path = match &args.file {
        Some(p) => p.clone(),
        None => get_default_key_path()?,
    };
    
    if !key_path.exists() {
        return Err(CliError::IoError(format!("Key file not found: {}", key_path.display())));
    }
    
    println!("Reading key from: {}", key_path.display());
    
    // Read the key file
    let key_content = fs::read_to_string(&key_path)?;
    
    // TODO: Parse the JSON and display relevant information
    // For now, just print the raw content
    println!("Key content (raw):");
    println!("{}", key_content);
    
    Err(CliError::Unimplemented("Proper key info display not implemented".to_string()))
}

// Helper to get default key path
fn get_default_key_path() -> Result<PathBuf, CliError> {
    let home = dirs::home_dir()
        .ok_or_else(|| CliError::IoError("Unable to determine home directory".to_string()))?;
    
    Ok(home.join(".icn").join("key.json"))
}

// Helper to resolve output path
fn get_output_path(output: &Option<PathBuf>) -> Result<PathBuf, CliError> {
    match output {
        Some(p) => Ok(p.clone()),
        None => get_default_key_path(),
    }
} 