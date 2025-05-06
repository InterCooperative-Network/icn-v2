// Add the receipt module
pub mod receipt;

// Re-export receipt functions
pub use receipt::{
    list_receipts,
    get_receipt_by_id,
    get_receipt_by_cid,
    save_receipt,
    delete_receipt,
    SerializedReceipt,
}; 