use crate::abi::context::HostContext;
use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionReceiptError, ExecutionSubject, ExecutionStatus, ExecutionScope};
use icn_identity_core::did::DidKey;
use icn_types::dag::EventId;
use icn_types::Cid;
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ReceiptError {
    #[error("Identity error: {0}")]
    Identity(#[from] ExecutionReceiptError),
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

// Define a trait as an extension to HostContext to provide the receipt-specific functionality
pub trait ReceiptContextExt {
    fn node_did(&self) -> Option<&icn_types::Did>;
    fn federation_did(&self) -> Option<&icn_types::Did>;
    fn caller_did(&self) -> Option<&icn_types::Did>;
    fn federation_keypair(&self) -> Option<DidKey>;
}

pub fn issue_execution_receipt(
    ctx: &dyn ReceiptContextExt,
    module_cid: &Cid,
    result_cid: &Cid,
    event_id: Option<&EventId>, // Made event_id optional as per plan
) -> Result<ExecutionReceipt, ReceiptError> {
    // Determine the DID of the node executing
    let executor_did = ctx.node_did().ok_or_else(|| ReceiptError::HostError("Node DID not found in context".to_string()))?;
    
    // Determine the Federation DID for issuing
    let federation_did = ctx.federation_did().ok_or_else(|| ReceiptError::HostError("Federation DID not found in context".to_string()))?;
    
    // Determine the submitter DID (caller)
    let submitter_did = ctx.caller_did().map(|did| did.to_string());

    // Create the ExecutionSubject
    // Scope might need to be more dynamic based on context or execution type
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
    let kp = ctx.federation_keypair().ok_or(ReceiptError::KeypairNotFound)?;

    // Create and sign the ExecutionReceipt
    let receipt = ExecutionReceipt::new(
        Uuid::new_v4().urn().to_string(), // Generate a URN ID
        federation_did.to_string(),       // Issuer is the Federation
        subject,
    )
    .sign(&kp)
    .map_err(|e| ReceiptError::Identity(e))?;

    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icn_types::Did;
    use std::sync::Arc;

    // Mock implementation of ReceiptContextExt for testing
    struct MockContext {
        node_did: Option<Did>,
        federation_did: Option<Did>,
        caller_did: Option<Did>,
        signing_key: Option<DidKey>,
    }

    impl ReceiptContextExt for MockContext {
        fn node_did(&self) -> Option<&Did> {
            self.node_did.as_ref()
        }
        
        fn federation_did(&self) -> Option<&Did> {
            self.federation_did.as_ref()
        }
        
        fn caller_did(&self) -> Option<&Did> {
            self.caller_did.as_ref()
        }
        
        fn federation_keypair(&self) -> Option<DidKey> {
            self.signing_key.clone()
        }
    }


    #[test]
    fn test_issue_basic_receipt() {
        let node_key = DidKey::generate().unwrap();
        let federation_key = DidKey::generate().unwrap();
        let caller_key = DidKey::generate().unwrap();

        let ctx = MockContext {
            node_did: Some(node_key.did().clone()),
            federation_did: Some(federation_key.did().clone()),
            caller_did: Some(caller_key.did().clone()),
            signing_key: Some(federation_key.clone()), // Federation signs receipts
        };

        let module_cid = Cid::from_bytes(&[1u8; 32]).unwrap();
        let result_cid = Cid::from_bytes(&[2u8; 32]).unwrap();
        let event_id_bytes = [3u8; 32];
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