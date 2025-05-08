use crate::context::CliContext;
use crate::error::CliError;

use chrono::{DateTime, Utc};
use icn_core_types::Did;
use icn_identity_core::trustbundle::{
    TrustBundle, QuorumConfig, QuorumType,
};
use icn_identity_core::{TrustBundleStore, MemoryTrustBundleStore, StorageError};
use icn_identity_core::trustbundle::storage::StoredTrustBundle;
#[cfg(feature = "persistence")]
use icn_identity_core::trustbundle::RocksDbTrustBundleStore;
use icn_types::dag::{DagEvent, EventType, EventPayload, merkle::calculate_event_hash};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
use std::str::FromStr;

/// Errors that can occur during federation bootstrap
#[derive(Error, Debug)]
pub enum BootstrapError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("TrustBundle error: {0}")]
    TrustBundle(#[from] StorageError),
    
    #[error("Federation with name '{0}' already exists")]
    FederationExists(String),
    
    #[error("Invalid quorum specification: {0}")]
    InvalidQuorum(String),
    
    #[error("Key error: {0}")]
    KeyError(String),
    
    #[error("Failed to create federation: {0}")]
    Creation(String),
}

impl From<BootstrapError> for CliError {
    fn from(err: BootstrapError) -> Self {
        CliError::Other(Box::new(err))
    }
}

/// Metadata about a federation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMetadata {
    /// Name of the federation
    pub name: String,
    
    /// Federation DID
    pub did: String,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Guardian DIDs (quorum participants)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub guardians: Vec<String>,
    
    /// Quorum configuration 
    pub quorum_config: String,
    
    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Representation of a key pair for a federation participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantKey {
    /// The DID of the participant
    pub did: String,
    
    /// Private key (JWK or encoded format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<serde_json::Value>,
    
    /// Public key (JWK or encoded format)
    pub public_key: serde_json::Value,
    
    /// Key format (jwk or base58)
    pub format: String,
    
    /// Optional additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ParticipantKey {
    /// Create a new ParticipantKey from a signing key
    pub fn new(signing_key: &SigningKey, format: &str) -> Result<Self, BootstrapError> {
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_bytes();
        
        // Generate DID
        let encoded = multibase::encode(multibase::Base::Base58Btc, pubkey_bytes);
        let did = format!("did:key:{}", encoded);
        
        // Format keys according to requested format
        let (private_key, public_key) = match format {
            "jwk" => {
                // JWK format
                let private_jwk = serde_json::json!({
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": base64::encode(pubkey_bytes),
                    "d": base64::encode(signing_key.to_bytes()),
                });
                
                let public_jwk = serde_json::json!({
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": base64::encode(pubkey_bytes),
                });
                
                (Some(private_jwk), public_jwk)
            },
            "base58" => {
                // Base58 format
                let private_b58 = serde_json::json!(multibase::encode(multibase::Base::Base58Btc, signing_key.to_bytes()));
                let public_b58 = serde_json::json!(multibase::encode(multibase::Base::Base58Btc, pubkey_bytes));
                
                (Some(private_b58), public_b58)
            },
            _ => return Err(BootstrapError::KeyError(format!("Unsupported key format: {}", format))),
        };
        
        Ok(ParticipantKey {
            did,
            private_key,
            public_key,
            format: format.to_string(),
            metadata: HashMap::new(),
        })
    }
    
    /// Load a participant key from a file
    pub fn from_file(path: &Path) -> Result<Self, BootstrapError> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let key: ParticipantKey = serde_json::from_str(&contents)
            .map_err(|e| BootstrapError::KeyError(format!("Failed to parse key file: {}", e)))?;
            
        Ok(key)
    }
    
    /// Get signing key from participant key
    pub fn to_signing_key(&self) -> Result<SigningKey, BootstrapError> {
        if self.private_key.is_none() {
            return Err(BootstrapError::KeyError("No private key available".to_string()));
        }
        
        match self.format.as_str() {
            "jwk" => {
                let jwk = self.private_key.as_ref().unwrap();
                let d = jwk["d"].as_str()
                    .ok_or_else(|| BootstrapError::KeyError("Invalid JWK format: missing 'd' field".to_string()))?;
                
                let decoded = base64::decode(d)
                    .map_err(|e| BootstrapError::KeyError(format!("Failed to decode private key: {}", e)))?;
                
                if decoded.len() != 32 {
                    return Err(BootstrapError::KeyError(format!("Invalid Ed25519 key length: {}", decoded.len())));
                }
                
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&decoded);
                Ok(SigningKey::from_bytes(&bytes))
            },
            "base58" => {
                let b58 = self.private_key.as_ref().unwrap().as_str()
                    .ok_or_else(|| BootstrapError::KeyError("Invalid Base58 format".to_string()))?;
                
                let (_, decoded) = multibase::decode(b58)
                    .map_err(|e| BootstrapError::KeyError(format!("Failed to decode Base58 key: {}", e)))?;
                
                if decoded.len() != 32 {
                    return Err(BootstrapError::KeyError(format!("Invalid Ed25519 key length: {}", decoded.len())));
                }
                
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&decoded);
                Ok(SigningKey::from_bytes(&bytes))
            },
            _ => Err(BootstrapError::KeyError(format!("Unsupported key format: {}", self.format))),
        }
    }
}

