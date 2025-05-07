use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use icn_types::Did;
use icn_identity_core::did::DidKey;
use ed25519_dalek::Signature;
use thiserror::Error;

/// Error types for token operations
#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid token amount (must be > 0)")]
    InvalidAmount,
    
    #[error("Insufficient funds")]
    InsufficientFunds,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Types of resources that can be represented by tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// CPU compute units
    ComputeUnit,
    
    /// Storage space (in MB)
    StorageMb,
    
    /// Network bandwidth (in MB)
    BandwidthMb,
    
    /// GPU compute (in minutes)
    GpuMinute,
    
    /// Cooperative governance rights
    GovernancePoint,
    
    /// Generic credits
    Credit,
    
    /// Custom resource type
    Custom(String),
}

/// Base token representing a digital asset within the ICN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceToken {
    /// Type of resource
    pub resource_type: ResourceType,
    
    /// Amount of the resource
    pub amount: u64,
    
    /// When the token was issued
    pub issued_at: DateTime<Utc>,
    
    /// When the token expires (if applicable)
    pub expires_at: Option<DateTime<Utc>>,
    
    /// Federation ID this token belongs to
    pub federation_id: String,
}

impl ResourceToken {
    /// Create a new resource token
    pub fn new(resource_type: ResourceType, amount: u64, federation_id: &str) -> Result<Self, TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        
        Ok(Self {
            resource_type,
            amount,
            issued_at: Utc::now(),
            expires_at: None,
            federation_id: federation_id.to_string(),
        })
    }
    
    /// Add an expiration date to this token
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
    
    /// Check if this token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expires_at {
            Utc::now() > expiry
        } else {
            false
        }
    }
}

/// A resource token scoped to a specific cooperative or community
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedResourceToken {
    /// Base token information
    pub token: ResourceToken,
    
    /// Cooperative or community ID this token belongs to
    pub scope_id: String,
    
    /// DID of the issuer
    pub issuer: Did,
    
    /// Signature of the issuer over the token data
    pub signature: Vec<u8>,
}

impl ScopedResourceToken {
    /// Create a new scoped resource token
    pub fn new(
        resource_type: ResourceType, 
        amount: u64, 
        federation_id: &str, 
        scope_id: &str,
        issuer: Did
    ) -> Result<Self, TokenError> {
        let token = ResourceToken::new(resource_type, amount, federation_id)?;
        
        Ok(Self {
            token,
            scope_id: scope_id.to_string(),
            issuer,
            signature: Vec::new(),
        })
    }
    
    /// Sign this token with a DID key
    pub fn sign(&mut self, key: &DidKey) -> Result<(), TokenError> {
        // Serialize token data for signing
        let token_data = serde_cbor::to_vec(&(
            &self.token,
            &self.scope_id,
            &self.issuer
        )).map_err(|e| TokenError::SerializationError(e.to_string()))?;
        
        // Sign the token data
        let signature = key.sign(&token_data);
        self.signature = signature.to_bytes().to_vec();
        
        Ok(())
    }
    
    /// Verify this token's signature
    pub fn verify(&self, public_key: &ed25519_dalek::VerifyingKey) -> Result<bool, TokenError> {
        // Check if the token is expired
        if self.token.is_expired() {
            return Err(TokenError::TokenExpired);
        }
        
        // Serialize token data for verification
        let token_data = serde_cbor::to_vec(&(
            &self.token,
            &self.scope_id,
            &self.issuer
        )).map_err(|e| TokenError::SerializationError(e.to_string()))?;
        
        // Convert signature bytes to Signature
        let signature = Signature::from_bytes(&self.signature)
            .map_err(|_| TokenError::InvalidSignature)?;
        
        // Verify the signature
        match public_key.verify(&token_data, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Err(TokenError::InvalidSignature),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_identity_core::did::DidKey;
    use ed25519_dalek::{SigningKey, Verifier};
    use rand::rngs::OsRng;
    
    #[test]
    fn test_resource_token_creation() {
        let token = ResourceToken::new(
            ResourceType::ComputeUnit, 
            100, 
            "test-federation"
        ).unwrap();
        
        assert_eq!(token.resource_type, ResourceType::ComputeUnit);
        assert_eq!(token.amount, 100);
        assert_eq!(token.federation_id, "test-federation");
        assert!(token.expires_at.is_none());
        assert!(!token.is_expired());
    }
    
    #[test]
    fn test_scoped_resource_token_signing_and_verification() {
        // Generate a key pair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        
        // Create a DID key and a DID
        let did_key = DidKey::from_signing_key(&signing_key);
        let did = Did::from_string(&did_key.to_did_string()).unwrap();
        
        // Create a scoped resource token
        let mut token = ScopedResourceToken::new(
            ResourceType::ComputeUnit,
            100,
            "test-federation",
            "test-coop",
            did,
        ).unwrap();
        
        // Sign the token
        token.sign(&did_key).unwrap();
        
        // Verify the token
        let result = token.verify(&verifying_key).unwrap();
        assert!(result);
    }
    
    #[test]
    fn test_token_expiration() {
        // Create a token that's already expired
        let mut token = ResourceToken::new(
            ResourceType::ComputeUnit,
            100,
            "test-federation",
        ).unwrap();
        
        // Set expiration to yesterday
        let yesterday = Utc::now() - chrono::Duration::days(1);
        token = token.with_expiration(yesterday);
        
        assert!(token.is_expired());
    }
} 