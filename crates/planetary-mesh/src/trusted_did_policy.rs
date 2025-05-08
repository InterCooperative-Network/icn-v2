use anyhow::{Result, anyhow, Context};
use icn_core_types::Did;
use icn_identity_core::did::DidKey;
use icn_core_types::Cid;
use icn_types::dag::{DagStore, DagPayload, SignedDagNode, SharedDagStore};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use log::{debug, info, warn, error};
use prometheus::{IntCounterVec, Registry};
use chrono::{DateTime, Utc};
use multibase::{Base, encode, decode};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use std::fs;
use icn_types::dag::DagNodeBuilder;
use std::pin::Pin;
use async_trait;

/// Trust level for DIDs in the policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Fully trusted entity (can submit manifests, dispatch tasks, etc.)
    Full,
    
    /// Can only submit manifests
    ManifestProvider,
    
    /// Can only request tasks
    Requestor,
    
    /// Can only execute tasks
    Worker,
    
    /// Trusted for admin operations
    Admin,
}

/// Policy configuration format (for TOML/JSON loading)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDidPolicyConfig {
    /// Federation ID this policy applies to
    pub federation_id: String,
    
    /// List of trusted DIDs with their trust levels
    pub trusted_dids: Vec<TrustedDidEntry>,
    
    /// Optional reference to a previous policy (for updates)
    pub previous_policy_cid: Option<String>,
    
    /// Whether to allow the policy to be updated via DAG
    pub allow_dag_updates: Option<bool>,
    
    /// Optional list of admin DIDs that can update the policy
    pub policy_admins: Option<Vec<String>>,
}

/// Entry in the trusted DID list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDidEntry {
    /// The DID to trust
    pub did: String,
    
    /// Trust level for this DID
    pub level: TrustLevel,
    
    /// Optional expiration date
    pub expires: Option<DateTime<Utc>>,
    
    /// Optional notes about this DID
    pub notes: Option<String>,
}

/// W3C Verifiable Credential for trust policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicyCredential {
    /// Credential context for JSON-LD
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Credential ID (unique identifier)
    pub id: String,
    
    /// Credential type
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    
    /// Issuer DID (admin that created the policy)
    pub issuer: String,
    
    /// Issuance date
    pub issuanceDate: DateTime<Utc>,
    
    /// Credential subject (trust policy)
    pub credentialSubject: TrustPolicySubject,
    
    /// Cryptographic proof
    pub proof: Option<TrustPolicyProof>,
}

/// Subject of the trust policy credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicySubject {
    /// Federation ID this policy applies to
    pub federationId: String,
    
    /// List of trusted DIDs with their roles
    pub trustedEntities: Vec<TrustedDidEntry>,
    
    /// Previous policy CID if this is an update
    pub previousPolicyId: Option<String>,
    
    /// Effective date of the policy
    pub effectiveDate: DateTime<Utc>,
    
    /// Optional expiration date for the policy
    pub expirationDate: Option<DateTime<Utc>>,
}

/// Cryptographic proof for the trust policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicyProof {
    /// Proof type (e.g., Ed25519Signature2020)
    #[serde(rename = "type")]
    pub proof_type: String,
    
    /// Verification method identifier
    pub verificationMethod: String,
    
    /// Creation date of the proof
    pub created: DateTime<Utc>,
    
    /// Hex-encoded signature value
    pub proofValue: String,
}

/// Main trust policy manager
pub struct TrustedDidPolicy {
    /// Federation ID this policy applies to
    federation_id: String,
    
    /// Set of fully trusted DIDs
    full_trust: RwLock<HashSet<Did>>,
    
    /// Set of manifest provider DIDs
    manifest_providers: RwLock<HashSet<Did>>,
    
    /// Set of requestor DIDs
    requestors: RwLock<HashSet<Did>>,
    
    /// Set of worker DIDs
    workers: RwLock<HashSet<Did>>,
    
    /// Set of admin DIDs
    admins: RwLock<HashSet<Did>>,
    
    /// DID entries with additional metadata
    entries: RwLock<Vec<TrustedDidEntry>>,
    
    /// Previous policy CID if this is an update
    previous_policy_cid: Option<String>,
    
    /// Whether to allow DAG-based updates
    allow_dag_updates: bool,
    
    /// Trust decision metrics
    #[cfg(feature = "metrics")]
    metrics: Option<TrustPolicyMetrics>,
}

/// Metrics for tracking trust policy decisions
#[cfg(feature = "metrics")]
pub struct TrustPolicyMetrics {
    /// Counter for trust check results (trusted vs untrusted, by level)
    pub trust_checks: IntCounterVec,
}

