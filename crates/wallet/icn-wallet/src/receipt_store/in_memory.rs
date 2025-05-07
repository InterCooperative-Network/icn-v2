use std::collections::HashMap;
use std::sync::RwLock;
use std::error::Error as StdError;
use std::fmt;
use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionScope, ExecutionStatus};
use icn_types::{Cid, Did};

use crate::receipt_store::{StoredReceipt, WalletReceiptStore, ReceiptFilter};

#[derive(Debug)]
pub struct InMemoryError(String);

impl fmt::Display for InMemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "In-memory store error: {}", self.0)
    }
}

impl StdError for InMemoryError {}

impl From<String> for InMemoryError {
    fn from(s: String) -> Self {
        InMemoryError(s)
    }
}

/// An in-memory implementation of the WalletReceiptStore trait.
/// 
/// This implementation is thread-safe using RwLock and stores all receipts in a HashMap.
/// It's suitable for prototyping and light clients that don't need persistent storage.
pub struct InMemoryWalletReceiptStore {
    receipts: RwLock<HashMap<String, StoredReceipt>>,
}

impl InMemoryWalletReceiptStore {
    /// Creates a new empty in-memory receipt store.
    pub fn new() -> Self {
        Self {
            receipts: RwLock::new(HashMap::new()),
        }
    }
}

impl WalletReceiptStore for InMemoryWalletReceiptStore {
    type Error = InMemoryError;

    fn save_receipt(&mut self, receipt: StoredReceipt) -> Result<(), Self::Error> {
        let mut lock = self.receipts.write().map_err(|e| InMemoryError(e.to_string()))?;
        lock.insert(receipt.id.clone(), receipt);
        Ok(())
    }

    fn get_receipt_by_id(&self, id: &str) -> Result<Option<StoredReceipt>, Self::Error> {
        let lock = self.receipts.read().map_err(|e| InMemoryError(e.to_string()))?;
        Ok(lock.get(id).cloned())
    }

