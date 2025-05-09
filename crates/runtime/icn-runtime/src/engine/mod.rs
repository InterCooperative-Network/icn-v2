// Export the executor module
pub mod executor;
// New Wasmtime integration with DAG store verification
#[cfg(feature = "wasmtime")]
pub mod wasmtime_integration;

// Re-export types from the executor module
pub use executor::ModernWasmExecutor;
pub use executor::ExecutionResult;
pub use executor::ContextExtension;
// Type alias for backward compatibility
pub type WasmExecutor = ModernWasmExecutor;

// Re-export types from the wasmtime_integration module
#[cfg(feature = "wasmtime")]
pub use wasmtime_integration::{
    WasmExecutionConfig, 
    WasmExecutionContext, 
    WasmExecutionResult, 
    WasmExecutionMetrics, 
    create_execution_receipt
};

// Implement ReceiptContextExt for any context extension
impl<T: ContextExtension + ?Sized> crate::host::receipt::ReceiptContextExt for T {
    fn node_did(&self) -> Option<&icn_types::Did> {
        ContextExtension::node_did(self)
    }
    
    fn federation_did(&self) -> Option<&icn_types::Did> {
        ContextExtension::federation_did(self)
    }
    
    fn caller_did(&self) -> Option<&icn_types::Did> {
        ContextExtension::caller_did(self)
    }
    
    fn federation_keypair(&self) -> Option<icn_identity_core::did::DidKey> {
        ContextExtension::federation_keypair(self)
    }
}

// Implement ContextExtension for Arc<T> wrapped in a Store
// This allows easier access to the context from wasmtime store operations
impl<T: ContextExtension + Send + Sync + 'static> ContextExtension for wasmtime::Store<std::sync::Arc<T>> {
    fn get_execution_config(&self) -> &crate::config::ExecutionConfig {
        self.data().get_execution_config()
    }
    
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)> {
        // Convert &mut Store<Arc<T>> to a mutable reference to Arc<T>
        let arc_mut = self.data_mut();
        
        // Try to get a mutable reference to T through the Arc
        // This will only succeed if the Arc is uniquely owned
        std::sync::Arc::get_mut(arc_mut).and_then(|inner| inner.get_dag_store_mut())
    }
    
    fn node_did(&self) -> Option<&icn_types::Did> {
        self.data().node_did()
    }
    
    fn federation_did(&self) -> Option<&icn_types::Did> {
        self.data().federation_did()
    }
    
    fn caller_did(&self) -> Option<&icn_types::Did> {
        self.data().caller_did()
    }
    
    fn federation_keypair(&self) -> Option<icn_identity_core::did::DidKey> {
        self.data().federation_keypair()
    }
    
    fn membership_index(&self) -> Option<std::sync::Arc<dyn crate::policy::MembershipIndex + Send + Sync>> {
        self.data().membership_index()
    }
    
    fn policy_loader(&self) -> Option<std::sync::Arc<dyn crate::policy::PolicyLoader + Send + Sync>> {
        self.data().policy_loader()
    }
}

// Implement ContextExtension for StoreContext
impl<'a, T: ContextExtension + Send + Sync + 'static> ContextExtension for wasmtime::StoreContext<'a, std::sync::Arc<T>> {
    fn get_execution_config(&self) -> &crate::config::ExecutionConfig {
        self.data().get_execution_config()
    }
    
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)> {
        // StoreContext is immutable so we can't modify the DAG store
        None
    }
    
    fn node_did(&self) -> Option<&icn_types::Did> {
        self.data().node_did()
    }
    
    fn federation_did(&self) -> Option<&icn_types::Did> {
        self.data().federation_did()
    }
    
    fn caller_did(&self) -> Option<&icn_types::Did> {
        self.data().caller_did()
    }
    
    fn federation_keypair(&self) -> Option<icn_identity_core::did::DidKey> {
        self.data().federation_keypair()
    }
    
    fn membership_index(&self) -> Option<std::sync::Arc<dyn crate::policy::MembershipIndex + Send + Sync>> {
        self.data().membership_index()
    }
    
    fn policy_loader(&self) -> Option<std::sync::Arc<dyn crate::policy::PolicyLoader + Send + Sync>> {
        self.data().policy_loader()
    }
}

// Implement ContextExtension for StoreContextMut
impl<'a, T: ContextExtension + Send + Sync + 'static> ContextExtension for wasmtime::StoreContextMut<'a, std::sync::Arc<T>> {
    fn get_execution_config(&self) -> &crate::config::ExecutionConfig {
        self.data().get_execution_config()
    }
    
    fn get_dag_store_mut(&mut self) -> Option<&mut (dyn icn_types::dag::DagStore + Send + Sync)> {
        // Convert &mut StoreContextMut<Arc<T>> to a mutable reference to Arc<T>
        let arc_mut = self.data_mut();
        
        // Try to get a mutable reference to T through the Arc
        // This will only succeed if the Arc is uniquely owned
        std::sync::Arc::get_mut(arc_mut).and_then(|inner| inner.get_dag_store_mut())
    }
    
    fn node_did(&self) -> Option<&icn_types::Did> {
        self.data().node_did()
    }
    
    fn federation_did(&self) -> Option<&icn_types::Did> {
        self.data().federation_did()
    }
    
    fn caller_did(&self) -> Option<&icn_types::Did> {
        self.data().caller_did()
    }
    
    fn federation_keypair(&self) -> Option<icn_identity_core::did::DidKey> {
        self.data().federation_keypair()
    }
    
    fn membership_index(&self) -> Option<std::sync::Arc<dyn crate::policy::MembershipIndex + Send + Sync>> {
        self.data().membership_index()
    }
    
    fn policy_loader(&self) -> Option<std::sync::Arc<dyn crate::policy::PolicyLoader + Send + Sync>> {
        self.data().policy_loader()
    }
}
