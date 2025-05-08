use crate::verification::{Signature, Verifiable};
use crate::error::CommonError;
use crate::identity::ScopedIdentity;

use ed25519_dalek::PublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::collections::HashSet;

/// Unique identifier for a DAG node
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DAGNodeID(String);

impl DAGNodeID {
    /// Create a new DAG node ID from a hash
    pub fn new(hash: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(hash);
        let result = hasher.finalize();
        
        Self(hex::encode(result))
    }
    
    /// Get the string representation of the ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Types of DAG nodes representing different cooperative operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DAGNodeType {
    /// Identity node for a cooperative or individual
    Identity,
    
    /// Cooperative formation with initial members
    CooperativeCreation,
    
    /// Federation creation with founding cooperatives
    FederationCreation,
    
    /// Credential issuance node
    CredentialIssuance,
    
    /// Resource allocation policy
    ResourcePolicy,
    
    /// Governance proposal
    Proposal,
    
    /// Vote on a proposal
    Vote,
    
    /// Execution receipt
    Receipt,
    
    /// Custom application-specific node
    Custom(String),
}

/// Header information for a DAG node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAGNodeHeader {
    /// Node type
    pub node_type: DAGNodeType,
    
    /// Timestamp of creation (milliseconds since Unix epoch)
    pub timestamp: u64,
    
    /// Parent node references
    pub parents: HashSet<DAGNodeID>,
    
    /// Scope that this node operates within
    pub scope: String,
    
    /// The identity that created this node
    pub creator: ScopedIdentity,
}

/// A node in the directed acyclic graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAGNode {
    /// Header information
    pub header: DAGNodeHeader,
    
    /// Payload data (specific to the node type)
    pub payload: serde_json::Value,
    
    /// Signature of the creator over the header and payload
    pub signature: Signature,
}

impl DAGNode {
    /// Create a new DAG node
    pub fn new(
        node_type: DAGNodeType,
        parents: HashSet<DAGNodeID>,
        scope: String,
        creator: ScopedIdentity,
        payload: serde_json::Value,
        private_key: &ed25519_dalek::SecretKey,
    ) -> Result<Self, CommonError> {
        let header = DAGNodeHeader {
            node_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            parents,
            scope,
            creator,
        };
        
        let header_json = serde_json::to_vec(&header)?;
        let payload_json = serde_json::to_vec(&payload)?;
        
        let mut data_to_sign = header_json;
        data_to_sign.extend_from_slice(&payload_json);
        
        let keypair = ed25519_dalek::Keypair {
            secret: *private_key,
            public: PublicKey::from(private_key),
        };
        
        let signature_bytes = keypair.sign(&data_to_sign).to_bytes();
        let signature = Signature(signature_bytes.to_vec());
        
        Ok(Self {
            header,
            payload,
            signature,
        })
    }
    
    /// Calculate the ID of this node
    pub fn id(&self) -> Result<DAGNodeID, CommonError> {
        let header_json = serde_json::to_vec(&self.header)?;
        let payload_json = serde_json::to_vec(&self.payload)?;
        
        let mut data = header_json;
        data.extend_from_slice(&payload_json);
        data.extend_from_slice(&self.signature.0);
        
        Ok(DAGNodeID::new(&data))
    }
}

impl Verifiable for DAGNode {
    fn verify(&self) -> Result<bool, CommonError> {
        let header_json = serde_json::to_vec(&self.header)?;
        let payload_json = serde_json::to_vec(&self.payload)?;
        
        let mut data_to_verify = header_json;
        data_to_verify.extend_from_slice(&payload_json);
        
        let public_key_bytes = self.header.creator.public_key();
        let public_key = PublicKey::from_bytes(public_key_bytes)
            .map_err(|_| CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Err(CommonError::SignatureVerification),
        }
    }
} 