    fn get_receipt_by_cid(&self, cid: &Cid) -> Result<Option<StoredReceipt>, Self::Error> {
        let lock = self.receipts.read().map_err(|e| InMemoryError(e.to_string()))?;
        Ok(lock.values().find(|r| &r.cid == cid).cloned())
    }

    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<StoredReceipt>, Self::Error> {
        let lock = self.receipts.read().map_err(|e| InMemoryError(e.to_string()))?;
        
        let mut results: Vec<_> = lock
            .values()
            .filter(|r| {
                // Apply all filter conditions
                filter.federation_did.as_ref().map_or(true, |f| f == &r.federation_did)
                    && filter.module_cid.as_ref().map_or(true, |m| {
                        // For module_cid, check if it exists in the subject
                        r.subject.module_cid == m.to_string()
                    })
                    && filter.scope.as_ref().map_or(true, |s| &r.subject.scope == s)
                    && filter.status.as_ref().map_or(true, |s| &r.subject.status == s)
                    && filter.submitter_did.as_ref().map_or(true, |d| {
                        // For submitter, check if it matches the DID string
                        r.subject.submitter.as_ref().map_or(false, |rs| rs == &d.to_string())
                    })
                    && filter.execution_date_range.as_ref().map_or(true, |(start, end)| {
                        r.execution_timestamp >= *start && r.execution_timestamp <= *end
                    })
            })
            .cloned()
            .collect();

        // Apply pagination
        if let Some(offset) = filter.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }
        
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    fn delete_receipt_by_id(&mut self, id: &str) -> Result<bool, Self::Error> {
        let mut lock = self.receipts.write().map_err(|e| InMemoryError(e.to_string()))?;
        Ok(lock.remove(id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_identity_core::vc::execution_receipt::ExecutionSubject;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn current_timestamp() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn mock_receipt(id: &str) -> StoredReceipt {
        let did: Did = "did:example:federation".parse().unwrap();
        let subject = ExecutionSubject {
            module_cid: Some(Cid::default()),
            status: ExecutionStatus::Completed,
            scope: ExecutionScope::Federation,
            submitter: Some(did.clone()),
            timestamp: current_timestamp(),
            result_summary: "Test execution completed".to_string(),
            ..Default::default()
        };
        
        StoredReceipt {
            id: id.to_string(),
            cid: Cid::default(),
            federation_did: did,
            subject,
            execution_timestamp: current_timestamp(),
            raw_vc: ExecutionReceipt::default(),
            source_event_id: None,
            wallet_stored_at: current_timestamp(),
        }
    }

    #[test]
    fn test_save_and_get_receipt() {
        let mut store = InMemoryWalletReceiptStore::new();
        let receipt = mock_receipt("urn:test:123");
        
        // Save the receipt
        store.save_receipt(receipt.clone()).unwrap();
        
        // Retrieve by ID
        let result = store.get_receipt_by_id("urn:test:123").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "urn:test:123");
        
        // Retrieve by CID
        let cid_result = store.get_receipt_by_cid(&receipt.cid).unwrap();
        assert!(cid_result.is_some());
        assert_eq!(cid_result.unwrap().id, receipt.id);
    }

    #[test]
    fn test_delete_receipt() {
        let mut store = InMemoryWalletReceiptStore::new();
        let receipt = mock_receipt("urn:test:delete");
        
        // Save and verify
        store.save_receipt(receipt).unwrap();
        assert!(store.get_receipt_by_id("urn:test:delete").unwrap().is_some());
        
        // Delete and verify
        let deleted = store.delete_receipt_by_id("urn:test:delete").unwrap();
        assert!(deleted);
        assert!(store.get_receipt_by_id("urn:test:delete").unwrap().is_none());
        
        // Try to delete non-existent receipt
        let deleted = store.delete_receipt_by_id("non-existent").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_list_receipts_with_filters() {
        let mut store = InMemoryWalletReceiptStore::new();
        let federation_did: Did = "did:example:federation".parse().unwrap();
        let submitter_did: Did = "did:example:submitter".parse().unwrap();
        
        // Create receipts with different properties
        let mut receipt1 = mock_receipt("r1");
        receipt1.federation_did = federation_did.clone();
        receipt1.subject.submitter = Some(submitter_did.clone());
        receipt1.subject.scope = ExecutionScope::Federation;
        receipt1.execution_timestamp = 1000;
        
        let mut receipt2 = mock_receipt("r2");
        receipt2.federation_did = "did:example:other".parse().unwrap();
        receipt2.subject.submitter = Some(submitter_did.clone());
        receipt2.subject.scope = ExecutionScope::MeshCompute;
        receipt2.execution_timestamp = 2000;
        
        let mut receipt3 = mock_receipt("r3");
        receipt3.federation_did = federation_did.clone();
        receipt3.subject.submitter = Some("did:example:other".parse().unwrap());
        receipt3.subject.scope = ExecutionScope::Federation;
        receipt3.execution_timestamp = 3000;
        
        // Save all receipts
        store.save_receipt(receipt1).unwrap();
        store.save_receipt(receipt2).unwrap();
        store.save_receipt(receipt3).unwrap();
        
        // Test: unfiltered
        let results = store.list_receipts(ReceiptFilter::default()).unwrap();
        assert_eq!(results.len(), 3);
        
        // Test: filter by federation DID
        let filter = ReceiptFilter {
            federation_did: Some(federation_did.clone()),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.federation_did == federation_did));
        
        // Test: filter by submitter DID
        let filter = ReceiptFilter {
            submitter_did: Some(submitter_did),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 2);
        
        // Test: filter by scope
        let filter = ReceiptFilter {
            scope: Some(ExecutionScope::MeshCompute),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "r2");
        
        // Test: filter by time range
        let filter = ReceiptFilter {
            execution_date_range: Some((1500, 2500)),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "r2");
        
        // Test: pagination with limit
        let filter = ReceiptFilter {
            limit: Some(2),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 2);
        
        // Test: pagination with offset
        let filter = ReceiptFilter {
            offset: Some(2),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 1);
        
        // Test: combined filters
        let filter = ReceiptFilter {
            federation_did: Some(federation_did),
            execution_date_range: Some((500, 2000)),
            ..Default::default()
        };
        let results = store.list_receipts(filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "r1");
    }
} 