/// Parse a quorum specification string into a QuorumType
fn parse_quorum_spec(quorum_spec: &str) -> Result<QuorumType, BootstrapError> {
    match quorum_spec {
        "all" => Ok(QuorumType::All),
        "majority" => Ok(QuorumType::Majority),
        s if s.starts_with("threshold:") => {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 2 {
                return Err(BootstrapError::InvalidQuorum(format!(
                    "Invalid threshold format: {}. Expected 'threshold:<number>'", s
                )));
            }
            
            let threshold = parts[1].parse::<u8>()
                .map_err(|_| BootstrapError::InvalidQuorum(format!(
                    "Invalid threshold value: {}. Expected a number between 1 and 100", parts[1]
                )))?;
                
            if threshold < 1 || threshold > 100 {
                return Err(BootstrapError::InvalidQuorum(format!(
                    "Threshold must be between 1 and 100, got: {}", threshold
                )));
            }
            
            Ok(QuorumType::Threshold(threshold))
        },
        _ => Err(BootstrapError::InvalidQuorum(format!(
            "Unsupported quorum type: {}. Valid values: all, majority, threshold:<num>", quorum_spec
        ))),
    }
}

/// Main function to run the federation initialization
pub async fn run_init(
    context: &CliContext, 
    name: &str, 
    output_dir: Option<&str>,
    dry_run: bool,
    participant_paths: &[String],
    quorum_spec: &str,
    export_keys: bool,
    key_format: &str,
) -> Result<(), BootstrapError> {
    println!("Initializing federation: {}", name);
    
    // Validate key format
    if key_format != "jwk" && key_format != "base58" {
        return Err(BootstrapError::KeyError(format!(
            "Unsupported key format: {}. Supported formats: jwk, base58", key_format
        )));
    }
    
    // Step 1: Load or generate participant keys
    let mut participant_keys = Vec::new();
    
    if participant_paths.is_empty() {
        // Generate a single federation key
        println!("No participant keys provided. Generating a federation key...");
        let mut csprng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut csprng);
        let participant = ParticipantKey::new(&signing_key, key_format)?;
        
        println!("Generated federation DID: {}", participant.did);
        participant_keys.push(participant);
    } else {
        // Load participant keys from files
        println!("Loading {} participant keys...", participant_paths.len());
        
        for path in participant_paths {
            match ParticipantKey::from_file(Path::new(path)) {
                Ok(key) => {
                    println!("Loaded participant DID: {}", key.did);
                    participant_keys.push(key);
                },
                Err(e) => {
                    return Err(BootstrapError::KeyError(format!(
                        "Failed to load participant key from {}: {}", path, e
                    )));
                }
            }
        }
    }
    
    // Step 2: Create federation metadata
    let fed_did = if participant_keys.len() == 1 {
        // Single participant acts as the federation DID
        participant_keys[0].did.clone()
    } else {
        // Generate a separate federation DID
        let mut csprng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut csprng);
        let federation_key = ParticipantKey::new(&signing_key, key_format)?;
        
        println!("Generated separate federation DID for multi-participant setup: {}", federation_key.did);
        
        // Add federation key as a participant if export_keys is true
        if export_keys {
            participant_keys.push(federation_key.clone());
        }
        
        federation_key.did
    };
    
    // Step 3: Parse quorum configuration
    let quorum_type = parse_quorum_spec(quorum_spec)?;
    
    // Format the quorum config for metadata
    let quorum_description = match &quorum_type {
        QuorumType::All => "all".to_string(),
        QuorumType::Majority => "majority".to_string(),
        QuorumType::Threshold(t) => format!("threshold:{}", t),
        QuorumType::Weighted(_) => "weighted".to_string(),
    };
    
    let metadata = FederationMetadata {
        name: name.to_string(),
        did: fed_did.clone(),
        created_at: Utc::now(),
        guardians: participant_keys.iter().map(|p| p.did.clone()).collect(),
        quorum_config: quorum_description,
        metadata: HashMap::new(),
    };
    
    // Step 4: Set up quorum config for TrustBundle
    let participants: Vec<Did> = participant_keys.iter()
        .map(|p| Did::from(p.did.clone()))
        .collect();
    
    let quorum_config = QuorumConfig {
        quorum_type: quorum_type.clone(),
        participants,
    };
    
    println!("Configured quorum: {} of {} participants required", 
        quorum_config.required_signatures(),
        quorum_config.participants.len()
    );
    
    // Step 5: Create genesis event
    let genesis_event = create_genesis_event(&fed_did, name)?;
    let event_id = calculate_event_hash(&genesis_event);
    
    println!("Created genesis event with ID: {:?}", event_id);
    
    // Step 6: Create TrustBundle (Genesis)
    let participant_dids: Vec<Did> = participant_keys.iter().map(|p| Did::from(p.did.clone())).collect();
    
    // Create quorum config struct correctly
    let quorum_config_for_bundle = QuorumConfig {
        quorum_type: quorum_type.clone(),
        participants: participant_dids,
    };
    
    let mut bundle = TrustBundle::new(
        fed_did.clone(),
        vec![event_id], // Reference the genesis event
        quorum_config_for_bundle,
    );
    
    // Add metadata to bundle
    bundle = bundle.with_metadata("genesis", "true");
    bundle = bundle.with_metadata("description", format!("Genesis bundle for federation '{}'", name));
    
    // Step 7: Sign the bundle with each participant
    println!("Signing bundle with {} participants...", participant_keys.len());
    
    for participant in &participant_keys {
        let signing_key = participant.to_signing_key()?;
        
        // Create a signing function
        let signing_function = |msg: &[u8]| {
            let signature = signing_key.sign(msg);
            signature.to_bytes().to_vec()
        };
        
        // Sign the bundle
        bundle.sign(participant.did.clone().into(), signing_function);
        println!("  Signed with DID: {}", participant.did);
    }
    
    // Generate a deterministic CID for the bundle
    let bundle_hash = hex::encode(bundle.calculate_hash());
    let bundle_cid = format!("bafy{}", &bundle_hash[..46]); // Simplified CID generation for example
    bundle = bundle.with_cid(bundle_cid);
    
    println!("Created genesis TrustBundle with CID: {}", bundle.bundle_cid.as_ref().unwrap());
    
    // If dry run, don't persist or write files
    if dry_run {
        println!("🧪 DRY RUN: Federation initialized (no files written)");
        return Ok(());
    }
    
    // Step 8: Persist with store
    #[cfg(feature = "persistence")]
    let bundle_id = {
        let db_path = context.get_db_path().join("trustbundles");
        fs::create_dir_all(&db_path)?;
        let store = RocksDbTrustBundleStore::new(db_path)?;
        store.store(bundle.clone()).await?
    };
    
    #[cfg(not(feature = "persistence"))]
    let bundle_id = {
        // Create stored bundle
        let stored_bundle = StoredTrustBundle {
            id: bundle.bundle_cid.clone().unwrap_or_else(|| "genesis".to_string()),
            federation_id: Some(fed_did.clone()),
            bundle_type: "genesis".to_string(),
            bundle_content: bundle.clone(),
            created_at: Utc::now().to_rfc3339(),
            anchored_cid: None,
        };
        
        let store = MemoryTrustBundleStore::new();
        let result = store.save_bundle(&stored_bundle).await?;
        // Return the bundle ID
        stored_bundle.id
    };
    
    println!("Persisted TrustBundle with ID: {}", bundle_id);
    
    // Step 9: Write federation files
    let out_dir = PathBuf::from(output_dir.unwrap_or("federation"));
    write_outputs(&out_dir, &metadata, &bundle, &genesis_event, &participant_keys, export_keys)?;
    
    println!("✅ Federation `{}` initialized successfully", name);
    println!("   Output directory: {}", out_dir.display());
    
    Ok(())
}

