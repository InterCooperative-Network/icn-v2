use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionSubject, ExecutionScope, ExecutionStatus};
use icn_types::{Cid, Did, dag::EventId};
use serde::{Deserialize, Serialize};

/// In-memory implementation of the wallet receipt store
pub mod in_memory;
pub use in_memory::InMemoryWalletReceiptStore;

/// Represents a Verifiable Credential ExecutionReceipt stored in the wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredReceipt {
    /// The unique ID of the receipt (e.g., URN or its own CID if self-certified).
    pub id: String,
    /// The CID of the ExecutionReceipt Verifiable Credential itself.
    pub cid: Cid,
    /// The DID of the federation or entity that issued this receipt.
    pub federation_did: Did, // Changed from 'federation' to 'federation_did' for clarity
    /// The detailed subject matter of the receipt.
    pub subject: ExecutionSubject,
    // ExecutionStatus is part of ExecutionSubject, so no need to duplicate here if subject is stored whole.
    // ExecutionScope is part of ExecutionSubject, so no need to duplicate here.
    /// Timestamp of when the execution occurred (from credentialSubject.timestamp).
    pub execution_timestamp: u64,
    /// The full raw ExecutionReceipt VC.
    pub raw_vc: ExecutionReceipt, // Changed from 'raw' to 'raw_vc' for clarity
    /// Optional EventId of the DAG event that anchored this receipt.
    pub source_event_id: Option<EventId>,
    /// Timestamp of when this StoredReceipt was added or last updated in the wallet.
    pub wallet_stored_at: u64, 
}

/// Criteria for filtering stored execution receipts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceiptFilter {
    pub federation_did: Option<Did>,
    pub module_cid: Option<Cid>,
    /// Unix timestamp range (start, end) for `execution_timestamp`.
    pub execution_date_range: Option<(u64, u64)>,
    pub scope: Option<ExecutionScope>, // Users might want to filter by a specific scope variant
    pub status: Option<ExecutionStatus>,
    pub submitter_did: Option<Did>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Trait for a wallet's storage layer that handles ExecutionReceipts.
pub trait WalletReceiptStore: Send + Sync {
    // Type for store-specific errors
    type Error: std::error::Error + Send + Sync + 'static;

    /// Adds or updates a receipt in the store.
    /// Verification of the receipt should happen before calling this method.
    fn save_receipt(&mut self, receipt: StoredReceipt) -> Result<(), Self::Error>;

    /// Retrieves a specific receipt by its ID (which could be its URN or CID).
    fn get_receipt_by_id(&self, id: &str) -> Result<Option<StoredReceipt>, Self::Error>;

    /// Retrieves a specific receipt by its content CID.
    fn get_receipt_by_cid(&self, cid: &Cid) -> Result<Option<StoredReceipt>, Self::Error>;

    /// Lists receipts based on the provided filter criteria.
    fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<StoredReceipt>, Self::Error>;

    /// Deletes a receipt by its ID.
    fn delete_receipt_by_id(&mut self, id: &str) -> Result<bool, Self::Error>; // Returns true if deleted
}

// Example of how DAG sync logic might interact (conceptual):
// fn process_dag_event_for_receipts(
//     event: &icn_types::dag::DagEvent,
//     dag_store: &impl icn_types::dag::DagStore,
//     wallet_receipt_store: &mut impl WalletReceiptStore,
//     trusted_issuers: &std::collections::HashSet<Did>, // To verify issuer trust
// ) -> Result<(), String> {
//     if event.event_type == icn_types::dag::EventType::Receipt {
//         if let icn_types::dag::EventPayload::Receipt { receipt_cid } = &event.payload {
//             // 1. Fetch the full ExecutionReceipt from a broader storage (e.g., IPFS via DAG store or network)
//             // let raw_receipt_bytes = some_storage_system.fetch_by_cid(receipt_cid)?;
//             // let execution_receipt: ExecutionReceipt = serde_json::from_slice(&raw_receipt_bytes)
//             //     .map_err(|e| format!("Failed to deserialize receipt {}: {}", receipt_cid, e))?;
            
//             // This part is conceptual as we don't have the raw receipt bytes here.
//             // Let's assume we fetched it.
//             let execution_receipt: ExecutionReceipt = todo!("Fetch actual ExecutionReceipt VC from receipt_cid via a DAG/IPFS lookup");

//             // 2. Verify the ExecutionReceipt
//             //    - Signature check
//             //    - Issuer trust check (e.g., against a list of trusted federation DIDs)
//             //    - Timestamp validity, etc.
//             if !execution_receipt.verify().unwrap_or(false) { // Assuming verify returns Result<bool, _>
//                 return Err(format!("Receipt {} failed signature verification.", receipt_cid));
//             }
//             if !trusted_issuers.contains(&Did::from_str(&execution_receipt.issuer).unwrap()) { // Simplified DID conversion
//                  return Err(format!("Issuer {} of receipt {} is not trusted.", execution_receipt.issuer, receipt_cid));
//             }

//             // 3. Convert to StoredReceipt and save
//             let stored_receipt = StoredReceipt {
//                 id: execution_receipt.id.clone(),
//                 cid: *receipt_cid, // This should be the CID of the VC itself
//                 federation_did: Did::from_str(&execution_receipt.issuer).unwrap(), // Simplified
//                 subject: execution_receipt.credential_subject.clone(),
//                 execution_timestamp: execution_receipt.credential_subject.timestamp,
//                 raw_vc: execution_receipt,
//                 source_event_id: Some(event.id), // Assuming DagEvent has an 'id' field or method
//                 wallet_stored_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
//             };
//             wallet_receipt_store.save_receipt(stored_receipt)
//                 .map_err(|e| format!("Failed to save receipt {}: {}", receipt_cid, e))?;
//         }
//     }
//     Ok(())
// } 