#[cfg(feature = "metrics")]
impl TrustPolicyMetrics {
    /// Create new metrics and register them
    pub fn new(registry: &Registry) -> Self {
        let trust_checks = IntCounterVec::new(
            prometheus::opts!("icn_trust_policy_checks", "Trust policy verification results"),
            &["operation", "result", "level"],
        ).unwrap();
        
        registry.register(Box::new(trust_checks.clone())).unwrap();
        
        Self {
            trust_checks,
        }
    }
}

impl TrustedDidPolicy {
    /// Create a new empty trust policy
    pub fn new(federation_id: String) -> Self {
        Self {
            federation_id,
            full_trust: RwLock::new(HashSet::new()),
            manifest_providers: RwLock::new(HashSet::new()),
            requestors: RwLock::new(HashSet::new()),
            workers: RwLock::new(HashSet::new()),
            admins: RwLock::new(HashSet::new()),
            entries: RwLock::new(Vec::new()),
            previous_policy_cid: None,
            allow_dag_updates: false,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }
    
    /// Create a policy from config file
    pub fn from_config_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = fs::read_to_string(path)
            .context("Failed to read trust policy config file")?;
        
        let config: TrustedDidPolicyConfig = toml::from_str(&config_str)
            .context("Failed to parse trust policy config")?;
        
        let mut policy = Self::new(config.federation_id);
        
        // Set config options
        policy.previous_policy_cid = config.previous_policy_cid;
        policy.allow_dag_updates = config.allow_dag_updates.unwrap_or(false);
        
        // Add all trusted DIDs
        for entry in config.trusted_dids {
            policy.add_trusted_entry(entry)?;
        }
        
        // Add admin DIDs if specified
        if let Some(admins) = config.policy_admins {
            for admin in admins {
                let did = Did::from(admin);
                policy.admins.write().unwrap().insert(did);
            }
        }
        
        Ok(policy)
    }
    
    /// Initialize metrics
    #[cfg(feature = "metrics")]
    pub fn init_metrics(&mut self, registry: &Registry) {
        self.metrics = Some(TrustPolicyMetrics::new(registry));
    }
    
    /// Check if a DID is trusted at any level
    pub fn is_trusted(&self, did: &Did) -> bool {
        let full = self.full_trust.read().unwrap().contains(did);
        let manifest = self.manifest_providers.read().unwrap().contains(did);
        let requestor = self.requestors.read().unwrap().contains(did);
        let worker = self.workers.read().unwrap().contains(did);
        let admin = self.admins.read().unwrap().contains(did);
        
        #[cfg(feature = "metrics")]
        if let Some(metrics) = &self.metrics {
            let result = if full || manifest || requestor || worker || admin {
                "trusted"
            } else {
                "untrusted"
            };
            metrics.trust_checks.with_label_values(&["check", result, "any"]).inc();
        }
        
        full || manifest || requestor || worker || admin
    }
    
    /// Check if a DID is trusted at a specific level
    pub fn is_trusted_for(&self, did: &Did, level: TrustLevel) -> bool {
        let is_trusted = match level {
            TrustLevel::Full => {
                self.full_trust.read().unwrap().contains(did)
            },
            TrustLevel::ManifestProvider => {
                self.full_trust.read().unwrap().contains(did) || 
                self.manifest_providers.read().unwrap().contains(did)
            },
            TrustLevel::Requestor => {
                self.full_trust.read().unwrap().contains(did) || 
                self.requestors.read().unwrap().contains(did)
            },
            TrustLevel::Worker => {
                self.full_trust.read().unwrap().contains(did) || 
                self.workers.read().unwrap().contains(did)
            },
            TrustLevel::Admin => {
                self.admins.read().unwrap().contains(did)
            },
        };
        
        #[cfg(feature = "metrics")]
        if let Some(metrics) = &self.metrics {
            let result = if is_trusted { "trusted" } else { "untrusted" };
            let level_str = match level {
                TrustLevel::Full => "full",
                TrustLevel::ManifestProvider => "manifest",
                TrustLevel::Requestor => "requestor",
                TrustLevel::Worker => "worker",
                TrustLevel::Admin => "admin",
            };
            metrics.trust_checks.with_label_values(&["check_level", result, level_str]).inc();
        }
        
        is_trusted
    }
    
