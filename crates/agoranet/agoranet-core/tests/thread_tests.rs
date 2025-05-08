//! Integration tests for AgoraThread functionality.

use agoranet_core::{
    // error::AgoraError, // Removed unused import warning
    message::{Body, CommentBody, Message, ThreadAnchor},
    storage::{AsyncStorage, InMemoryStorage},
    thread::{AgoraThread, ThreadOperations},
};
// use cid::Cid; // Remove potentially conflicting import
use icn_core_types::{Cid, Did}; // Use Cid wrapper from icn_core_types directly
use std::sync::Arc;

// Helper to create a dummy message
// Note: This assumes Did::default() and Cid::default() are suitable for tests.
async fn create_dummy_message(
    storage: Arc<InMemoryStorage>,
    text: String,
    parent: Option<Cid>,
) -> Message {
    let body = Body::Comment(CommentBody { text });
    let body_cid = storage
        .put_ipld(&body)
        .await
        .expect("Failed to store body");

    Message {
        author: Did::default(), // Use a default/dummy DID for testing
        parent,
        body_cid,
        signature: vec![0u8; 64], // Dummy signature
        timestamp: chrono::Utc::now().timestamp(), // Use chrono from dependencies
    }
}

#[tokio::test]
async fn test_thread_append_cursor_anchor() { // Test name from user blueprint, adjusted slightly
    let store = Arc::new(InMemoryStorage::default());
    // Need a valid CID for the thread ID. Create using the helper from icn_core_types.
    let thread_id = Cid::from_bytes(b"thread-id").expect("Failed to create genesis CID");
    let thread = AgoraThread::new(thread_id, store.clone());

    let mut last_cid_opt: Option<Cid> = None; // Store Option<Cid> to hold cloned CIDs
    let mut actual_message_cids = Vec::new();
    let mut actual_anchor_cids = Vec::new();
    const ANCHOR_EVERY: usize = 25; // Match the constant in thread.rs

    // Append 30 messages
    for i in 0..30 {
        let msg_text = format!("msg{}", i);
        // Pass cloned last_cid_opt to create_dummy_message
        let message = create_dummy_message(store.clone(), msg_text.clone(), last_cid_opt.clone()).await;
        let (msg_cid, anchor_cid_opt) = thread.append(message).await.unwrap();

        // Clone msg_cid for multiple uses
        let current_msg_cid = msg_cid.clone();
        actual_message_cids.push(current_msg_cid.clone());
        last_cid_opt = Some(current_msg_cid.clone());

        // Check anchor triggering
        if (i + 1) % ANCHOR_EVERY == 0 {
            assert!(
                anchor_cid_opt.is_some(),
                "Anchor should trigger at message {}",
                i + 1
            );
            if let Some(anchor_cid) = anchor_cid_opt {
                // Clone anchor_cid for multiple uses
                let current_anchor_cid = anchor_cid.clone();
                actual_anchor_cids.push(current_anchor_cid.clone());
                let anchor: ThreadAnchor = store
                    .get_ipld(&current_anchor_cid) // Use cloned version
                    .await
                    .unwrap()
                    .expect("Anchor CID not found in storage");
                assert_eq!(anchor.tail, current_msg_cid, "Anchor tail CID mismatch"); // Use cloned msg_cid
            }
        } else {
            assert!(
                anchor_cid_opt.is_none(),
                "Anchor should NOT trigger at message {}",
                i + 1
            );
        }
    }

    // Fetch the internal list of CIDs to compare against (needed because AgoraThread::message_cids is private)
    let internal_cids_from_thread = thread.get_message_cids(None).await.unwrap();
    assert_eq!(internal_cids_from_thread.len(), 30);
    assert_eq!(actual_anchor_cids.len(), 1, "Expected exactly one anchor to be created");

    // Cursor (get_message_cids) from index 25 yields 5 items
    let cursor_cids = thread.get_message_cids(Some(25)).await.unwrap();
    assert_eq!(cursor_cids.len(), 5);
    assert_eq!(cursor_cids, &actual_message_cids[25..]);

    // Cursor (get_message_cids) from index 0 yields all items
    let all_cids_from_cursor = thread.get_message_cids(None).await.unwrap();
    assert_eq!(all_cids_from_cursor.len(), 30);
    assert_eq!(all_cids_from_cursor, actual_message_cids);

    // Test get_message
    let first_msg_cid_to_fetch = actual_message_cids[0].clone();
    let fetched_msg_opt = thread.get_message(&first_msg_cid_to_fetch).await.unwrap();
    assert!(fetched_msg_opt.is_some());
    let fetched_msg = fetched_msg_opt.unwrap();

    // Fetch the body to compare content
    let expected_body_cid = create_dummy_message(store.clone(), "msg0".to_string(), None).await.body_cid;
    let expected_body: Body = store.get_ipld(&expected_body_cid).await.unwrap().unwrap();
    let actual_body: Body = store.get_ipld(&fetched_msg.body_cid).await.unwrap().unwrap();
    assert_eq!(actual_body, expected_body);
} 