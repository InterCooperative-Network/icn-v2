use crate::error::CclError;
use crate::quorum::{QuorumPolicy, QuorumProof, QuorumType};
use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
use icn_common::identity::{Identity, ScopedIdentity, Credential};
use icn_common::verification::{Signature, Verifiable};
use icn_services::dag::DagStorage;

use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Types of recovery methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryMethodType {
    /// Quorum of trusted parties
    QuorumApproval,
    
    /// Pre-established backup key
    BackupKey,
    
    /// Social recovery via contacts
    SocialRecovery,
    
    /// Recovery phrase (seed)
    RecoveryPhrase,
}

/// A method for key recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMethod {
    /// Unique ID for this method
    pub id: String,
    
    /// Type of recovery method
    pub method_type: RecoveryMethodType,
    
    /// The identity this method is for
    pub identity_id: String,
    
    /// The scope this method applies to
    pub scope: String,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Optional metadata with method-specific data
    pub metadata: Option<serde_json::Value>,
    
    /// Signature of the original identity
    pub signature: Signature,
}

impl RecoveryMethod {
    /// Create a new recovery method
    pub fn new(
        method_type: RecoveryMethodType,
        identity_id: String,
        scope: String,
        metadata: Option<serde_json::Value>,
        private_key: &SecretKey,
    ) -> Result<Self, CclError> {
        let id = Uuid::new_v4().to_string();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a temporary method without signature for signing
        let temp_method = Self {
            id: id.clone(),
            method_type: method_type.clone(),
            identity_id: identity_id.clone(),
            scope: scope.clone(),
            created_at: timestamp,
            metadata: metadata.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_method)
            .map_err(|e| CclError::Common(e.into()))?;
            
        // Sign the method
        let public_key = PublicKey::from(private_key);
        
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            id,
            method_type,
            identity_id,
            scope,
            created_at: timestamp,
            metadata,
            signature,
        })
    }
    
    /// Verify this recovery method
    pub fn verify(&self, public_key: &[u8]) -> Result<bool, CclError> {
        // Create a temporary method without signature for verification
        let temp_method = Self {
            id: self.id.clone(),
            method_type: self.method_type.clone(),
            identity_id: self.identity_id.clone(),
            scope: self.scope.clone(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_method)
            .map_err(|e| CclError::Common(e.into()))?;
            
        // Verify the signature
        let public_key = PublicKey::from_bytes(public_key)
            .map_err(|_| CclError::Recovery("Invalid public key".into()))?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| CclError::Recovery("Invalid signature format".into()))?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// A key rotation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationRequest {
    /// Unique ID for this request
    pub id: String,
    
    /// The identity being rotated
    pub identity_id: String,
    
    /// The scope of the identity
    pub scope: String,
    
    /// The old public key
    pub old_public_key: Vec<u8>,
    
    /// The new public key
    pub new_public_key: Vec<u8>,
    
    /// The recovery method being used
    pub recovery_method: RecoveryMethodType,
    
    /// Additional proof data for recovery
    pub proof_data: serde_json::Value,
    
    /// Timestamp of the request
    pub timestamp: u64,
    
    /// Optional justification for the rotation
    pub justification: Option<String>,
    
    /// The DAG node that anchors this request
    pub anchor: Option<DAGNodeID>,
}

impl KeyRotationRequest {
    /// Create a new key rotation request
    pub fn new(
        identity_id: String,
        scope: String,
        old_public_key: Vec<u8>,
        new_public_key: Vec<u8>,
        recovery_method: RecoveryMethodType,
        proof_data: serde_json::Value,
        justification: Option<String>,
        anchor: Option<DAGNodeID>,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            id,
            identity_id,
            scope,
            old_public_key,
            new_public_key,
            recovery_method,
            proof_data,
            timestamp,
            justification,
            anchor,
        }
    }
}

/// A key rotation approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationApproval {
    /// The request being approved
    pub request_id: String,
    
    /// The identity approving the rotation
    pub approver: ScopedIdentity,
    
    /// Timestamp of the approval
    pub timestamp: u64,
    
    /// Signature of the approver
    pub signature: Signature,
}

impl KeyRotationApproval {
    /// Create a new key rotation approval
    pub fn new(
        request_id: String,
        approver: ScopedIdentity,
        private_key: &SecretKey,
    ) -> Result<Self, CclError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create a temporary approval without signature for signing
        let temp_approval = Self {
            request_id: request_id.clone(),
            approver: approver.clone(),
            timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_approval)
            .map_err(|e| CclError::Common(e.into()))?;
            
