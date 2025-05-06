use icn_identity_core::vc::execution_receipt::ExecutionReceipt;
use icn_types::dag::{DagEvent, EventPayload, EventType, EventId, DagStore, DagError};
use icn_types::Cid; // Ensure Cid is imported
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Error, Debug)]
pub enum AnchorError {
    #[error("Identity error: {0}")]
    Identity(#[from] icn_identity_core::vc::execution_receipt::ExecutionReceiptError),
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
    dag_store: &mut (impl DagStore + Send + Sync), // Added Send + Sync for async context
    triggering_event_id: Option<EventId>, // Optional ID of the event that triggered this execution
) -> Result<EventId, AnchorError> {
    // Use the to_cid() method we added to ExecutionReceipt
    let receipt_cid = receipt.to_cid()?;

    // The author of the DagEvent will be the issuer of the receipt.
    let author_did = receipt.issuer.clone();

    // Determine parent events for the new DAG event.
    // If a triggering_event_id is provided, use it as a parent.
    // Otherwise, try to use the latest event from the DAG store as a parent (common practice).
    let parent_events = if let Some(parent_id) = triggering_event_id {
        vec![parent_id]
    } else {
        // Fallback to latest event if no specific parent is given.
        // This requires DagStore to have a method like `get_latest_event_ids` or similar.
        // For now, assuming a simple case or that it might be empty if no parent context.
        // dag_store.get_latest_event_ids(1).await.unwrap_or_default()
        // Let's assume for now, if no specific parent, it might be an initial event or link to a known anchor.
        // For simplicity, if not provided, we'll use an empty vec, or one would fetch a relevant head.
        match dag_store.get_head_cids().await {
            Ok(heads) => heads.into_iter().map(EventId::from).collect(),
            Err(_) => vec![], // Fallback if heads can't be fetched
        }
    };

    let event_payload = EventPayload::Receipt { receipt_cid };

    // Create the DagEvent using its constructor
    let dag_event = DagEvent::new(
        EventType::Receipt, // Assuming EventType::Receipt is now defined
        author_did,
        parent_events,
        event_payload,
    );

    // The DagEvent::new constructor initializes signature as empty.
    // If the event itself needs to be signed (e.g., by the node/author creating this DAG entry),
    // that would be a separate step, e.g.:
    // let signed_event = dag_event.sign(author_keypair)?; // Assuming a sign method on DagEvent
    // For now, we insert it with the default empty signature as per DagEvent::new.

    // Insert the event into the DAG store and get its EventId (which is its CID).
    let event_id = dag_store.add_event(dag_event).await?;

    Ok(event_id)
} 