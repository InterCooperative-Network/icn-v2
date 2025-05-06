use crate::receipt_store::{
    ReceiptFilter, StoredReceipt, InMemoryWalletReceiptStore, WalletReceiptStore
};
use icn_identity_core::vc::execution_receipt::{ExecutionScope, ExecutionStatus};
use icn_types::{Cid, Did, dag::EventId};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Global store instance
lazy_static::lazy_static! {
    static ref RECEIPT_STORE: Mutex<InMemoryWalletReceiptStore> = Mutex::new(InMemoryWalletReceiptStore::new());
}

/// Get all receipts matching the provided filter criteria
#[uniffi::export]
pub fn list_receipts(
    federation_did: Option<String>,
    module_cid: Option<String>,
    scope: Option<String>,
    status: Option<String>,
    submitter_did: Option<String>,
    start_time: Option<u64>,
    end_time: Option<u64>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Vec<SerializedReceipt> {
    // Parse filter parameters
    let federation_did = federation_did.and_then(|s| s.parse::<Did>().ok());
    let module_cid = module_cid.and_then(|s| s.parse::<Cid>().ok());
    let scope = parse_scope(scope);
    let status = parse_status(status);
    let submitter_did = submitter_did.and_then(|s| s.parse::<Did>().ok());
    
    let execution_date_range = match (start_time, end_time) {
        (Some(start), Some(end)) => Some((start, end)),
        (Some(start), None) => {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            Some((start, now))
        },
        (None, Some(end)) => Some((0, end)),
        (None, None) => None,
    };

    let filter = ReceiptFilter {
        federation_did,
        module_cid,
        scope,
        status,
        submitter_did,
        execution_date_range,
        limit: limit.map(|l| l as usize),
        offset: offset.map(|o| o as usize),
    };

    // Query the store
    match RECEIPT_STORE.lock() {
        Ok(store) => {
            match store.list_receipts(filter) {
                Ok(receipts) => receipts.into_iter().map(SerializedReceipt::from).collect(),
                Err(_) => Vec::new(),
            }
        },
        Err(_) => Vec::new(),
    }
}

/// Get a specific receipt by its ID
#[uniffi::export]
pub fn get_receipt_by_id(id: String) -> Option<SerializedReceipt> {
    match RECEIPT_STORE.lock() {
        Ok(store) => {
            match store.get_receipt_by_id(&id) {
                Ok(Some(receipt)) => Some(SerializedReceipt::from(receipt)),
                _ => None,
            }
        },
        Err(_) => None,
    }
}

/// Get a specific receipt by its Content ID
#[uniffi::export]
pub fn get_receipt_by_cid(cid: String) -> Option<SerializedReceipt> {
    let cid = match cid.parse::<Cid>() {
        Ok(cid) => cid,
        Err(_) => return None,
    };

    match RECEIPT_STORE.lock() {
        Ok(store) => {
            match store.get_receipt_by_cid(&cid) {
                Ok(Some(receipt)) => Some(SerializedReceipt::from(receipt)),
                _ => None,
            }
        },
        Err(_) => None,
    }
}

/// Add a new receipt to the store
#[uniffi::export]
pub fn save_receipt(receipt: SerializedReceipt) -> bool {
    match receipt.try_into() {
        Ok(stored_receipt) => {
            match RECEIPT_STORE.lock() {
                Ok(mut store) => {
                    store.save_receipt(stored_receipt).is_ok()
                },
                Err(_) => false,
            }
        },
        Err(_) => false,
    }
}

/// Delete a receipt by its ID
#[uniffi::export]
pub fn delete_receipt(id: String) -> bool {
    match RECEIPT_STORE.lock() {
        Ok(mut store) => {
            match store.delete_receipt_by_id(&id) {
                Ok(deleted) => deleted,
                Err(_) => false,
            }
        },
        Err(_) => false,
    }
}

// Helper functions to parse string enum values
fn parse_scope(scope: Option<String>) -> Option<ExecutionScope> {
    scope.and_then(|s| match s.to_lowercase().as_str() {
        "federation" => Some(ExecutionScope::Federation),
        "meshcompute" => Some(ExecutionScope::MeshCompute),
        "cooperative" => Some(ExecutionScope::Cooperative),
        _ if s.starts_with("custom:") => {
            Some(ExecutionScope::Custom(s[7..].to_string()))
        },
        _ => None,
    })
}

fn parse_status(status: Option<String>) -> Option<ExecutionStatus> {
    status.and_then(|s| match s.to_lowercase().as_str() {
        "pending" => Some(ExecutionStatus::Pending),
        "completed" => Some(ExecutionStatus::Completed),
        "failed" => Some(ExecutionStatus::Failed),
        _ => None,
    })
}

/// A serializable version of StoredReceipt for FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct SerializedReceipt {
    pub id: String,
    pub cid: String,
    pub federation_did: String,
    pub module_cid: Option<String>,
    pub status: String,
    pub scope: String,
    pub submitter: Option<String>,
    pub execution_timestamp: u64,
    pub result_summary: Option<String>,
    pub source_event_id: Option<String>,
    pub wallet_stored_at: u64,
    pub json_vc: String,
}

