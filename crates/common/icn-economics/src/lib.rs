// ! ICN-Economics: Economic primitives for the InterCooperative Network
// !
// ! This crate provides token and resource tracking for cooperative economics within ICN.

pub mod token;
pub mod storage;
pub mod transaction;

// Re-export key types
pub use token::{ResourceToken, ScopedResourceToken, ResourceType};
pub use storage::{TokenStore, InMemoryTokenStore};
pub use transaction::{ResourceTransaction, TransactionType, TransactionError}; 