    /// Add a trusted DID entry
    pub fn add_trusted_entry(&mut self, entry: TrustedDidEntry) -> Result<()> {
        let did = Did::from(entry.did.clone());
        
        // Check expiration
        if let Some(expires) = entry.expires {
            if expires < Utc::now() {
                return Err(anyhow!("Cannot add expired trust entry"));
            }
        }
        
        // Add to appropriate collection based on trust level
        match entry.level {
            TrustLevel::Full => {
                self.full_trust.write().unwrap().insert(did.clone());
            },
            TrustLevel::ManifestProvider => {
                self.manifest_providers.write().unwrap().insert(did.clone());
            },
            TrustLevel::Requestor => {
                self.requestors.write().unwrap().insert(did.clone());
            },
            TrustLevel::Worker => {
                self.workers.write().unwrap().insert(did.clone());
            },
            TrustLevel::Admin => {
                self.admins.write().unwrap().insert(did.clone());
            },
        }
        
        // Add to entries collection
        self.entries.write().unwrap().push(entry);
        
        Ok(())
    }
    
    /// Add a trusted DID with specified level
    pub fn add_trusted(&mut self, did: Did, level: TrustLevel) {
        let entry = TrustedDidEntry {
            did: did.to_string(),
            level,
            expires: None,
            notes: None,
        };
        
        // Ignore any errors (e.g., already trusted)
        let _ = self.add_trusted_entry(entry);
    }
    
    /// Remove a trusted DID
    pub fn remove_trusted(&mut self, did: &Did) {
        // Remove from all collections
        self.full_trust.write().unwrap().remove(did);
        self.manifest_providers.write().unwrap().remove(did);
        self.requestors.write().unwrap().remove(did);
        self.workers.write().unwrap().remove(did);
        self.admins.write().unwrap().remove(did);
        
        // Remove from entries collection
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| Did::from(e.did.clone()) != *did);
    }
    
    /// Export the policy as a config
    pub fn to_config(&self) -> TrustedDidPolicyConfig {
        TrustedDidPolicyConfig {
            federation_id: self.federation_id.clone(),
            trusted_dids: self.entries.read().unwrap().clone(),
            previous_policy_cid: self.previous_policy_cid.clone(),
            allow_dag_updates: Some(self.allow_dag_updates),
            policy_admins: Some(
                self.admins.read().unwrap().iter()
                    .map(|did| did.to_string())
                    .collect()
            ),
        }
    }
    
    /// Save the policy to a config file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let config = self.to_config();
        let config_str = toml::to_string_pretty(&config)
            .context("Failed to serialize trust policy config")?;
        
        fs::write(path, config_str)
            .context("Failed to write trust policy config file")?;
        
        Ok(())
    }
    
    /// Create a trust policy credential
    pub fn to_credential(&self, issuer_did: &str) -> TrustPolicyCredential {
        let entries = self.entries.read().unwrap();
        
        let subject = TrustPolicySubject {
            federationId: self.federation_id.clone(),
            trustedEntities: entries.clone(),
            previousPolicyId: self.previous_policy_cid.clone(),
            effectiveDate: Utc::now(),
            expirationDate: None,
        };
        
        TrustPolicyCredential {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://icn.network/context/trust-policy/v1".to_string(),
            ],
            id: format!("urn:icn:trustpolicy:{}", uuid::Uuid::new_v4()),
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "TrustPolicyCredential".to_string(),
            ],
            issuer: issuer_did.to_string(),
            issuanceDate: Utc::now(),
            credentialSubject: subject,
            proof: None,
        }
    }
    
    /// Sign a trust policy credential
    pub fn sign_credential(
        mut credential: TrustPolicyCredential,
        did_key: &DidKey,
    ) -> Result<TrustPolicyCredential> {
        // Store the current issuance date
        let issuance_date = credential.issuanceDate;
        
        // Remove any existing proof before signing
        credential.proof = None;
        
        // Convert to canonical form for signing
        let canonical_bytes = serde_json::to_vec(&credential)
            .context("Failed to serialize credential for signing")?;
        
        // Sign the credential
        let signature = did_key.sign(&canonical_bytes);
        
        // Create proof
        credential.proof = Some(TrustPolicyProof {
            proof_type: "Ed25519Signature2020".to_string(),
            verificationMethod: format!("{}#keys-1", did_key.did()),
            created: issuance_date,
            proofValue: hex::encode(signature.to_bytes()),
        });
        
        Ok(credential)
    }
    
    /// Load a trust policy from a DAG record
    pub async fn from_dag(
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        cid: &Cid,
    ) -> Result<Self> {
        // Get the node from the DAG
        let node = dag_store.get_node(cid).await
            .context("Failed to get trust policy from DAG")?;
        
        // Check if it's a TrustPolicy record
        if let DagPayload::Json(payload) = &node.node.payload {
            if payload.get("type").and_then(|t| t.as_str()) == Some("TrustPolicyRecord") {
                // Extract the policy credential
                if let Some(credential_value) = payload.get("policy") {
                    // Parse the credential
                    let credential: TrustPolicyCredential = serde_json::from_value(credential_value.clone())
                        .context("Failed to parse trust policy credential")?;
                    
                    // Create policy from credential
                    let mut policy = Self::new(credential.credentialSubject.federationId.clone());
                    
                    // Set additional properties
                    policy.previous_policy_cid = credential.credentialSubject.previousPolicyId.clone();
                    policy.allow_dag_updates = true; // It came from DAG, so updates are allowed
                    
                    // Add all trusted DIDs from the credential
                    for entry in credential.credentialSubject.trustedEntities {
                        policy.add_trusted_entry(entry)?;
                    }
                    
                    return Ok(policy);
                }
            }
        }
        
        Err(anyhow!("Node is not a TrustPolicy record or lacks a policy credential"))
    }
}

