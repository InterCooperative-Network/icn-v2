use icn_identity_core::vc::execution_receipt::{ExecutionReceipt, ExecutionReceiptError};
use icn_types::dag::{DagEvent, DagNode, EventPayload, EventType, EventId, DagStore, DagError, SignedDagNode};
use icn_types::{DagPayload, Did, DagNodeBuilder};
use ed25519_dalek::Signature;
use thiserror::Error;
use sha2::{Sha256, Digest};
use chrono::Utc;

#[derive(Error, Debug)]
pub enum AnchorError {
    #[error("Identity error: {0}")]
    Identity(#[from] ExecutionReceiptError),
    #[error("DAG store error: {0}")]
    DagStore(#[from] DagError),
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("Failed to convert receipt ID to CID: {0}")]
    CidConversion(String),
}

/// Anchors an ExecutionReceipt to the DAG by creating a new DagEvent.
pub async fn anchor_execution_receipt(
    receipt: &ExecutionReceipt,
    dag_store: &mut (impl DagStore + Send + Sync + ?Sized), // Added ?Sized to allow trait objects
    triggering_event_id: Option<EventId>, // Optional ID of the event that triggered this execution
) -> Result<EventId, AnchorError> {
    // Convert the receipt to a CID
    let receipt_cid = match receipt.to_cid() {
        Ok(cid) => cid,
        Err(e) => return Err(AnchorError::Identity(e)),
    };

    // The author of the DagEvent will be the issuer of the receipt.
    let author_did = icn_types::Did::from_string(&receipt.issuer)
        .map_err(|e| AnchorError::CidConversion(e.to_string()))?;

    // Determine parent events for the new DAG event.
    // If a triggering_event_id is provided, use it as a parent.
    // Otherwise, use the tips from the DAG store as parents.
    let parent_events = if let Some(parent_id) = triggering_event_id {
        vec![parent_id]
    } else {
        // Get the current tips of the DAG as parents
        match dag_store.get_tips().await {
            Ok(tips) => {
                // Convert Cid objects to EventId
                // Since we're not actually able to convert from one to the other directly,
                // we'll create new EventIds by hashing the CID string
                let mut event_ids = Vec::new();
                for cid in tips {
                    let cid_str = cid.to_string();
                    let event_id = EventId::new(cid_str.as_bytes());
                    event_ids.push(event_id);
                }
                event_ids
            }
            Err(_) => vec![], // Fallback if tips can't be fetched
        }
    };

    let event_payload = EventPayload::Receipt { receipt_cid: receipt_cid.clone() };

    // Create the DagEvent using its constructor
    let dag_event = DagEvent::new(
        EventType::Receipt, // Receipt event type
        author_did.to_string(),
        parent_events,
        event_payload,
    );

    // Create a DagNode using the DagNodeBuilder
    let dag_node = DagNodeBuilder::new()
        .with_payload(DagPayload::ExecutionReceipt(receipt_cid))
        .with_author(author_did)
        .with_label("ExecutionReceipt".to_string())
        .build()
        .map_err(|e| AnchorError::DagStore(e))?;

    // Create a placeholder Signature (64 bytes of zeros)
    // In this version of ed25519-dalek, from_bytes doesn't return a Result
    let empty_sig = Signature::from_bytes(&[0u8; 64]);

    // Create a SignedDagNode with the DagNode
    let signed_node = SignedDagNode {
        node: dag_node,
        signature: empty_sig,
        cid: None
    };

    // Insert the event into the DAG store and get its Cid
    let node_cid = dag_store.add_node(signed_node).await?;
    
    // Create an EventId from the CID by hashing it
    let event_id = EventId::new(node_cid.to_string().as_bytes());

    Ok(event_id)
} 