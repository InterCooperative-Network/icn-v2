use crate::error::CclError;
use icn_common::identity::ScopedIdentity;
use icn_common::verification::Signature;

use serde::{Deserialize, Serialize};

/// Types of attestations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationType {
    /// Membership attestation
    Membership,
    
    /// Resource allocation
    ResourceAllocation,
    
    /// Governance action
    Governance,
    
    /// Custom attestation
    Custom(String),
}

/// An attestation from one identity about another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Unique identifier
    pub id: String,
    
    /// Type of attestation
    pub attestation_type: AttestationType,
    
    /// The identity making the attestation
    pub attester: ScopedIdentity,
    
    /// The identity being attested about
    pub subject: String,
    
    /// Attestation data
    pub data: serde_json::Value,
    
    /// Timestamp of the attestation
    pub timestamp: u64,
    
    /// Signature of the attester
    pub signature: Signature,
}

/// Attestation manager interface
pub trait AttestationManager: Send + Sync + 'static {
    /// Create a new attestation
    fn create_attestation(
        &self,
        attestation: Attestation,
    ) -> Result<(), CclError>;
    
    /// Get attestations about a subject
    fn get_attestations_about(
        &self,
        subject: &str,
    ) -> Result<Vec<Attestation>, CclError>;
    
    /// Get attestations from an attester
    fn get_attestations_from(
        &self,
        attester: &str,
    ) -> Result<Vec<Attestation>, CclError>;
    
    /// Verify an attestation
    fn verify_attestation(
        &self,
        attestation: &Attestation,
    ) -> Result<bool, CclError>;
} 