/// TrustPolicy update record to be stored in the DAG
#[derive(Debug, Serialize, Deserialize)]
pub struct TrustPolicyRecord {
    /// Record type for DAG identification
    pub r#type: String,
    
    /// Federation ID
    pub federation_id: String,
    
    /// Timestamp of the record
    pub timestamp: DateTime<Utc>,
    
    /// The policy credential
    pub policy: TrustPolicyCredential,
}

impl TrustPolicyRecord {
    /// Create a new trust policy record
    pub fn new(federation_id: String, policy: TrustPolicyCredential) -> Self {
        Self {
            r#type: "TrustPolicyRecord".to_string(),
            federation_id,
            timestamp: Utc::now(),
            policy,
        }
    }
    
    /// Convert to a DAG payload
    pub fn to_dag_payload(&self) -> Result<DagPayload> {
        let value = serde_json::to_value(self)
            .context("Failed to serialize trust policy record")?;
        
        Ok(DagPayload::Json(value))
    }
}

/// Factory for creating trust policy instances
pub struct TrustPolicyFactory {
    /// Registry for metrics
    #[cfg(feature = "metrics")]
    registry: Option<Registry>,
}

impl TrustPolicyFactory {
    /// Create a new factory
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "metrics")]
            registry: None,
        }
    }
    
    /// Set metrics registry
    #[cfg(feature = "metrics")]
    pub fn with_registry(mut self, registry: Registry) -> Self {
        self.registry = Some(registry);
        self
    }
    
    /// Load a policy from a file
    pub fn from_file<P: AsRef<Path>>(&self, path: P) -> Result<TrustedDidPolicy> {
        let mut policy = TrustedDidPolicy::from_config_file(path)?;
        
        #[cfg(feature = "metrics")]
        if let Some(registry) = &self.registry {
            policy.init_metrics(registry);
        }
        
        Ok(policy)
    }
    
    /// Load a policy from the DAG
    pub async fn from_dag(
        &self,
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        cid: &Cid,
    ) -> Result<TrustedDidPolicy> {
        let mut policy = TrustedDidPolicy::from_dag(dag_store, cid).await?;
        
        #[cfg(feature = "metrics")]
        if let Some(registry) = &self.registry {
            policy.init_metrics(registry);
        }
        
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    fn create_test_policy() -> TrustedDidPolicy {
        let mut policy = TrustedDidPolicy::new("test-federation".to_string());
        
        // Add some test DIDs
        policy.add_trusted(Did::from("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"), TrustLevel::Full);
        policy.add_trusted(Did::from("did:key:z6MkjchhfUsD6mmvni8mCdXHw216Xrm9bQe2mBH1P5RDjVJG"), TrustLevel::ManifestProvider);
        policy.add_trusted(Did::from("did:key:z6MknGc3ocHs3zdPiJbnaaqDi58WdZaL3X6jpo4FpDcVgW9x"), TrustLevel::Requestor);
        
        policy
    }
    
    #[test]
    fn test_trust_basic_checks() {
        let policy = create_test_policy();
        
        // Check trusted DIDs
        assert!(policy.is_trusted(&Did::from("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK")));
        assert!(policy.is_trusted(&Did::from("did:key:z6MkjchhfUsD6mmvni8mCdXHw216Xrm9bQe2mBH1P5RDjVJG")));
        assert!(policy.is_trusted(&Did::from("did:key:z6MknGc3ocHs3zdPiJbnaaqDi58WdZaL3X6jpo4FpDcVgW9x")));
        
        // Check untrusted DID
        assert!(!policy.is_trusted(&Did::from("did:key:z6MkhyDx5DjrfudJDK9oYM1rhxRtPPphPGQRbwk6jgr9dVRQ")));
    }
    
    #[test]
    fn test_trust_level_checks() {
        let policy = create_test_policy();
        
        // Full trust can do everything
        let full_did = Did::from("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK");
        assert!(policy.is_trusted_for(&full_did, TrustLevel::Full));
        assert!(policy.is_trusted_for(&full_did, TrustLevel::ManifestProvider));
        assert!(policy.is_trusted_for(&full_did, TrustLevel::Requestor));
        assert!(policy.is_trusted_for(&full_did, TrustLevel::Worker));
        
        // Manifest provider can only provide manifests
        let manifest_did = Did::from("did:key:z6MkjchhfUsD6mmvni8mCdXHw216Xrm9bQe2mBH1P5RDjVJG");
        assert!(!policy.is_trusted_for(&manifest_did, TrustLevel::Full));
        assert!(policy.is_trusted_for(&manifest_did, TrustLevel::ManifestProvider));
        assert!(!policy.is_trusted_for(&manifest_did, TrustLevel::Requestor));
        assert!(!policy.is_trusted_for(&manifest_did, TrustLevel::Worker));
        
        // Requestor can only request tasks
        let requestor_did = Did::from("did:key:z6MknGc3ocHs3zdPiJbnaaqDi58WdZaL3X6jpo4FpDcVgW9x");
        assert!(!policy.is_trusted_for(&requestor_did, TrustLevel::Full));
        assert!(!policy.is_trusted_for(&requestor_did, TrustLevel::ManifestProvider));
        assert!(policy.is_trusted_for(&requestor_did, TrustLevel::Requestor));
        assert!(!policy.is_trusted_for(&requestor_did, TrustLevel::Worker));
    }
    
    #[test]
    fn test_add_remove_trusted() {
        let mut policy = create_test_policy();
        
        // Add a new worker DID
        let worker_did = Did::from("did:key:z6MkhyDx5DjrfudJDK9oYM1rhxRtPPphPGQRbwk6jgr9dVRQ");
        policy.add_trusted(worker_did.clone(), TrustLevel::Worker);
        
        // Check it was added correctly
        assert!(policy.is_trusted(&worker_did));
        assert!(policy.is_trusted_for(&worker_did, TrustLevel::Worker));
        assert!(!policy.is_trusted_for(&worker_did, TrustLevel::Full));
        
        // Remove the DID
        policy.remove_trusted(&worker_did);
        
        // Check it was removed
        assert!(!policy.is_trusted(&worker_did));
        assert!(!policy.is_trusted_for(&worker_did, TrustLevel::Worker));
    }
    
    #[test]
    fn test_save_load_config() {
        let policy = create_test_policy();
        
        // Create a temporary file
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        
        // Save the policy
        policy.save_to_file(path).unwrap();
        
        // Load the policy
        let loaded_policy = TrustedDidPolicy::from_config_file(path).unwrap();
        
        // Check federation ID
        assert_eq!(loaded_policy.federation_id, "test-federation");
        
        // Check trusted DIDs
        assert!(loaded_policy.is_trusted(&Did::from("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK")));
        assert!(loaded_policy.is_trusted(&Did::from("did:key:z6MkjchhfUsD6mmvni8mCdXHw216Xrm9bQe2mBH1P5RDjVJG")));
        assert!(loaded_policy.is_trusted(&Did::from("did:key:z6MknGc3ocHs3zdPiJbnaaqDi58WdZaL3X6jpo4FpDcVgW9x")));
    }
    
    #[test]
    fn test_credential_creation() {
        let policy = create_test_policy();
        let did_key = DidKey::new();
        
        // Create a credential from the policy
        let credential = policy.to_credential(&did_key.did().to_string());
        
        // Check credential properties
        assert_eq!(credential.issuer, did_key.did().to_string());
        assert_eq!(credential.credentialSubject.federationId, "test-federation");
        assert_eq!(credential.credentialSubject.trustedEntities.len(), 3);
        
        // Verify we can sign it
        let signed = TrustedDidPolicy::sign_credential(credential, &did_key).unwrap();
        assert!(signed.proof.is_some());
    }
}