/// Generate a new DID for the federation
fn generate_federation_did() -> Result<String, BootstrapError> {
    // Generate ED25519 keypair
    let mut csprng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut csprng);
    
    // Convert public key to multibase for DID
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let encoded = multibase::encode(multibase::Base::Base58Btc, pubkey_bytes);
    
    // Create did:key identifier
    let did = format!("did:key:{}", encoded);
    
    Ok(did)
}

/// Create a genesis event for the federation
fn create_genesis_event(federation_did: &str, federation_name: &str) -> Result<DagEvent, BootstrapError> {
    // Create a genesis payload
    let payload = EventPayload::genesis(federation_name);
    
    // Create the event
    let event = DagEvent::new(
        EventType::Genesis,
        federation_did,
        vec![], // No parent events for genesis
        payload,
    );
    
    Ok(event)
}

/// Write federation output files
fn write_outputs(
    output_dir: &Path,
    metadata: &FederationMetadata,
    bundle: &TrustBundle,
    genesis_event: &DagEvent,
    participant_keys: &[ParticipantKey],
    export_keys: bool,
) -> Result<(), BootstrapError> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;
    
    // Write federation.toml
    let federation_toml = toml::to_string(&metadata)
        .map_err(|e| BootstrapError::Creation(format!("Failed to serialize federation metadata: {}", e)))?;
    
    let federation_file_path = output_dir.join("federation.toml");
    let mut federation_file = File::create(federation_file_path)?;
    federation_file.write_all(federation_toml.as_bytes())?;
    
    // Write genesis_bundle.json
    let bundle_json = serde_json::to_string_pretty(bundle)?;
    let bundle_file_path = output_dir.join("genesis_bundle.json");
    let mut bundle_file = File::create(bundle_file_path)?;
    bundle_file.write_all(bundle_json.as_bytes())?;
    
    // Write genesis_event.json
    let event_json = serde_json::to_string_pretty(genesis_event)?;
    let event_file_path = output_dir.join("genesis_event.json");
    let mut event_file = File::create(event_file_path)?;
    event_file.write_all(event_json.as_bytes())?;
    
    // Write federation_keys.json if export_keys is true
    if export_keys {
        let keys_file_path = output_dir.join("federation_keys.json");
        
        // Create a map of DID -> key details
        let mut key_map = HashMap::new();
        for key in participant_keys {
            key_map.insert(key.did.clone(), key);
        }
        
        let keys_json = serde_json::to_string_pretty(&key_map)?;
        let mut keys_file = File::create(&keys_file_path)?;
        keys_file.write_all(keys_json.as_bytes())?;
        
        println!("Exported {} federation keys to {}", participant_keys.len(), keys_file_path.display());
    }
    
    Ok(())
} 