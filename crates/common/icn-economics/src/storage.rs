use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::token::{ResourceType, TokenError};
use crate::transaction::{ResourceTransaction, TransactionType, TransactionError};
use async_trait::async_trait;
use anyhow::Result;

/// Trait defining the interface for token storage backends
#[async_trait]
pub trait TokenStore: Send + Sync {
    /// Get the balance of a specific resource type for a cooperative/community
    async fn get_balance(&self, scope_id: &str, resource_type: &ResourceType) -> Result<u64, TokenError>;
    
    /// Credit tokens to a cooperative/community
    async fn credit(&self, scope_id: &str, resource_type: ResourceType, amount: u64) -> Result<(), TokenError>;
    
    /// Debit tokens from a cooperative/community
    async fn debit(&self, scope_id: &str, resource_type: ResourceType, amount: u64) -> Result<(), TokenError>;
    
    /// Transfer tokens between cooperatives/communities
    async fn transfer(
        &self, 
        source_id: &str, 
        destination_id: &str, 
        resource_type: ResourceType, 
        amount: u64
    ) -> Result<(), TokenError>;
    
    /// Apply a transaction to the token store
    async fn apply_transaction(&self, transaction: &ResourceTransaction) -> Result<(), TransactionError>;
    
    /// Get transaction history for a cooperative/community
    async fn get_transaction_history(&self, scope_id: &str) -> Result<Vec<ResourceTransaction>, TransactionError>;
}

/// In-memory implementation of the TokenStore trait
pub struct InMemoryTokenStore {
    /// Balances indexed by cooperative/community ID and resource type
    balances: RwLock<HashMap<String, HashMap<ResourceType, u64>>>,
    
    /// Transaction history indexed by cooperative/community ID
    transactions: RwLock<HashMap<String, Vec<ResourceTransaction>>>,
}

impl InMemoryTokenStore {
    /// Create a new in-memory token store
    pub fn new() -> Self {
        Self {
            balances: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
        }
    }
    
    /// Get a key for the balances map
    fn balance_key(scope_id: &str, resource_type: &ResourceType) -> String {
        format!("{}:{:?}", scope_id, resource_type)
    }
    
