// Export the executor module
pub mod executor;

// Re-export types from the executor module
pub use executor::ModernWasmExecutor;
pub use executor::ExecutionResult;
pub use executor::ContextExtension;

// Type alias for backward compatibility
pub type WasmExecutor = ModernWasmExecutor;

// Implement ReceiptContextExt for any context extension
impl<T: ContextExtension> crate::host::receipt::ReceiptContextExt for T {
    fn node_did(&self) -> Option<&icn_types::Did> {
        self.node_did()
    }
    
    fn federation_did(&self) -> Option<&icn_types::Did> {
        self.federation_did()
    }
    
    fn caller_did(&self) -> Option<&icn_types::Did> {
        self.caller_did()
    }
    
    fn federation_keypair(&self) -> Option<icn_identity_core::did::DidKey> {
        self.federation_keypair()
    }
} 