use crate::context::CliContext;
use crate::error::CliError;
use crate::commands::federation::bootstrap::FederationMetadata;

use icn_identity_core::trustbundle::TrustBundle;
use icn_types::dag::DagEvent;
use std::collections::HashMap;
use std::fs::{self, File, create_dir_all};
use std::io::{Read, Write, Cursor, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use multihash::{Multihash, MultihashDigest, Code};
use cid::{Cid, Version};

/// Error types for import operations
#[derive(Error, Debug)]
pub enum ImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IPLD error: {0}")]
    Ipld(String),
    
    #[error("CAR error: {0}")]
    Car(String),
    
    #[error("Federation with the same name already exists: {0}")]
    FederationExists(String),
    
    #[error("Required file not found in archive: {0}")]
    FileNotFound(String),
    
    #[error("Failed to verify archive integrity: {0}")]
    VerificationFailed(String),
    
    #[error("Import error: {0}")]
    Import(String),
}

impl From<ImportError> for CliError {
    fn from(err: ImportError) -> Self {
        CliError::Other(Box::new(err))
    }
}

/// CAR archive header format
#[derive(Serialize, Deserialize)]
struct CarHeader {
    roots: Vec<String>,
    version: u64,
}

/// Entry for a file in the export manifest
#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    cid: String,
    size: u64,
    content_type: String,
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

/// Main function to run the import operation
pub async fn run_import(
    _context: &CliContext,
    archive_path: &str,
    output_dir: Option<&str>,
    verify_only: bool,
    override_existing: bool,
    no_keys: bool,
) -> Result<(), ImportError> {
    let archive_path = Path::new(archive_path);
    if !archive_path.exists() {
        return Err(ImportError::FileNotFound(format!(
            "Archive file not found: {}", archive_path.display()
        )));
    }
    
    println!("Importing federation from {}", archive_path.display());
    
    // Step 1: Parse the CAR archive
    let (manifest, blocks) = parse_car_archive(archive_path)?;
    
    println!("Found federation: {}", manifest.federation_name);
    println!("Federation ID: {}", manifest.federation_id);
    println!("Total files: {}", manifest.files.len());
    
    // Step 2: Validate archive integrity
    validate_archive(&manifest, &blocks)?;
    
    // If verify-only flag is set, we're done
    if verify_only {
        println!("✅ Archive verification successful");
        println!("   Federation name: {}", manifest.federation_name);
        println!("   Federation ID: {}", manifest.federation_id);
        println!("   Files: {}", manifest.files.len());
        return Ok(());
    }
    
    // Step 3: Determine output directory
    let output_dir = if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        PathBuf::from(&manifest.federation_name)
    };
    
    // Check if federation already exists
    if output_dir.exists() && !override_existing {
        return Err(ImportError::FederationExists(format!(
            "Directory already exists: {}. Use --override-existing to force", 
            output_dir.display()
        )));
    }
    
    // Create output directory if it doesn't exist
    create_dir_all(&output_dir)?;
    
    // Step 4: Extract files from archive
    extract_files(&manifest, &blocks, &output_dir, no_keys)?;
    
    println!("✅ Federation imported successfully to {}", output_dir.display());
    println!("   Federation name: {}", manifest.federation_name);
    println!("   Federation ID: {}", manifest.federation_id);
    
    Ok(())
}

/// Parse a CAR archive file and extract blocks
fn parse_car_archive(path: &Path) -> Result<(ExportManifest, HashMap<String, Vec<u8>>), ImportError> {
    let mut file = File::open(path)?;
    let mut blocks = HashMap::new();
    
    // Read varint for header length
    let header_length = read_unsigned_varint(&mut file)?;
    
    // Read header
    let mut header_bytes = vec![0u8; header_length as usize];
    file.read_exact(&mut header_bytes)?;
    
    let header: CarHeader = serde_json::from_slice(&header_bytes)
        .map_err(|e| ImportError::Car(format!("Failed to parse CAR header: {}", e)))?;
    
    if header.version != 1 {
        return Err(ImportError::Car(format!("Unsupported CAR version: {}", header.version)));
    }
    
    if header.roots.is_empty() {
        return Err(ImportError::Car("CAR archive has no roots".to_string()));
    }
    
    let manifest_cid = &header.roots[0];
    let mut manifest_data = None;
    
    // Read blocks
    while let Ok(block_length) = read_unsigned_varint(&mut file) {
        if block_length == 0 {
            break;
        }
        
        // Read CID
        let mut cid_prefix = [0u8; 2]; // First bytes of CID to determine version and length
        file.read_exact(&mut cid_prefix)?;
        
        // Reset position to read the full CID
        file.seek(SeekFrom::Current(-2))?;
        
        let cid_version = match cid_prefix[0] {
            0x12 => Version::V1,
            0x01 => Version::V0,
            _ => return Err(ImportError::Car(format!("Unsupported CID version prefix: {:02x}", cid_prefix[0])))
        };
        
        // Estimate CID length based on version
        let cid_length = match cid_version {
            Version::V0 => 34, // Fixed length for CIDv0
            Version::V1 => {
                // For CIDv1, we need to parse more of the structure
                // Format: version (1 byte) + codec (varint) + hash type (varint) + hash length (varint) + hash
                // We'll read a larger buffer and then parse the CID from it
                let mut cid_buffer = vec![0u8; 64]; // Large enough for most CIDs
                let read_len = file.read(&mut cid_buffer)?;
                cid_buffer.truncate(read_len);
                
                // Try to parse the CID to get its length
                let cid = Cid::try_from(cid_buffer.as_slice())
                    .map_err(|e| ImportError::Car(format!("Failed to parse CID: {}", e)))?;
                    
                let cid_bytes = cid.to_bytes();
                cid_bytes.len()
            }
        };
        
        // Reset position and read the exact CID
        file.seek(SeekFrom::Current(-(cid_length as i64)))?;
        
        let mut cid_bytes = vec![0u8; cid_length];
        file.read_exact(&mut cid_bytes)?;
        
        // Parse the CID
        let cid = Cid::try_from(cid_bytes.as_slice())
            .map_err(|e| ImportError::Car(format!("Failed to parse CID: {}", e)))?;
            
        // Read the data
        let data_length = block_length as usize - cid_length;
        let mut data = vec![0u8; data_length];
        file.read_exact(&mut data)?;
        
        // Store the block
        let cid_str = cid.to_string();
        blocks.insert(cid_str.clone(), data.clone());
        
        // Check if this is the manifest
        if cid_str == *manifest_cid {
            manifest_data = Some(data);
        }
    }
    
    // Parse manifest
    let manifest_data = manifest_data.ok_or_else(|| 
        ImportError::FileNotFound(format!("Manifest not found with CID: {}", manifest_cid))
    )?;
    
    let manifest: ExportManifest = serde_json::from_slice(&manifest_data)?;
    
    Ok((manifest, blocks))
}