        // Sign the approval
        let public_key = PublicKey::from_bytes(approver.public_key())
            .map_err(|_| CclError::Recovery("Invalid public key".into()))?;
            
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            request_id,
            approver,
            timestamp,
            signature,
        })
    }
    
    /// Verify this approval
    pub fn verify(&self) -> Result<bool, CclError> {
        // Create a temporary approval without signature for verification
        let temp_approval = Self {
            request_id: self.request_id.clone(),
            approver: self.approver.clone(),
            timestamp: self.timestamp,
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_approval)
            .map_err(|e| CclError::Common(e.into()))?;
            
        // Verify the signature
        let public_key = PublicKey::from_bytes(self.approver.public_key())
            .map_err(|_| CclError::Recovery("Invalid public key".into()))?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| CclError::Recovery("Invalid signature format".into()))?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// A completed key rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotation {
    /// The original request
    pub request: KeyRotationRequest,
    
    /// The approvals for this rotation
    pub approvals: Vec<KeyRotationApproval>,
    
    /// The quorum proof for this rotation
    pub quorum_proof: Option<QuorumProof>,
    
    /// Completion timestamp
    pub completed_at: u64,
    
    /// The DAG node that anchors this rotation
    pub anchor: Option<DAGNodeID>,
}

/// Key recovery manager interface
pub trait KeyRecoveryManager: Send + Sync + 'static {
    /// Register a recovery method for an identity
    fn register_recovery_method(
        &self,
        method: RecoveryMethod,
    ) -> Result<(), CclError>;
    
    /// Get recovery methods for an identity
    fn get_recovery_methods(
        &self,
        identity_id: &str,
        scope: &str,
    ) -> Result<Vec<RecoveryMethod>, CclError>;
    
    /// Request a key rotation
    fn request_key_rotation(
        &self,
        request: KeyRotationRequest,
    ) -> Result<(), CclError>;
    
    /// Approve a key rotation
    fn approve_key_rotation(
        &self,
        approval: KeyRotationApproval,
    ) -> Result<(), CclError>;
    
    /// Get a key rotation request
    fn get_rotation_request(
        &self,
        request_id: &str,
    ) -> Result<KeyRotationRequest, CclError>;
    
    /// Get approvals for a key rotation
    fn get_rotation_approvals(
        &self,
        request_id: &str,
    ) -> Result<Vec<KeyRotationApproval>, CclError>;
    
    /// Complete a key rotation
    fn complete_key_rotation(
        &self,
        request_id: &str,
    ) -> Result<KeyRotation, CclError>;
    
    /// Verify recovery proof for a key rotation
    fn verify_recovery_proof(
        &self,
        request: &KeyRotationRequest,
        approvals: &[KeyRotationApproval],
    ) -> Result<bool, CclError>;
}

/// Memory-based implementation of KeyRecoveryManager (for demonstration)
pub struct MemoryKeyRecoveryManager {
    recovery_methods: HashMap<String, Vec<RecoveryMethod>>, // identity_id -> methods
    rotation_requests: HashMap<String, KeyRotationRequest>, // request_id -> request
    rotation_approvals: HashMap<String, Vec<KeyRotationApproval>>, // request_id -> approvals
    completed_rotations: HashMap<String, KeyRotation>, // request_id -> rotation
}

impl MemoryKeyRecoveryManager {
    /// Create a new memory-based key recovery manager
    pub fn new() -> Self {
        Self {
            recovery_methods: HashMap::new(),
            rotation_requests: HashMap::new(),
            rotation_approvals: HashMap::new(),
            completed_rotations: HashMap::new(),
        }
    }
}

impl KeyRecoveryManager for MemoryKeyRecoveryManager {
    fn register_recovery_method(
        &self,
        method: RecoveryMethod,
    ) -> Result<(), CclError> {
        let identity_id = method.identity_id.clone();
        
        let mut methods = self.recovery_methods.get(&identity_id)
            .cloned()
            .unwrap_or_default();
            
        methods.push(method);
        
        self.recovery_methods.insert(identity_id, methods);
        
        Ok(())
    }
    
    fn get_recovery_methods(
        &self,
        identity_id: &str,
        scope: &str,
    ) -> Result<Vec<RecoveryMethod>, CclError> {
        let methods = self.recovery_methods.get(identity_id)
            .cloned()
            .unwrap_or_default();
            
        // Filter methods by scope
        let filtered_methods = methods.into_iter()
            .filter(|m| m.scope == scope)
            .collect();
            
        Ok(filtered_methods)
    }
    
    fn request_key_rotation(
        &self,
        request: KeyRotationRequest,
    ) -> Result<(), CclError> {
        self.rotation_requests.insert(request.id.clone(), request);
        self.rotation_approvals.insert(request.id.clone(), Vec::new());
        
        Ok(())
    }
    
    fn approve_key_rotation(
        &self,
        approval: KeyRotationApproval,
    ) -> Result<(), CclError> {
        let request_id = approval.request_id.clone();
        
        if !self.rotation_requests.contains_key(&request_id) {
            return Err(CclError::Recovery(
                format!("Key rotation request {} not found", request_id)
            ));
        }
        
        // Verify the approval
        if !approval.verify()? {
            return Err(CclError::Recovery("Invalid approval signature".into()));
        }
        
        let mut approvals = self.rotation_approvals.get(&request_id)
            .cloned()
            .unwrap_or_default();
            
        approvals.push(approval);
        
        self.rotation_approvals.insert(request_id, approvals);
        
        Ok(())
    }
    
