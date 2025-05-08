use crate::context::CliContext;
use crate::error::CliError;
use crate::commands::federation::bootstrap::FederationMetadata;

use icn_identity_core::trustbundle::TrustBundle;
use icn_types::dag::DagEvent;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write, Cursor};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use multihash::{Multihash, MultihashDigest, Code};
use cid::{Cid, Version};

/// Error types for export operations
#[derive(Error, Debug)]
pub enum ExportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IPLD error: {0}")]
    Ipld(String),
    
    #[error("CAR error: {0}")]
    Car(String),
    
    #[error("Federation directory not found: {0}")]
    DirectoryNotFound(String),
    
    #[error("Required federation file not found: {0}")]
    FileNotFound(String),
    
    #[error("Export error: {0}")]
    Export(String),
}

impl From<ExportError> for CliError {
    fn from(err: ExportError) -> Self {
        CliError::Other(Box::new(err))
    }
}

/// CAR archive header format
#[derive(Serialize, Deserialize)]
struct CarHeader {
    roots: Vec<String>,
    version: u64,
}

/// CAR archive block entry (CID + data)
#[derive(Serialize, Deserialize)]
struct CarBlock {
    cid: String,
    data: Vec<u8>,
}

/// Metadata for the federation export
#[derive(Serialize, Deserialize)]
struct ExportManifest {
    federation_name: String,
    federation_id: String,
    bundle_cid: String,
    genesis_event_cid: String,
    files: Vec<FileEntry>,
    timestamp: u64,
}

/// Entry for a file in the export manifest
#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    cid: String,
    size: u64,
    content_type: String,
}

/// Main function to run the export
pub async fn run_export(
    _context: &CliContext,
    federation_dir: &str,
    output: Option<&str>,
    include_keys: bool,
    include_paths: &[String],
) -> Result<(), ExportError> {
    let federation_path = Path::new(federation_dir);
    if !federation_path.exists() || !federation_path.is_dir() {
        return Err(ExportError::DirectoryNotFound(federation_dir.to_string()));
    }
    
    println!("Exporting federation from {}", federation_dir);
    
    // Step 1: Load federation metadata
    let metadata_path = federation_path.join("federation.toml");
    if !metadata_path.exists() {
        return Err(ExportError::FileNotFound(format!(
            "federation.toml not found in {}", federation_dir
        )));
    }
    
    let metadata = load_federation_metadata(&metadata_path)?;
    println!("Loaded federation metadata for: {}", metadata.name);
    
    // Step 2: Load the TrustBundle
    let bundle_path = federation_path.join("genesis_bundle.json");
    if !bundle_path.exists() {
        return Err(ExportError::FileNotFound(format!(
            "genesis_bundle.json not found in {}", federation_dir
        )));
    }
    
    let bundle = load_trust_bundle(&bundle_path)?;
    println!("Loaded TrustBundle with {} referenced events", bundle.referenced_events.len());
    
    // Step 3: Load the genesis event
    let event_path = federation_path.join("genesis_event.json");
    if !event_path.exists() {
        return Err(ExportError::FileNotFound(format!(
            "genesis_event.json not found in {}", federation_dir
        )));
    }
    
    let genesis_event = load_genesis_event(&event_path)?;
    println!("Loaded genesis event");
    
    // Step 4: Collect additional files to include
    let mut files_to_include = Vec::new();
    
    // Always include the core federation files
    files_to_include.push(metadata_path.clone());
    files_to_include.push(bundle_path.clone());
    files_to_include.push(event_path.clone());
    
    // Include keys if requested
    if include_keys {
        let keys_path = federation_path.join("federation_keys.json");
        if keys_path.exists() {
            files_to_include.push(keys_path);
            println!("Including federation keys in export");
        } else {
            println!("Warning: federation_keys.json not found, skipping");
        }
    }
    
    // Add additional included paths
    for path_str in include_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            // If it's a directory, add all files recursively
            if path.is_dir() {
                collect_files_recursively(&path, &mut files_to_include)?;
            } else {
                files_to_include.push(path);
            }
        } else {
            println!("Warning: included path not found: {}", path_str);
        }
    }
    
    println!("Total files to include: {}", files_to_include.len());
    
    // Step 5: Create the CAR archive
    let output_path = if let Some(out) = output {
        PathBuf::from(out)
    } else {
        PathBuf::from(format!("{}.car", metadata.name))
    };
    
    create_car_archive(
        &output_path,
        &metadata,
        &bundle,
        &genesis_event,
        &files_to_include,
    )?;
    
    println!("✅ Federation exported to CAR archive: {}", output_path.display());
    println!("   Federation name: {}", metadata.name);
    println!("   Federation DID: {}", metadata.did);
    println!("   Total files: {}", files_to_include.len());
    
    Ok(())
}

/// Load federation metadata from a file
fn load_federation_metadata(path: &Path) -> Result<FederationMetadata, ExportError> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let metadata: FederationMetadata = toml::from_str(&contents)
        .map_err(|e| ExportError::Export(format!("Failed to parse federation metadata: {}", e)))?;
        
    Ok(metadata)
}

/// Load a TrustBundle from file
fn load_trust_bundle(path: &Path) -> Result<TrustBundle, ExportError> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let bundle: TrustBundle = serde_json::from_str(&contents)?;
    Ok(bundle)
}

/// Load a genesis event from file
fn load_genesis_event(path: &Path) -> Result<DagEvent, ExportError> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let event: DagEvent = serde_json::from_str(&contents)?;
    Ok(event)
}

