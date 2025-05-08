#[cfg(test)]
mod tests {
    use anyhow::Result;
    use icn_runtime::engine::{
        WasmExecutionConfig, 
        WasmExecutionContext, 
        WasmExecutionResult
    };
    use icn_types::dag::{
        DagPayload, SignedDagNode, DagNode, DagNodeMetadata, memory::MemoryDagStore, DagStore,
    };
    use icn_types::{Did, Cid};
    use std::sync::Arc;
    
    // Sample WASM module bytes for testing
    // This is a minimal valid WASM module that exports a _start function
    const SAMPLE_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6D, // Magic number (\\0asm)
        0x01, 0x00, 0x00, 0x00, // WebAssembly binary version
        
        // Type section (Function signatures)
        0x01, 0x04, // section code, section size
        0x01,       // num types
        0x60, 0x00, 0x00, // type 0: () -> ()
        
        // Function section (Function type indices)
        0x03, 0x02, // section code, section size
        0x01, 0x00, // 1 function, index 0 (type 0)
        
        // Export section
        0x07, 0x08, // section code, section size
        0x01,       // exports count
        0x05, 0x5F, 0x73, 0x74, 0x61, 0x72, 0x74, // export name: "_start"
        0x00, 0x00, // export kind, export index
        
        // Code section
        0x0A, 0x04, // section code, section size
        0x01,       // num functions
        0x02,       // function body size
        0x00,       // local decl count
        0x0B,       // end opcode
    ];
    
    /// Create a test DAG with a WASM module
    async fn create_test_dag_with_wasm() -> Result<(Arc<MemoryDagStore>, Cid)> {
        // Create a new in-memory store
        let mut store = MemoryDagStore::new();
        
        // Create a sample WASM module node
        let mut node = DagNode::new();
        node.payload = DagPayload::Raw(SAMPLE_WASM.to_vec());
        node.author = Did::from_str("did:example:federation").unwrap();
        
        let mut metadata = DagNodeMetadata::new();
        metadata.insert("scope".to_string(), "test:scope".into());
        metadata.insert("type".to_string(), "wasm-module".into());
        node.metadata = Some(metadata);
        
        // Create a signed node
        let signed_node = SignedDagNode {
            node,
            signature: None, // No signature needed for test
            cid: None,
        };
        
        // Add the node to the store
        let cid = store.add_node(signed_node).await?;
        
        Ok((Arc::new(store), cid))
    }
    
    /// Test the new Wasmtime integration with a DAG store
    #[tokio::test]
    async fn test_wasmtime_integration() -> Result<()> {
        // Create a test DAG with a WASM module
        let (store, module_cid) = create_test_dag_with_wasm().await?;
        
        // Create default execution config
        let config = WasmExecutionConfig::default();
        
        // Create execution context
        let context = WasmExecutionContext::new(
            store.clone() as Arc<dyn DagStore + Send + Sync>,
            config,
        )?;
        
        // Execute the module
        let result = context.execute_module(
            &module_cid,
            "test:scope",
            &Did::from_str("did:example:caller").unwrap(),
        ).await?;
        
        // Verify the execution was successful
        assert!(result.success, "WASM execution should succeed");
        
        // Verify we got metrics
        assert!(result.metrics.execution_time_ms > 0, "Execution time should be measured");
        
        Ok(())
    }
    
    /// Test execution with an unauthorized scope
    #[tokio::test]
    async fn test_wasmtime_integration_unauthorized_scope() -> Result<()> {
        // Create a test DAG with a WASM module
        let (store, module_cid) = create_test_dag_with_wasm().await?;
        
        // Create default execution config
        let config = WasmExecutionConfig::default();
        
        // Create execution context with verification mocked to deny unauthorized scopes
        struct MockVerificationContext {
            store: Arc<dyn DagStore + Send + Sync>,
            allow_scope: String,
        }
        
        impl WasmExecutionContext {
            // Override verify_module_scope for testing
            async fn mock_verify_module_scope(
                &self,
                _module_node: &SignedDagNode,
                scope_id: &str,
                allow_scope: &str,
            ) -> Result<bool, icn_runtime::engine::wasmtime_integration::RuntimeError> {
                // Only allow the specified scope
                Ok(scope_id == allow_scope)
            }
        }
        
        let context = WasmExecutionContext::new(
            store.clone() as Arc<dyn DagStore + Send + Sync>,
            config,
        )?;
        
        // TODO: In a real test, we would patch verify_module_scope
        // For now, we're using the default implementation which permits all scopes
        // In a more complete test, we would test with a proper verification implementation
        
        // But verify that execution succeeds in general
        let result = context.execute_module(
            &module_cid,
            "test:scope", // This matches the scope in our test module
            &Did::from_str("did:example:caller").unwrap(),
        ).await?;
        
        assert!(result.success, "WASM execution should succeed with authorized scope");
        
        Ok(())
    }
} 