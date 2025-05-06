use crate::VmContext;
use icn_identity_core::did::DidKey;
use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionSubject, ExecutionStatus, ExecutionScope};
use icn_types::dag::EventId;
use icn_types::Cid;
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ReceiptError {
    #[error("Identity error: {0}")]
    Identity(#[from] icn_identity_core::vc::execution_receipt::ExecutionReceiptError),
    #[error("Keypair not found in VmContext")]
    KeypairNotFound,
    #[error("Host error: {0}")]
    HostError(String), // Placeholder for more specific host errors
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn issue_execution_receipt(
    ctx: &VmContext,
    module_cid: &Cid,
    result_cid: &Cid,
    event_id: Option<&EventId>, // Made event_id optional as per plan
    // Potentially add more context if needed, like specific task_id for MeshCompute
) -> Result<ExecutionReceipt, ReceiptError> {
    // Determine the DID of the node executing
    let executor_did = ctx.node_did.as_ref().ok_or_else(|| ReceiptError::HostError("Node DID not found in VmContext".to_string()))?;
    
    // Determine the Federation DID for issuing
    let federation_did = ctx.federation_did.as_ref().ok_or_else(|| ReceiptError::HostError("Federation DID not found in VmContext".to_string()))?;
    
    // Determine the submitter DID (caller)
    let submitter_did = ctx.caller_did.as_ref().map(|did| did.to_string());

    // Create the ExecutionSubject
    // Scope might need to be more dynamic based on VmContext or execution type
    let subject = ExecutionSubject {
        id: executor_did.to_string(),
        scope: ExecutionScope::Federation { // Defaulting to Federation scope for now
            federation_id: federation_did.to_string(),
        },
        submitter: submitter_did,
        module_cid: module_cid.to_string(),
        result_cid: result_cid.to_string(),
        event_id: event_id.cloned(), // Clone if EventId is passed as a reference
        timestamp: unix_ts(),
        status: ExecutionStatus::Success, // Assuming success for now, could be a param
        additional_properties: None, // Can be extended later
    };

    // Get the federation keypair for signing
    // This assumes VmContext has a method to retrieve the relevant DidKey
    let kp = ctx.federation_keypair().ok_or(ReceiptError::KeypairNotFound)?;
    let verification_method = format!("{}#keys-1", federation_did); // Standard key ID assumption

    // Create and sign the ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        Uuid::new_v4().urn().to_string(), // Generate a URN ID
        federation_did.to_string(),       // Issuer is the Federation
        subject,
    )
    .sign(&kp)?;

    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_identity_core::did::DidKey;
    use icn_types::Did;
    use std::sync::Arc;

    // Mock VmContext for testing
    impl VmContext {
        fn federation_keypair(&self) -> Option<DidKey> {
            self.signing_key.clone()
        }
    }


    #[test]
    fn test_issue_basic_receipt() {
        let node_key = DidKey::generate().unwrap();
        let federation_key = DidKey::generate().unwrap();
        let caller_key = DidKey::generate().unwrap();

        let ctx = VmContext {
            node_did: Some(node_key.did()),
            federation_did: Some(federation_key.did()),
            caller_did: Some(caller_key.did()),
            signing_key: Some(federation_key.clone()), // Federation signs receipts
            // ... other VmContext fields if needed, with defaults or mocks
            gas_limit: 0, 
            gas_used: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            host_abi_version: 0,
            env_vars: std::collections::HashMap::new(),
            max_memory_bytes: 0,
        };

        let module_cid = Cid::try_from("bafyreibgnclts5p6s2jmjvkdbrj36fxxcL734eadwlp73w6k6mhl2gcue").unwrap();
        let result_cid = Cid::try_from("bafyreihg3tkxnvw5t54bhyd67p2agenyycb4wvfxhdxrh2v2qtnPmdazby").unwrap();
        let event_id_bytes = [1u8; 32];
        let event_id = EventId(event_id_bytes);

        let receipt_result = issue_execution_receipt(&ctx, &module_cid, &result_cid, Some(&event_id));

        assert!(receipt_result.is_ok());
        let receipt = receipt_result.unwrap();

        assert_eq!(receipt.issuer, federation_key.did().to_string());
        assert_eq!(receipt.credential_subject.id, node_key.did().to_string());
        assert_eq!(receipt.credential_subject.module_cid, module_cid.to_string());
        assert_eq!(receipt.credential_subject.result_cid, result_cid.to_string());
        assert_eq!(receipt.credential_subject.event_id, Some(event_id));
        assert_eq!(receipt.credential_subject.status, ExecutionStatus::Success);

        match receipt.credential_subject.scope {
            ExecutionScope::Federation { federation_id } => {
                assert_eq!(federation_id, federation_key.did().to_string());
            }
            _ => panic!("Incorrect scope type"),
        }

        // Verify signature
        let verification_result = receipt.verify();
        assert!(verification_result.is_ok());
        assert!(verification_result.unwrap());
    }
} 