    /// Add a transaction to the history
    async fn add_transaction(&self, transaction: ResourceTransaction) -> Result<(), TransactionError> {
        let mut transactions = self.transactions.write().await;
        
        // Add to source's history if applicable
        if let Some(source_id) = &transaction.source_id {
            if !transactions.contains_key(source_id) {
                transactions.insert(source_id.clone(), Vec::new());
            }
            
            if let Some(tx_list) = transactions.get_mut(source_id) {
                tx_list.push(transaction.clone());
            }
        }
        
        // Add to destination's history if applicable
        if let Some(destination_id) = &transaction.destination_id {
            if !transactions.contains_key(destination_id) {
                transactions.insert(destination_id.clone(), Vec::new());
            }
            
            if let Some(tx_list) = transactions.get_mut(destination_id) {
                tx_list.push(transaction);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl TokenStore for InMemoryTokenStore {
    async fn get_balance(&self, scope_id: &str, resource_type: &ResourceType) -> Result<u64, TokenError> {
        let balances = self.balances.read().await;
        
        if let Some(scope_balances) = balances.get(scope_id) {
            if let Some(balance) = scope_balances.get(resource_type) {
                Ok(*balance)
            } else {
                Ok(0) // No balance for this resource type
            }
        } else {
            Ok(0) // No balances for this scope
        }
    }
    
    async fn credit(&self, scope_id: &str, resource_type: ResourceType, amount: u64) -> Result<(), TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        
        let mut balances = self.balances.write().await;
        
        // Ensure the scope exists in the balances map
        if !balances.contains_key(scope_id) {
            balances.insert(scope_id.to_string(), HashMap::new());
        }
        
        // Update the balance
        if let Some(scope_balances) = balances.get_mut(scope_id) {
            let current = scope_balances.get(&resource_type).copied().unwrap_or(0);
            scope_balances.insert(resource_type, current + amount);
        }
        
        Ok(())
    }
    
    async fn debit(&self, scope_id: &str, resource_type: ResourceType, amount: u64) -> Result<(), TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        
        let mut balances = self.balances.write().await;
        
        // Check if the scope exists and has enough balance
        if let Some(scope_balances) = balances.get(scope_id) {
            let current = scope_balances.get(&resource_type).copied().unwrap_or(0);
            if current < amount {
                return Err(TokenError::InsufficientFunds);
            }
        } else {
            return Err(TokenError::InsufficientFunds);
        }
        
        // Update the balance
        if let Some(scope_balances) = balances.get_mut(scope_id) {
            let current = scope_balances.get(&resource_type).copied().unwrap_or(0);
            scope_balances.insert(resource_type, current - amount);
        }
        
        Ok(())
    }
    
    async fn transfer(
        &self, 
        source_id: &str, 
        destination_id: &str, 
        resource_type: ResourceType, 
        amount: u64
    ) -> Result<(), TokenError> {
        // First debit from source
        self.debit(source_id, resource_type.clone(), amount).await?;
        
        // Then credit to destination
        self.credit(destination_id, resource_type, amount).await?;
        
        Ok(())
    }
    
    async fn apply_transaction(&self, transaction: &ResourceTransaction) -> Result<(), TransactionError> {
        // Validate the transaction
        transaction.validate()?;
        
        // Apply the transaction based on its type
        match transaction.transaction_type {
            TransactionType::Debit => {
                if let Some(source_id) = &transaction.source_id {
                    self.debit(
                        source_id, 
                        transaction.resource_type.clone(), 
                        transaction.amount
                    ).await.map_err(|e| TransactionError::VerificationFailed)?;
                }
            },
            TransactionType::Credit => {
                if let Some(destination_id) = &transaction.destination_id {
                    self.credit(
                        destination_id, 
                        transaction.resource_type.clone(), 
                        transaction.amount
                    ).await.map_err(|e| TransactionError::VerificationFailed)?;
                }
            },
            TransactionType::Transfer => {
                if let (Some(source_id), Some(destination_id)) = (&transaction.source_id, &transaction.destination_id) {
                    self.transfer(
                        source_id,
                        destination_id,
                        transaction.resource_type.clone(),
                        transaction.amount
                    ).await.map_err(|e| TransactionError::VerificationFailed)?;
                }
            },
            TransactionType::Mint => {
                if let Some(destination_id) = &transaction.destination_id {
                    self.credit(
                        destination_id, 
                        transaction.resource_type.clone(), 
                        transaction.amount
                    ).await.map_err(|e| TransactionError::VerificationFailed)?;
                }
            },
            TransactionType::Burn => {
                if let Some(source_id) = &transaction.source_id {
                    self.debit(
                        source_id, 
                        transaction.resource_type.clone(), 
                        transaction.amount
                    ).await.map_err(|e| TransactionError::VerificationFailed)?;
                }
            },
        }
        
        // Add the transaction to history
        self.add_transaction(transaction.clone()).await?;
        
        Ok(())
    }
    
    async fn get_transaction_history(&self, scope_id: &str) -> Result<Vec<ResourceTransaction>, TransactionError> {
        let transactions = self.transactions.read().await;
        
        if let Some(tx_list) = transactions.get(scope_id) {
            Ok(tx_list.clone())
        } else {
            Ok(Vec::new())
        }
    }
}

/// Create a shared token store
pub fn create_shared_token_store() -> Arc<dyn TokenStore> {
    Arc::new(InMemoryTokenStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_types::Did;
    use crate::token::ResourceType;
    
    #[tokio::test]
    async fn test_token_credit_and_debit() {
        let store = InMemoryTokenStore::new();
        
        // Credit tokens
        store.credit("test-coop", ResourceType::ComputeUnit, 100).await.unwrap();
        
        // Check balance
        let balance = store.get_balance("test-coop", &ResourceType::ComputeUnit).await.unwrap();
        assert_eq!(balance, 100);
        
        // Debit tokens
        store.debit("test-coop", ResourceType::ComputeUnit, 30).await.unwrap();
        
        // Check balance again
        let balance = store.get_balance("test-coop", &ResourceType::ComputeUnit).await.unwrap();
        assert_eq!(balance, 70);
    }
    
    #[tokio::test]
    async fn test_token_transfer() {
        let store = InMemoryTokenStore::new();
        
        // Credit tokens to source
        store.credit("source-coop", ResourceType::ComputeUnit, 100).await.unwrap();
        
        // Transfer tokens
        store.transfer(
            "source-coop", 
            "dest-coop", 
            ResourceType::ComputeUnit, 
            50
        ).await.unwrap();
        
        // Check balances
        let source_balance = store.get_balance("source-coop", &ResourceType::ComputeUnit).await.unwrap();
        let dest_balance = store.get_balance("dest-coop", &ResourceType::ComputeUnit).await.unwrap();
        
        assert_eq!(source_balance, 50);
        assert_eq!(dest_balance, 50);
    }
    
    #[tokio::test]
    async fn test_apply_transaction() {
        let store = InMemoryTokenStore::new();
        let did = Did::from_string("did:icn:test").unwrap();
        
        // Credit transaction
        let credit_tx = ResourceTransaction::new_credit(
            ResourceType::ComputeUnit,
            100,
            "test-coop",
            "test-federation",
            did.clone()
        );
        
        // Apply the transaction
        store.apply_transaction(&credit_tx).await.unwrap();
        
        // Check balance
        let balance = store.get_balance("test-coop", &ResourceType::ComputeUnit).await.unwrap();
        assert_eq!(balance, 100);
        
        // Check transaction history
        let history = store.get_transaction_history("test-coop").await.unwrap();
        assert_eq!(history.len(), 1);
    }
} 