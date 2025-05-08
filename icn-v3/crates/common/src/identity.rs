use crate::dag::{DAGNode, DAGNodeID};
use crate::error::CommonError;
use crate::verification::{Signature, Verifiable};

use serde::{Deserialize, Serialize};
use ed25519_dalek::{PublicKey, SecretKey, Keypair};
use sha2::{Digest, Sha256};

use std::collections::HashMap;

/// Identity type representing actors in the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityType {
    /// Individual person
    Individual,
    
    /// Cooperative organization
    Cooperative,
    
    /// Federation of cooperatives
    Federation,
    
    /// Community (unincorporated group)
    Community,
    
    /// Service provider
    Service,
}

/// Base identity representing any actor in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Unique identifier
    pub id: String,
    
    /// Type of identity
    pub identity_type: IdentityType,
    
    /// Human-readable name
    pub name: String,
    
    /// Public key for verification
    pub public_key: Vec<u8>,
    
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

impl Identity {
    /// Create a new identity with a generated keypair
    pub fn new(
        identity_type: IdentityType,
        name: String,
        metadata: Option<serde_json::Value>,
    ) -> (Self, SecretKey) {
        let mut rng = rand::thread_rng();
        let keypair = Keypair::generate(&mut rng);
        
        let id = {
            let mut hasher = Sha256::new();
            hasher.update(keypair.public.as_bytes());
            hasher.update(name.as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };
        
        let identity = Self {
            id,
            identity_type,
            name,
            public_key: keypair.public.to_bytes().to_vec(),
            metadata,
        };
        
        (identity, keypair.secret)
    }
    
    /// Create an identity from an existing keypair
    pub fn from_keypair(
        identity_type: IdentityType,
        name: String,
        keypair: &Keypair,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        let id = {
            let mut hasher = Sha256::new();
            hasher.update(keypair.public.as_bytes());
            hasher.update(name.as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };
        
        Self {
            id,
            identity_type,
            name,
            public_key: keypair.public.to_bytes().to_vec(),
            metadata,
        }
    }
}

/// An identity within a specific scope (cooperative, federation, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedIdentity {
    /// The base identity
    pub identity: Identity,
    
    /// The scope this identity is operating within
    pub scope: String,
    
    /// Reference to the DAG node that establishes this identity's role in this scope
    pub scope_reference: Option<DAGNodeID>,
}

impl ScopedIdentity {
    /// Create a new scoped identity
    pub fn new(
        identity: Identity,
        scope: String, 
        scope_reference: Option<DAGNodeID>,
    ) -> Self {
        Self {
            identity,
            scope,
            scope_reference,
        }
    }
    
    /// Get the public key bytes
    pub fn public_key(&self) -> &[u8] {
        &self.identity.public_key
    }
    
    /// Get the identity ID
    pub fn id(&self) -> &str {
        &self.identity.id
    }
    
    /// Get the identity type
    pub fn identity_type(&self) -> &IdentityType {
        &self.identity.identity_type
    }
}

/// A verifiable credential with claims about an identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// ID of this credential
    pub id: String,
    
    /// The identity this credential is about
    pub subject: String,
    
    /// The identity that issued this credential
    pub issuer: ScopedIdentity,
    
    /// The scope this credential is valid within
    pub scope: String,
    
    /// When this credential was issued (Unix timestamp in milliseconds)
    pub issuance_date: u64,
    
    /// When this credential expires (Unix timestamp in milliseconds), if any
    pub expiry_date: Option<u64>,
    
    /// Claim key-value pairs
    pub claims: HashMap<String, serde_json::Value>,
    
    /// The DAG node that anchors this credential
    pub anchor: Option<DAGNodeID>,
    
    /// Signature of the issuer
    pub signature: Signature,
}

impl Credential {
    /// Create a new credential
    pub fn new(
        subject: String,
        issuer: ScopedIdentity,
        scope: String,
        claims: HashMap<String, serde_json::Value>,
        anchor: Option<DAGNodeID>,
        expiry_date: Option<u64>,
        private_key: &SecretKey,
    ) -> Result<Self, CommonError> {
        let issuance_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        let id = {
            let mut hasher = Sha256::new();
            hasher.update(subject.as_bytes());
            hasher.update(issuer.id().as_bytes());
            hasher.update(issuance_date.to_be_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };
        
        // Create a temporary credential without signature for data signing
        let temp_credential = Self {
            id: id.clone(),
            subject: subject.clone(),
            issuer: issuer.clone(),
            scope: scope.clone(),
            issuance_date,
            expiry_date,
            claims: claims.clone(),
            anchor: anchor.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for signing
        let data_to_sign = serde_json::to_vec(&temp_credential)?;
        
        // Sign the credential
        let public_key = PublicKey::from_bytes(issuer.public_key())
            .map_err(|_| CommonError::SignatureVerification)?;
            
        let keypair = Keypair {
            secret: *private_key,
            public: public_key,
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            id,
            subject,
            issuer,
            scope,
            issuance_date,
            expiry_date,
            claims,
            anchor,
            signature,
        })
    }
}

impl Verifiable for Credential {
    fn verify(&self) -> Result<bool, CommonError> {
        // Create a temporary credential without signature for verification
        let temp_credential = Self {
            id: self.id.clone(),
            subject: self.subject.clone(),
            issuer: self.issuer.clone(),
            scope: self.scope.clone(),
            issuance_date: self.issuance_date,
            expiry_date: self.expiry_date,
            claims: self.claims.clone(),
            anchor: self.anchor.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_credential)?;
        
        // Verify the signature
        let public_key = PublicKey::from_bytes(self.issuer.public_key())
            .map_err(|_| CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => {
                // Check expiry if present
                if let Some(expiry) = self.expiry_date {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                        
                    if now > expiry {
                        return Err(CommonError::InvalidCredential("Credential expired".into()));
                    }
                }
                
                Ok(true)
            }
            Err(_) => Err(CommonError::SignatureVerification),
        }
    }
} 