impl TrustPolicyCredential {
    /// Verify this credential's signature
    pub fn verify(&self) -> Result<bool> {
        if self.proof.is_none() {
            return Ok(false);
        }
        
        let proof = self.proof.as_ref().unwrap();
        
        // Extract DID from the issuer
        let issuer_did = Did::from(self.issuer.clone());
        
        // Create temporary credential without proof for verification
        let temp_credential = Self {
            context: self.context.clone(),
            id: self.id.clone(),
            credential_type: self.credential_type.clone(),
            issuer: self.issuer.clone(),
            issuanceDate: self.issuanceDate,
            credentialSubject: self.credentialSubject.clone(),
            proof: None,
        };
        
        // Get canonical form for verification
        let canonical_bytes = serde_json::to_vec(&temp_credential)
            .context("Failed to serialize credential for verification")?;
        
        // Extract public key from issuer DID
        // In a real implementation, this would use a DID resolver
        // Here we do a basic check for did:key format
        if !self.issuer.starts_with("did:key:z") {
            return Err(anyhow!("Only did:key DIDs are supported for verification"));
        }
        
        // Extract the key part
        let key_part = self.issuer.trim_start_matches("did:key:");
        
        // Decode the multibase encoding
        let multibase_decoded = decode(key_part)
            .map_err(|e| anyhow!("Failed to decode key part: {}", e))?;
        
        // Check for Ed25519 prefix (0xed01)
        if multibase_decoded.1.len() < 2 || multibase_decoded.1[0] != 0xed || multibase_decoded.1[1] != 0x01 {
            return Err(anyhow!("Unsupported key type, expected Ed25519"));
        }
        
        // Extract public key bytes
        let key_bytes = &multibase_decoded.1[2..];
        if key_bytes.len() != 32 {
            return Err(anyhow!("Invalid key length"));
        }
        
        // Create verifying key
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(key_bytes.try_into().unwrap())
            .map_err(|e| anyhow!("Invalid public key: {}", e))?;
        
        // Decode signature
        let signature_bytes = hex::decode(&proof.proofValue)
            .context("Failed to decode signature")?;
        
        if signature_bytes.len() != 64 {
            return Err(anyhow!("Invalid signature length"));
        }
        
        // Convert Vec<u8> to [u8; 64] for Signature::from_bytes
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&signature_bytes);
        let signature = Signature::from_bytes(&sig_bytes);
        