/// Recursively collect files from a directory
fn collect_files_recursively(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ExportError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            collect_files_recursively(&path, files)?;
        } else {
            files.push(path);
        }
    }
    
    Ok(())
}

/// Create a CID for a block of data
fn create_cid(data: &[u8]) -> Result<Cid, ExportError> {
    // Generate SHA-256 hash of the data
    let hash = Code::Sha2_256.digest(data);
    
    // Create a CID with dag-json codec (0x0129)
    Ok(Cid::new_v1(0x0129, hash))
}

/// Create a CAR archive from federation data
fn create_car_archive(
    output_path: &Path,
    metadata: &FederationMetadata,
    bundle: &TrustBundle,
    genesis_event: &DagEvent,
    files: &[PathBuf],
) -> Result<(), ExportError> {
    // Create the output file
    let mut output_file = File::create(output_path)?;
    
    // Generate CIDs for core components
    let metadata_json = serde_json::to_vec(metadata)?;
    let metadata_cid = create_cid(&metadata_json)?;
    
    let bundle_json = serde_json::to_vec(bundle)?;
    let bundle_cid = create_cid(&bundle_json)?;
    
    let event_json = serde_json::to_vec(genesis_event)?;
    let event_cid = create_cid(&event_json)?;
    
    // Create file entries for the manifest
    let mut file_entries = Vec::new();
    let mut blocks = Vec::new();
    
    // Add core components to blocks
    blocks.push((metadata_cid.to_string(), metadata_json.clone()));
    
    file_entries.push(FileEntry {
        path: "federation.toml".to_string(),
        cid: metadata_cid.to_string(),
        size: metadata_json.len() as u64,
        content_type: "application/toml".to_string(),
    });
    
    blocks.push((bundle_cid.to_string(), bundle_json.clone()));
    
    file_entries.push(FileEntry {
        path: "genesis_bundle.json".to_string(),
        cid: bundle_cid.to_string(),
        size: bundle_json.len() as u64,
        content_type: "application/json".to_string(),
    });
    
    blocks.push((event_cid.to_string(), event_json.clone()));
    
    file_entries.push(FileEntry {
        path: "genesis_event.json".to_string(),
        cid: event_cid.to_string(),
        size: event_json.len() as u64,
        content_type: "application/json".to_string(),
    });
    
    // Process all additional files
    for file_path in files {
        // Skip core files we've already processed
        if file_path.file_name().unwrap_or_default() == "federation.toml" ||
           file_path.file_name().unwrap_or_default() == "genesis_bundle.json" ||
           file_path.file_name().unwrap_or_default() == "genesis_event.json" {
            continue;
        }
        
        let file_data = match fs::read(file_path) {
            Ok(data) => data,
            Err(e) => {
                println!("Warning: Failed to read file {}: {}", file_path.display(), e);
                continue;
            }
        };
        
        let file_cid = create_cid(&file_data)?;
        
        // Add to blocks
        blocks.push((file_cid.to_string(), file_data.clone()));
        
        // Add to file entries
        file_entries.push(FileEntry {
            path: file_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            cid: file_cid.to_string(),
            size: file_data.len() as u64,
            content_type: guess_content_type(file_path),
        });
    }
    
    // Create the manifest
    let manifest = ExportManifest {
        federation_name: metadata.name.clone(),
        federation_id: metadata.did.clone(),
        bundle_cid: bundle_cid.to_string(),
        genesis_event_cid: event_cid.to_string(),
        files: file_entries,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    
    let manifest_json = serde_json::to_vec(&manifest)?;
    let manifest_cid = create_cid(&manifest_json)?;
    
    // Add manifest to blocks
    blocks.push((manifest_cid.to_string(), manifest_json));
    
    // Write CAR header (with manifest CID as root)
    let header = CarHeader {
        roots: vec![manifest_cid.to_string()],
        version: 1,
    };
    
    let header_bytes = serde_json::to_vec(&header)?;
    
    // CAR format: 
    // - varint header length
    // - header
    // - blocks (each with varint length, CID, data)
    
    // Write the header length as a varint
    write_unsigned_varint(&mut output_file, header_bytes.len() as u64)?;
    
    // Write the header
    output_file.write_all(&header_bytes)?;
    
    // Write each block
    for (cid_str, data) in blocks {
        // Convert CID string to binary
        let cid = Cid::from_str(&cid_str)
            .map_err(|e| ExportError::Ipld(format!("Invalid CID: {}", e)))?;
            
        let cid_bytes = cid.to_bytes();
        
        // Calculate and write block length (CID length + data length)
        let block_length = cid_bytes.len() + data.len();
        write_unsigned_varint(&mut output_file, block_length as u64)?;
        
        // Write CID
        output_file.write_all(&cid_bytes)?;
        
        // Write data
        output_file.write_all(&data)?;
    }
    
    Ok(())
}

/// Write an unsigned varint to the given writer
fn write_unsigned_varint<W: Write>(writer: &mut W, mut value: u64) -> Result<(), ExportError> {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        
        if value != 0 {
            byte |= 0x80;
        }
        
        writer.write_all(&[byte])?;
        
        if value == 0 {
            break;
        }
    }
    
    Ok(())
}

/// Guess the content type based on file extension
fn guess_content_type(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json".to_string(),
        Some("toml") => "application/toml".to_string(),
        Some("txt") => "text/plain".to_string(),
        Some("md") => "text/markdown".to_string(),
        Some("car") => "application/vnd.ipld.car".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("gif") => "image/gif".to_string(),
        _ => "application/octet-stream".to_string(),
    }
} 