impl From<StoredReceipt> for SerializedReceipt {
    fn from(receipt: StoredReceipt) -> Self {
        SerializedReceipt {
            id: receipt.id,
            cid: receipt.cid.to_string(),
            federation_did: receipt.federation_did.to_string(),
            module_cid: receipt.subject.module_cid.map(|cid| cid.to_string()),
            status: format!("{:?}", receipt.subject.status),
            scope: format!("{:?}", receipt.subject.scope),
            submitter: receipt.subject.submitter.map(|did| did.to_string()),
            execution_timestamp: receipt.execution_timestamp,
            result_summary: Some(receipt.subject.result_summary),
            source_event_id: receipt.source_event_id.map(|id| id.to_string()),
            wallet_stored_at: receipt.wallet_stored_at,
            json_vc: serde_json::to_string(&receipt.raw_vc).unwrap_or_default(),
        }
    }
}

impl TryFrom<SerializedReceipt> for StoredReceipt {
    type Error = String;

    fn try_from(ser: SerializedReceipt) -> Result<Self, Self::Error> {
        let cid = ser.cid.parse::<Cid>()
            .map_err(|e| format!("Invalid CID: {}", e))?;
        
        let federation_did = ser.federation_did.parse::<Did>()
            .map_err(|e| format!("Invalid federation DID: {}", e))?;
        
        let module_cid = if let Some(cid_str) = ser.module_cid {
            Some(cid_str.parse::<Cid>()
                .map_err(|e| format!("Invalid module CID: {}", e))?)
        } else {
            None
        };
        
        let submitter = if let Some(did_str) = ser.submitter {
            Some(did_str.parse::<Did>()
                .map_err(|e| format!("Invalid submitter DID: {}", e))?)
        } else {
            None
        };
        
        let status = match ser.status.to_lowercase().as_str() {
            "pending" => ExecutionStatus::Pending,
            "completed" => ExecutionStatus::Completed,
            "failed" => ExecutionStatus::Failed,
            _ => ExecutionStatus::Pending, // Default
        };
        
        let scope = match ser.scope.to_lowercase().as_str() {
            "federation" => ExecutionScope::Federation,
            "meshcompute" => ExecutionScope::MeshCompute,
            "cooperative" => ExecutionScope::Cooperative,
            s if s.starts_with("custom(") && s.ends_with(")") => {
                let custom = s[7..s.len()-1].to_string();
                ExecutionScope::Custom(custom)
            },
            _ => ExecutionScope::Federation, // Default
        };
        
        let source_event_id = if let Some(id_str) = ser.source_event_id {
            Some(id_str.parse::<EventId>()
                .map_err(|e| format!("Invalid event ID: {}", e))?)
        } else {
            None
        };
        
        let raw_vc = serde_json::from_str::<ExecutionReceipt>(&ser.json_vc)
            .map_err(|e| format!("Invalid ExecutionReceipt JSON: {}", e))?;
        
        let subject = ExecutionSubject {
            module_cid,
            status,
            scope,
            submitter,
            timestamp: ser.execution_timestamp,
            result_summary: ser.result_summary.unwrap_or_default(),
            ..Default::default()
        };
        
        Ok(StoredReceipt {
            id: ser.id,
            cid,
            federation_did,
            subject,
            execution_timestamp: ser.execution_timestamp,
            raw_vc,
            source_event_id,
            wallet_stored_at: ser.wallet_stored_at,
        })
    }
} 