        // Verify signature
        match verifying_key.verify(&canonical_bytes, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Check if the credential is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expiration) = self.credentialSubject.expirationDate {
            expiration < Utc::now()
        } else {
            false
        }
    }
    
    /// Anchor this credential to the DAG
    pub async fn anchor_to_dag(
        &self,
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        did_key: &DidKey,
    ) -> Result<Cid> {
        // Create a policy record for DAG storage
        let record = TrustPolicyRecord::new(
            self.credentialSubject.federationId.clone(),
            self.clone(),
        );
        
        // Convert to DAG payload
        let payload = record.to_dag_payload()?;

        // Create a DAG node for the policy
        let node = DagNodeBuilder::new()
            .with_payload(payload)
            .with_author(did_key.did().clone())
            .with_federation_id(self.credentialSubject.federationId.clone())
            .with_label("TrustPolicyCredential".to_string())
            .build()?;
        
        // Serialize the node for signing
        let node_bytes = serde_json::to_vec(&node)
            .context("Failed to serialize node")?;
        
        // Sign the node
        let signature = did_key.sign(&node_bytes);
        
        // Create a signed node
        let mut signed_node = SignedDagNode {
            node,
            signature,
            cid: None,
        };
        
        // Calculate the CID
        signed_node.ensure_cid()?;
        
        // Add to the DAG store
        let shared_store = SharedDagStore::from_arc(dag_store.clone());
        let cid = shared_store.add_node(signed_node).await?;
            
        Ok(cid)
    }
}