    fn get_rotation_request(
        &self,
        request_id: &str,
    ) -> Result<KeyRotationRequest, CclError> {
        self.rotation_requests.get(request_id)
            .cloned()
            .ok_or_else(|| CclError::Recovery(
                format!("Key rotation request {} not found", request_id)
            ))
    }
    
    fn get_rotation_approvals(
        &self,
        request_id: &str,
    ) -> Result<Vec<KeyRotationApproval>, CclError> {
        self.rotation_approvals.get(request_id)
            .cloned()
            .ok_or_else(|| CclError::Recovery(
                format!("No approvals found for request {}", request_id)
            ))
    }
    
    fn complete_key_rotation(
        &self,
        request_id: &str,
    ) -> Result<KeyRotation, CclError> {
        let request = self.get_rotation_request(request_id)?;
        let approvals = self.get_rotation_approvals(request_id)?;
        
        // Verify we have enough approvals
        if !self.verify_recovery_proof(&request, &approvals)? {
            return Err(CclError::Recovery(
                "Insufficient approvals for key rotation".into()
            ));
        }
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        let rotation = KeyRotation {
            request,
            approvals,
            quorum_proof: None, // Would be set in a real implementation
            completed_at: timestamp,
            anchor: None,
        };
        
        self.completed_rotations.insert(request_id.to_string(), rotation.clone());
        
        Ok(rotation)
    }
    
    fn verify_recovery_proof(
        &self,
        request: &KeyRotationRequest,
        approvals: &[KeyRotationApproval],
    ) -> Result<bool, CclError> {
        match request.recovery_method {
            RecoveryMethodType::QuorumApproval => {
                // Check for minimum number of approvals (for demonstration, require at least 3)
                if approvals.len() < 3 {
                    return Ok(false);
                }
                
                // In a real implementation, we would verify the approvers against an authorized list
                
                Ok(true)
            }
            RecoveryMethodType::BackupKey => {
                // For backup key, verify proof data contains a valid signature from the backup key
                if let Some(backup_sig_str) = request.proof_data.get("backup_signature") {
                    if let Some(backup_sig) = backup_sig_str.as_str() {
                        // In a real implementation, we would verify the signature
                        
                        return Ok(true);
                    }
                }
                
                Ok(false)
            }
            RecoveryMethodType::SocialRecovery => {
                // For social recovery, check for approvals from trusted contacts
                if approvals.len() < 2 {
                    return Ok(false);
                }
                
                // In a real implementation, we would verify the approvers against trusted contacts
                
                Ok(true)
            }
            RecoveryMethodType::RecoveryPhrase => {
                // For recovery phrase, verify the proof data contains a valid derived key
                if let Some(derived_key) = request.proof_data.get("derived_key") {
                    if let Some(dk_str) = derived_key.as_str() {
                        // In a real implementation, we would derive a key from the recovery phrase
                        // and verify it matches the expected value
                        
                        return Ok(true);
                    }
                }
                
                Ok(false)
            }
        }
    }
}

/// Create recovery method for quorum approval
pub fn create_quorum_recovery(
    identity_id: String,
    scope: String,
    approvers: Vec<String>,
    threshold: u32,
    private_key: &SecretKey,
) -> Result<RecoveryMethod, CclError> {
    let metadata = serde_json::json!({
        "approvers": approvers,
        "threshold": threshold,
    });
    
    RecoveryMethod::new(
        RecoveryMethodType::QuorumApproval,
        identity_id,
        scope,
        Some(metadata),
        private_key,
    )
}

/// Create recovery method using a backup key
pub fn create_backup_key_recovery(
    identity_id: String,
    scope: String,
    backup_public_key: Vec<u8>,
    private_key: &SecretKey,
) -> Result<RecoveryMethod, CclError> {
    let metadata = serde_json::json!({
        "backup_public_key": hex::encode(backup_public_key),
    });
    
    RecoveryMethod::new(
        RecoveryMethodType::BackupKey,
        identity_id,
        scope,
        Some(metadata),
        private_key,
    )
}

/// Create recovery method using social recovery
pub fn create_social_recovery(
    identity_id: String,
    scope: String,
    trusted_contacts: Vec<String>,
    threshold: u32,
    private_key: &SecretKey,
) -> Result<RecoveryMethod, CclError> {
    let metadata = serde_json::json!({
        "trusted_contacts": trusted_contacts,
        "threshold": threshold,
    });
    
    RecoveryMethod::new(
        RecoveryMethodType::SocialRecovery,
        identity_id,
        scope,
        Some(metadata),
        private_key,
    )
}

/// Create recovery method using a recovery phrase
pub fn create_recovery_phrase(
    identity_id: String,
    scope: String,
    phrase_hash: Vec<u8>,
    private_key: &SecretKey,
) -> Result<RecoveryMethod, CclError> {
    let metadata = serde_json::json!({
        "phrase_hash": hex::encode(phrase_hash),
    });
    
    RecoveryMethod::new(
        RecoveryMethodType::RecoveryPhrase,
        identity_id,
        scope,
        Some(metadata),
        private_key,
    )
} 