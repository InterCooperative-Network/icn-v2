use clap::{Args, Subcommand};
use crate::{CliContext, error::{CliError, CliResult}};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use rand::rngs::OsRng;
use chrono::Utc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Serialize, Deserialize};

/// Key file format that matches the expected format in the rest of the system
#[derive(Serialize, Deserialize)]
struct KeyFile {
    did: String,
    #[serde(rename = "privateKey")]
    private_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<String>,
}

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
    
    // Support only ed25519 for now
    if args.key_type != "ed25519" {
        return Err(CliError::Config(format!("Unsupported key type: {}. Only ed25519 is supported.", args.key_type)));
    }
    
    // Generate actual key
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    
    // Create a DID from the public key (similar to what's done in the icn-identity-core)
    let pubkey_bytes = verifying_key.as_bytes();
    let mut did_string = String::from("did:icn:");
    did_string.push_str(&hex::encode(&pubkey_bytes[0..8]));
    
    // Format the private key consistently
    let private_key_string = format!("ed25519-priv:{}", BASE64_STANDARD.encode(signing_key.as_bytes()));
    
    // Create the JSON key file
    let key_file = KeyFile {
        did: did_string.clone(),
        private_key: private_key_string,
        created: Some(Utc::now().to_rfc3339()),
    };
    
    let key_json = serde_json::to_string_pretty(&key_file)
        .map_err(|e| CliError::IoError(format!("Failed to serialize key: {}", e)))?;
    
    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Write to file
    fs::write(&output_path, key_json)?;
    
    println!("DID key successfully generated and written to: {}", output_path.display());
    println!("DID: {}", did_string);
    
    Ok(())
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
    
    // Read and validate the key file format
    let key_content = fs::read_to_string(&args.file)?;
    let key_file: KeyFile = serde_json::from_str(&key_content)
        .map_err(|e| CliError::IoError(format!("Invalid key file format: {}", e)))?;
    
    // Ensure DID is valid
    if !key_file.did.starts_with("did:icn:") {
        return Err(CliError::IoError("Invalid DID format in key file".to_string()));
    }
    
    // Ensure private key is valid
    if !key_file.private_key.starts_with("ed25519-priv:") {
        return Err(CliError::IoError("Invalid private key format in key file".to_string()));
    }
    
    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Copy the file
    fs::write(&output_path, key_content)?;
    
    println!("Key imported successfully");
    println!("DID: {}", key_file.did);
    
    Ok(())
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
    
    // Read and parse the key file
    let key_content = fs::read_to_string(&key_path)?;
    let key_file: KeyFile = serde_json::from_str(&key_content)
        .map_err(|e| CliError::IoError(format!("Invalid key file format: {}", e)))?;
    
    // Display key information
    println!("DID: {}", key_file.did);
    if let Some(created) = key_file.created {
        println!("Created: {}", created);
    }
    println!("Key type: ed25519");
    
    Ok(())
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