/// Validate the integrity of the archive
fn validate_archive(
    manifest: &ExportManifest, 
    blocks: &HashMap<String, Vec<u8>>
) -> Result<(), ImportError> {
    // Check if all required files are present
    let mut required_cids = vec![
        &manifest.bundle_cid,
        &manifest.genesis_event_cid,
    ];
    
    // Check all files listed in manifest
    for file_entry in &manifest.files {
        required_cids.push(&file_entry.cid);
    }
    
    // Verify that all required CIDs are in the blocks
    for cid in required_cids {
        if !blocks.contains_key(cid) {
            return Err(ImportError::FileNotFound(format!(
                "Required file with CID {} not found in archive", cid
            )));
        }
    }
    
    // Verify bundle content integrity
    let bundle_data = blocks.get(&manifest.bundle_cid)
        .ok_or_else(|| ImportError::FileNotFound(format!(
            "TrustBundle with CID {} not found", manifest.bundle_cid
        )))?;
    
    let bundle: TrustBundle = serde_json::from_slice(bundle_data)
        .map_err(|e| ImportError::Serialization(e))?;
        
    if bundle.federation_id != manifest.federation_id {
        return Err(ImportError::VerificationFailed(format!(
            "Federation ID mismatch: {} in manifest, {} in TrustBundle", 
            manifest.federation_id, bundle.federation_id
        )));
    }
    
    // Verify genesis event content
    let event_data = blocks.get(&manifest.genesis_event_cid)
        .ok_or_else(|| ImportError::FileNotFound(format!(
            "Genesis event with CID {} not found", manifest.genesis_event_cid
        )))?;
        
    let _event: DagEvent = serde_json::from_slice(event_data)
        .map_err(|e| ImportError::Serialization(e))?;
    
    // Basic validation passed
    println!("✓ Archive integrity verified");
    
    Ok(())
}

/// Extract files from the archive to the output directory
fn extract_files(
    manifest: &ExportManifest,
    blocks: &HashMap<String, Vec<u8>>,
    output_dir: &Path,
    no_keys: bool,
) -> Result<(), ImportError> {
    for file_entry in &manifest.files {
        // Skip keys if no_keys flag is set
        if no_keys && file_entry.path.contains("keys") {
            println!("Skipping keys file: {}", file_entry.path);
            continue;
        }
        
        let data = blocks.get(&file_entry.cid)
            .ok_or_else(|| ImportError::FileNotFound(format!(
                "File with CID {} not found", file_entry.cid
            )))?;
            
        let output_path = output_dir.join(&file_entry.path);
        
        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            create_dir_all(parent)?;
        }
        
        // Write the file
        let mut file = File::create(&output_path)?;
        file.write_all(data)?;
        
        println!("✓ Extracted: {}", file_entry.path);
    }
    
    // Also extract federation.toml if it's not already in the files list
    let metadata_entry = manifest.files.iter()
        .find(|e| e.path == "federation.toml");
        
    if metadata_entry.is_none() {
        // Find federation.toml by content type
        for (cid, data) in blocks {
            // Try to parse as TOML to see if it's the federation metadata
            if let Ok(metadata_str) = String::from_utf8(data.clone()) {
                if metadata_str.contains("name") && metadata_str.contains("did") {
                    if let Ok(_) = toml::from_str::<FederationMetadata>(&metadata_str) {
                        // Write the file
                        let output_path = output_dir.join("federation.toml");
                        let mut file = File::create(&output_path)?;
                        file.write_all(data)?;
                        
                        println!("✓ Extracted (inferred): federation.toml");
                        break;
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Read an unsigned varint from the given reader
fn read_unsigned_varint<R: Read>(reader: &mut R) -> Result<u64, ImportError> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut buffer = [0u8; 1];
    
    loop {
        if reader.read_exact(&mut buffer).is_err() {
            // End of file
            if value == 0 && shift == 0 {
                return Err(ImportError::Car("Unexpected end of file".to_string()));
            }
            break;
        }
        
        let byte = buffer[0];
        value |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        
        if byte & 0x80 == 0 {
            break;
        }
        
        if shift > 63 {
            return Err(ImportError::Car("Varint overflow".to_string()));
        }
    }
    
    Ok(value)
} 