// Add a new function to TrustedDidPolicy to verify the policy lineage
impl TrustedDidPolicy {
    /// Verify the policy lineage in the DAG
    pub async fn verify_policy_lineage(
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        cid: &Cid,
    ) -> Result<bool> {
        let dag_store_clone = dag_store.clone();
        let cid_clone = cid.clone();
        
        // Create a future that can be sent between threads
        let future = async move {
            // Get the node from the DAG
            let node = dag_store_clone.get_node(&cid_clone).await
                .context("Failed to get trust policy from DAG")?;
            
            // Check if it's a TrustPolicy record
            if let DagPayload::Json(payload) = &node.node.payload {
                if payload.get("type").and_then(|t| t.as_str()) == Some("TrustPolicyRecord") {
                    // Extract the policy credential
                    if let Some(credential_value) = payload.get("policy") {
                        // Parse the credential
                        let credential: TrustPolicyCredential = serde_json::from_value(credential_value.clone())
                            .context("Failed to parse trust policy credential")?;
                        
                        // Verify the signature
                        if !credential.verify()? {
                            debug!("Policy credential signature verification failed");
                            return Ok(false);
                        }
                        
                        // Check if expired
                        if credential.is_expired() {
                            debug!("Policy credential is expired");
                            return Ok(false);
                        }
                        
                        // Check previous policy reference if exists
                        if let Some(prev_cid_str) = &credential.credentialSubject.previousPolicyId {
                            // Check if the CID matches the previous CID
                            let prev_cid = Cid::try_from(prev_cid_str.as_str())
                                .map_err(|_| anyhow!("Invalid previous CID format"))?;
                            
                            // Verify that the previous node exists and is reachable
                            match dag_store_clone.get_node(&prev_cid).await {
                                Ok(_) => {
                                    // Verify previous policy lineage recursively
                                    if !Self::verify_policy_lineage(&dag_store_clone, &prev_cid).await? {
                                        debug!("Previous policy lineage verification failed");
                                        return Ok(false);
                                    }
                                },
                                Err(e) => {
                                    debug!("Failed to get previous policy: {}", e);
                                    return Ok(false);
                                }
                            }
                        }
                        
                        // Verify that the issuer is authorized to update the policy
                        // For the genesis policy, any issuer is accepted
                        // For updates, the issuer must be in the previous policy's admin list
                        if let Some(prev_cid_str) = &credential.credentialSubject.previousPolicyId {
                            // Check if the CID matches the previous CID
                            let prev_cid = Cid::try_from(prev_cid_str.as_str())
                                .map_err(|_| anyhow!("Invalid previous CID format"))?;
                            
                            // Load the previous policy
                            let prev_policy = Self::from_dag(&dag_store_clone, &prev_cid).await?;
                            
                            // Check if the issuer is in the admins list
                            let issuer_did = Did::from(credential.issuer.clone());
                            if !prev_policy.is_trusted_for(&issuer_did, TrustLevel::Admin) {
                                debug!("Policy update issuer is not authorized");
                                return Ok(false);
                            }
                        }
                        
                        // All checks passed
                        return Ok(true);
                    }
                }
            }
            
            debug!("Node is not a TrustPolicy record or lacks a policy credential");
            Ok(false)
        };
        
        // Box the future to resolve the recursion issue
        Box::pin(future).await
    }

    /// Find the latest valid policy in the DAG starting from a specific CID
    pub async fn find_latest_valid_policy(
        dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
        cid: &Cid,
    ) -> Result<Option<(Cid, TrustedDidPolicy)>> {
        // Verify the policy lineage
        if !Self::verify_policy_lineage(dag_store, cid).await? {
            return Ok(None);
        }
        
        // Load the policy
        let policy = Self::from_dag(dag_store, cid).await?;
        
        // Check for newer versions by scanning the DAG
        let nodes = dag_store.get_ordered_nodes().await
            .context("Failed to get DAG nodes")?;
        
        let mut latest_cid = cid.clone();
        let mut latest_policy = policy;
        let mut latest_timestamp = chrono::DateTime::<Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
            Utc,
        );
        
