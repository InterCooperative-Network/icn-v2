use anyhow::{Result, anyhow, Context};
use chrono::{DateTime, Utc};
use icn_identity_core::{Did, did::DidKey};
use icn_types::dag::{DagStore, Cid, DagPayload, SignedDagNode};
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::fs;
use log::{debug, info, warn, error};
use prometheus::{IntCounterVec, Registry};

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
        dag_store: &Arc<Box<dyn DagStore>>,
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
        dag_store: &Arc<Box<dyn DagStore>>,
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