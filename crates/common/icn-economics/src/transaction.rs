use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use icn_types::Did;
use icn_types::Cid;
use crate::token::ResourceType;
use thiserror::Error;

/// Error types for resource transactions
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Invalid transaction type")]
    InvalidTransactionType,
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Transaction verification failed")]
    VerificationFailed,
    
    #[error("DAG anchoring failed: {0}")]
    DagAnchoringFailed(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Types of resource transactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    /// Debit tokens from a cooperative
    Debit,
    
    /// Credit tokens to a cooperative
    Credit,
    
    /// Transfer tokens between cooperatives
    Transfer,
    
    /// Create new tokens (minting)
    Mint,
    
    /// Remove tokens from circulation (burning)
    Burn,
}

/// A transaction representing a change in resource tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTransaction {
    /// Type of transaction
    pub transaction_type: TransactionType,
    
    /// Type of resource being transacted
    pub resource_type: ResourceType,
    
    /// Amount of the resource
    pub amount: u64,
    
    /// Source cooperative/community ID (for transfers, debits, burns)
    pub source_id: Option<String>,
    
    /// Destination cooperative/community ID (for transfers, credits, mints)
    pub destination_id: Option<String>,
    
    /// Federation ID this transaction belongs to
    pub federation_id: String,
    
    /// Reference to a job or task this transaction is for
    pub job_reference: Option<Cid>,
    
    /// DID of the authority approving this transaction
    pub authority: Did,
    
    /// Timestamp when the transaction was created
    pub timestamp: DateTime<Utc>,
    
    /// DAG anchors for this transaction (optional)
    pub dag_anchors: Vec<Cid>,
    
    /// Transaction ID (derived from other fields)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Cid>,
}

impl ResourceTransaction {
    /// Create a new resource debit transaction
    pub fn new_debit(
        resource_type: ResourceType,
        amount: u64,
        coop_id: &str,
        federation_id: &str,
        authority: Did,
    ) -> Self {
        Self {
            transaction_type: TransactionType::Debit,
            resource_type,
            amount,
            source_id: Some(coop_id.to_string()),
            destination_id: None,
            federation_id: federation_id.to_string(),
            job_reference: None,
            authority,
            timestamp: Utc::now(),
            dag_anchors: Vec::new(),
            id: None,
        }
    }
    
    /// Create a new resource credit transaction
    pub fn new_credit(
        resource_type: ResourceType,
        amount: u64,
        coop_id: &str,
        federation_id: &str,
        authority: Did,
    ) -> Self {
        Self {
            transaction_type: TransactionType::Credit,
            resource_type,
            amount,
            source_id: None,
            destination_id: Some(coop_id.to_string()),
            federation_id: federation_id.to_string(),
            job_reference: None,
            authority,
            timestamp: Utc::now(),
            dag_anchors: Vec::new(),
            id: None,
        }
    }
    
    /// Create a new resource transfer transaction
    pub fn new_transfer(
        resource_type: ResourceType,
        amount: u64,
        source_id: &str,
        destination_id: &str,
        federation_id: &str,
        authority: Did,
    ) -> Self {
        Self {
            transaction_type: TransactionType::Transfer,
            resource_type,
            amount,
            source_id: Some(source_id.to_string()),
            destination_id: Some(destination_id.to_string()),
            federation_id: federation_id.to_string(),
            job_reference: None,
            authority,
            timestamp: Utc::now(),
            dag_anchors: Vec::new(),
            id: None,
        }
    }
    
    /// Set a reference to a job or task
    pub fn with_job_reference(mut self, job_cid: Cid) -> Self {
        self.job_reference = Some(job_cid);
        self
    }
    
    /// Add a DAG anchor CID to this transaction
    pub fn add_dag_anchor(&mut self, cid: Cid) {
        self.dag_anchors.push(cid);
    }
    
    /// Validate that the transaction has all required fields
    pub fn validate(&self) -> Result<(), TransactionError> {
        match self.transaction_type {
            TransactionType::Debit => {
                if self.source_id.is_none() {
                    return Err(TransactionError::MissingField("source_id".to_string()));
                }
            },
            TransactionType::Credit => {
                if self.destination_id.is_none() {
                    return Err(TransactionError::MissingField("destination_id".to_string()));
                }
            },
            TransactionType::Transfer => {
                if self.source_id.is_none() {
                    return Err(TransactionError::MissingField("source_id".to_string()));
                }
                if self.destination_id.is_none() {
                    return Err(TransactionError::MissingField("destination_id".to_string()));
                }
            },
            TransactionType::Mint => {
                if self.destination_id.is_none() {
                    return Err(TransactionError::MissingField("destination_id".to_string()));
                }
            },
            TransactionType::Burn => {
                if self.source_id.is_none() {
                    return Err(TransactionError::MissingField("source_id".to_string()));
                }
            },
        }
        
        Ok(())
    }
} 