        for node in nodes {
            if let Some(node_cid) = &node.cid {
                // Skip the current policy
                if node_cid == cid {
                    continue;
                }
                
                // Check if it's a TrustPolicy record
                if let DagPayload::Json(payload) = &node.node.payload {
                    if payload.get("type").and_then(|t| t.as_str()) == Some("TrustPolicyRecord") {
                        if let Some(credential_value) = payload.get("policy") {
                            if let Ok(credential) = serde_json::from_value::<TrustPolicyCredential>(credential_value.clone()) {
                                // Check if it's for the same federation
                                if credential.credentialSubject.federationId != latest_policy.federation_id {
                                    continue;
                                }
                                
                                // Check if it's newer
                                if credential.issuanceDate > latest_timestamp {
                                    // Verify lineage
                                    if Self::verify_policy_lineage(dag_store, node_cid).await? {
                                        // Load the policy
                                        if let Ok(new_policy) = Self::from_dag(dag_store, node_cid).await {
                                            latest_cid = node_cid.clone();
                                            latest_policy = new_policy;
                                            latest_timestamp = credential.issuanceDate;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(Some((latest_cid, latest_policy)))
    }
}

// Add a new CLI utility to anchor trust policies
/// Publish a trusted policy to the DAG
pub async fn publish_trust_policy_to_dag(
    policy: &TrustedDidPolicy,
    issuer_did: &str,
    issuer_key: &DidKey,
    dag_store: &Arc<Box<dyn DagStore + Send + Sync>>,
    prev_policy_cid: Option<&str>,
) -> Result<Cid> {
    // Create a credential from the policy
    let mut credential = policy.to_credential(issuer_did);
    
    // Set previous policy CID if provided
    if let Some(prev_cid) = prev_policy_cid {
        credential.credentialSubject.previousPolicyId = Some(prev_cid.to_string());
    }
    
    // Sign the credential
    let signed_credential = TrustedDidPolicy::sign_credential(credential, issuer_key)?;
    
    // Anchor to DAG
    signed_credential.anchor_to_dag(dag_store, issuer_key).await
}

// Helper function to verify a signature
fn verify_signature(public_key_multibase: &str, message: &[u8], signature_multibase: &str) -> Result<bool> {
    // Decode the public key from multibase
    let multibase_decoded = decode(public_key_multibase)
        .map_err(|_| anyhow!("Invalid public key format"))?;
    
    // Check if it's an Ed25519 key
    if multibase_decoded.1.len() < 2 || multibase_decoded.1[0] != 0xed || multibase_decoded.1[1] != 0x01 {
        return Err(anyhow!("Invalid public key format - not Ed25519"));
    }
    
    // Extract the key bytes
    let key_bytes = &multibase_decoded.1[2..];
    
    // Create the verifying key
    let bytes32: [u8; 32] = key_bytes.try_into()
        .map_err(|_| anyhow!("Invalid key length"))?;
        
    let verifying_key = VerifyingKey::from_bytes(&bytes32)
        .map_err(|_| anyhow!("Invalid public key"))?;
    
    // Decode the signature from multibase
    let signature_bytes = decode(signature_multibase)
        .map_err(|_| anyhow!("Invalid signature format"))?.1;
    
    // Need exactly 64 bytes for an Ed25519 signature
    if signature_bytes.len() != 64 {
        return Err(anyhow!("Invalid signature length"));
    }
    
    // Create a 64-byte array from the signature bytes
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&signature_bytes[0..64]);
    
    // Create the signature object
    let signature = Signature::from_bytes(&sig_bytes);
    
    // Verify the signature
    verifying_key.verify_strict(message, &signature)
        .map(|_| true)
        .or_else(|_| Ok(false))
}

// Replace CloneDagStore with a Send + Sync + 'static compatible version
/// Helper structure to wrap DagStore in a way that can be cloned
pub(crate) struct CloneDagStore {
    store: Arc<Box<dyn DagStore + Send + Sync + 'static>>,
}

impl CloneDagStore {
    pub(crate) fn new(store: Arc<Box<dyn DagStore + Send + Sync + 'static>>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl DagStore for CloneDagStore {
    async fn add_node(&mut self, node: SignedDagNode) -> Result<Cid, icn_types::dag::DagError> {
        // Use SharedDagStore::from_arc which is specifically designed for this case
        let shared_store = SharedDagStore::from_arc(self.store.clone());
        shared_store.add_node(node).await
    }

    async fn get_node(&self, cid: &Cid) -> Result<SignedDagNode, icn_types::dag::DagError> {
        self.store.get_node(cid).await
    }

    async fn get_data(&self, cid: &Cid) -> Result<Option<Vec<u8>>, icn_types::dag::DagError> {
        self.store.get_data(cid).await
    }

    async fn get_tips(&self) -> Result<Vec<Cid>, icn_types::dag::DagError> {
        self.store.get_tips().await
    }

    async fn get_ordered_nodes(&self) -> Result<Vec<SignedDagNode>, icn_types::dag::DagError> {
        self.store.get_ordered_nodes().await
    }

    async fn get_nodes_by_author(&self, author: &Did) -> Result<Vec<SignedDagNode>, icn_types::dag::DagError> {
        self.store.get_nodes_by_author(author).await
    }

    async fn get_nodes_by_payload_type(&self, payload_type: &str) -> Result<Vec<SignedDagNode>, icn_types::dag::DagError> {
        self.store.get_nodes_by_payload_type(payload_type).await
    }

    async fn find_path(&self, from: &Cid, to: &Cid) -> Result<Vec<SignedDagNode>, icn_types::dag::DagError> {
        self.store.find_path(from, to).await
    }

    async fn verify_branch(&self, tip: &Cid, resolver: &(dyn icn_types::dag::PublicKeyResolver + Send + Sync)) -> Result<(), icn_types::dag::DagError> {
        self.store.verify_branch(tip, resolver).await
    }
} 