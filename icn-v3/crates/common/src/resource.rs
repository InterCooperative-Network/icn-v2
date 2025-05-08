use crate::dag::DAGNodeID;
use crate::error::CommonError;
use crate::identity::ScopedIdentity;
use crate::verification::{Signature, Verifiable};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::collections::HashMap;

/// Types of resources that can be metered and allocated
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// CPU computation time in milliseconds
    ComputeTime,
    
    /// Memory allocation in bytes
    Memory,
    
    /// Storage space in bytes
    Storage,
    
    /// Network bandwidth in bytes
    Bandwidth,
    
    /// Number of operations (e.g., API calls, transactions)
    Operations,
    
    /// Custom resource type
    Custom(String),
}

/// A resource allocation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// The unique ID of this allocation
    pub id: String,
    
    /// The identity that issued this allocation
    pub issuer: ScopedIdentity,
    
    /// The identity that this allocation is for
    pub recipient: String,
    
    /// The scope this allocation is valid within
    pub scope: String,
    
    /// When this allocation was issued (Unix timestamp in milliseconds)
    pub issuance_date: u64,
    
    /// When this allocation expires (Unix timestamp in milliseconds), if any
    pub expiry_date: Option<u64>,
    
    /// The allocated resources and their limits
    pub resources: HashMap<ResourceType, u64>,
    
    /// The DAG node that anchors this allocation
    pub anchor: Option<DAGNodeID>,
    
    /// Additional metadata about the allocation
    pub metadata: Option<Value>,
    
    /// Signature of the issuer
    pub signature: Signature,
}

/// A record of resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// The unique ID of this usage record
    pub id: String,
    
    /// The identity that consumed the resources
    pub consumer: ScopedIdentity,
    
    /// The allocation this usage is tracked against
    pub allocation_id: String,
    
    /// The scope this usage occurred within
    pub scope: String,
    
    /// When this usage occurred (Unix timestamp in milliseconds)
    pub timestamp: u64,
    
    /// The resources consumed and their amounts
    pub resources: HashMap<ResourceType, u64>,
    
    /// Optional context about what the resources were used for
    pub context: Option<String>,
    
    /// Additional metadata about the usage
    pub metadata: Option<Value>,
}

/// A receipt for completed execution with resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// The unique ID of this receipt
    pub id: String,
    
    /// The identity that issued this receipt
    pub issuer: ScopedIdentity,
    
    /// The identity that requested the execution
    pub requester: String,
    
    /// The scope this execution occurred within
    pub scope: String,
    
    /// When this execution completed (Unix timestamp in milliseconds)
    pub timestamp: u64,
    
    /// The resources used during execution
    pub resource_usage: ResourceUsage,
    
    /// Result of the execution (success/failure)
    pub success: bool,
    
    /// Optional result data or error message
    pub result: Option<Value>,
    
    /// The DAG node that anchors this receipt
    pub anchor: Option<DAGNodeID>,
    
    /// Signature of the issuer
    pub signature: Signature,
}

impl Verifiable for Receipt {
    fn verify(&self) -> Result<bool, CommonError> {
        // Create a temporary receipt without signature for verification
        let temp_receipt = Self {
            id: self.id.clone(),
            issuer: self.issuer.clone(),
            requester: self.requester.clone(),
            scope: self.scope.clone(),
            timestamp: self.timestamp,
            resource_usage: self.resource_usage.clone(),
            success: self.success,
            result: self.result.clone(),
            anchor: self.anchor.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_receipt)?;
        
        // Verify the signature
        let public_key = ed25519_dalek::PublicKey::from_bytes(self.issuer.public_key())
            .map_err(|_| CommonError::SignatureVerification)?;
            
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.0)
            .map_err(|_| CommonError::SignatureVerification)?;
            
        match public_key.verify_strict(&data_to_verify, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Err(CommonError::SignatureVerification),
        }
    }
}

impl Verifiable for ResourceAllocation {
    fn verify(&self) -> Result<bool, CommonError> {
        // Create a temporary allocation without signature for verification
        let temp_allocation = Self {
            id: self.id.clone(),
            issuer: self.issuer.clone(),
            recipient: self.recipient.clone(),
            scope: self.scope.clone(),
            issuance_date: self.issuance_date,
            expiry_date: self.expiry_date,
            resources: self.resources.clone(),
            anchor: self.anchor.clone(),
            metadata: self.metadata.clone(),
            signature: Signature(vec![]),
        };
        
        // Serialize for verification
        let data_to_verify = serde_json::to_vec(&temp_allocation)?;
        
        // Verify the signature
        let public_key = ed25519_dalek::PublicKey::from_bytes(self.issuer.public_key())
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
                        return Err(CommonError::ResourceExceeded("Allocation expired".into()));
                    }
                }
                
                Ok(true)
            }
            Err(_) => Err(CommonError::SignatureVerification),
        }
    }
} 