#![allow(unused_imports)] // Allow while prototyping
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};
use icn_core_types::Did;

// Use conditional compilation based on the 'agora' feature
#[cfg(feature = "agora")]
use {
    agoranet_core::{
        forward_anchor,
        message::{Body, CommentBody, Message},
        storage::{AsyncStorage, InMemoryStorage},
        thread::{AgoraThread, ThreadOperations},
    },
    icn_core_types::Cid,
    tokio::sync::RwLock, // For async RwLock on storage map
};

/// Commands for interacting with AgoraNet threads.
#[derive(Parser, Debug, Clone)]
#[cfg(feature = "agora")]
pub struct AgoraCmd {
    #[clap(subcommand)]
    pub command: AgoraSubcommand,
}

#[derive(Subcommand, Debug, Clone)]
#[cfg(feature = "agora")]
pub enum AgoraSubcommand {
    /// Manage AgoraNet threads.
    Thread { #[clap(subcommand)] action: ThreadCmd },
}

/// Thread management actions.
#[derive(Subcommand, Debug, Clone)]
#[cfg(feature = "agora")]
pub enum ThreadCmd {
    /// Create a new discussion thread.
    Create { 
        /// Title or short description for the new thread.
        title: String 
    },
    /// Post a message to an existing thread.
    Post { 
        /// CID of the thread to post to.
        thread_cid: String, 
        /// Path to a file containing the message body (UTF-8 text).
        body: PathBuf 
    },
    /// List message CIDs in a thread.
    Cursor { 
        /// CID of the thread to view.
        thread_cid: String, 
        /// Optional starting index (0-based).
        #[clap(long)]
        from: Option<usize> 
    },
}

// --- Placeholder Storage --- 
// In a real CLI, this would use RocksDB or similar, keyed by thread_id.
// Using Arc<RwLock<...>> for async access simulation.
#[cfg(feature = "agora")]
type ThreadStore = Arc<RwLock<HashMap<Cid, Arc<InMemoryStorage>>>>;

#[cfg(feature = "agora")]
lazy_static::lazy_static! {
    static ref IN_MEMORY_THREAD_STORE: ThreadStore = Arc::new(RwLock::new(HashMap::new()));
}

#[cfg(feature = "agora")]
async fn get_or_create_store(thread_id: &Cid) -> Arc<InMemoryStorage> {
    let mut store_map = IN_MEMORY_THREAD_STORE.write().await;
    store_map.entry(thread_id.clone()).or_insert_with(|| Arc::new(InMemoryStorage::new())).clone()
}

// --- Async Handlers --- 

#[cfg(feature = "agora")]
pub async fn handle_agora_cmd(cmd: AgoraCmd) -> Result<()> {
    match cmd.command {
        AgoraSubcommand::Thread { action } => handle_thread_cmd(action).await,
    }
}

#[cfg(feature = "agora")]
async fn handle_thread_cmd(action: ThreadCmd) -> Result<()> {
    match action {
        ThreadCmd::Create { title } => {
            // 1. Generate a unique ID for the thread.
            // In reality, this might be based on the creator's DID and a nonce,
            // or derived from the first message/proposal content.
            // For now, generate from title + timestamp for basic uniqueness.
            let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let data = format!("{}:{}", title, timestamp);
            let thread_id = Cid::from_bytes(data.as_bytes())?;
            
            println!("Creating new AgoraNet thread...");
            println!("Title: {}", title);
            println!("Thread ID (CID): {}", thread_id);

            // 2. Optionally, create a genesis message/body (e.g., Proposal with title)
            // let store = get_or_create_store(&thread_id).await;
            // let genesis_body = Body::Proposal(ProposalBody { title, description: "Genesis".to_string(), ..Default::default() });
            // let genesis_body_cid = store.put_ipld(&genesis_body).await?;
            // let genesis_message = Message { ... body_cid: genesis_body_cid, ... };
            // let thread = AgoraThread::new(thread_id.clone(), store);
            // let (genesis_msg_cid, _) = thread.append(genesis_message).await?;
            // println!("Genesis Message CID: {}", genesis_msg_cid);
            
            // For now, just print the ID. User needs to use this ID for `post`.
            // A real CLI would persist this locally (e.g., in a config file or simple DB)
            // mapped to a user-friendly name or the title.
            
            Ok(())
        }
        ThreadCmd::Post { thread_cid, body } => {
            let cid = Cid::from_str(&thread_cid)
                .map_err(|e| anyhow!("Invalid Thread CID: {}", e))?;
            
            let store = get_or_create_store(&cid).await;
            // We need an AgoraThread instance. In a real app, we might load its state 
            // (message CIDs, last anchor) from the store if persisted.
            // For this stub, we create a new one each time, losing history across calls.
            // TODO: Implement state persistence/loading for AgoraThread.
            let thread = AgoraThread::new(cid.clone(), store.clone()); 

            let msg_content = fs::read_to_string(body)?;
            
            // Create message body (assume Comment for now)
            let msg_body = Body::Comment(CommentBody { text: msg_content });
            let body_cid = store.put_ipld(&msg_body).await?;
            
            // Create message envelope
            let message = Message {
                // TODO: Use actual identity/key from CLI config/wallet
                author: Did::default(), 
                parent: None, // TODO: Get actual last message CID to form thread
                body_cid: body_cid.clone(),
                signature: vec![], // TODO: Sign canonical bytes
                timestamp: chrono::Utc::now().timestamp(),
            };

            println!("Posting message to thread: {}", cid);
            let (msg_envelope_cid, anchor_cid_opt) = thread.append(message).await?;
            
            println!("  Message Body CID: {}", body_cid);
            println!("  Message Envelope CID: {}", msg_envelope_cid);

            if let Some(anchor_cid) = anchor_cid_opt {
                println!("** Anchor emitted: {} **", anchor_cid);
                // Call the forwarder stub
                if let Err(e) = forward_anchor(&anchor_cid).await {
                    eprintln!("Warning: Failed to forward anchor: {}", e);
                }
            }
            Ok(())
        }
        ThreadCmd::Cursor { thread_cid, from } => {
            let cid = Cid::from_str(&thread_cid)
                .map_err(|e| anyhow!("Invalid Thread CID: {}", e))?;

            // As with post, we create a new thread instance here, losing history.
            // TODO: Implement state persistence/loading for AgoraThread.
            let store = get_or_create_store(&cid).await;
            let thread = AgoraThread::new(cid.clone(), store);
            
            println!("Messages in thread {}:", cid);
            let message_cids = thread.get_message_cids(from).await?;

            if message_cids.is_empty() {
                println!("  (No messages found from index {})", from.unwrap_or(0));
            } else {
                for (i, msg_cid) in message_cids.iter().enumerate() {
                    println!("  [{}]: {}", from.unwrap_or(0) + i, msg_cid);
                }
            }
            Ok(())
        }
    }
}

// Need to handle the case where the feature is not enabled
#[cfg(not(feature = "agora"))]
pub async fn handle_agora_cmd(_cmd: AgoraCmd) -> Result<()> {
    Err(anyhow!("AgoraNet feature not enabled during compilation."))
}

// Dummy structs/types if feature is not enabled, to satisfy main.rs
#[cfg(not(feature = "agora"))]
#[derive(Parser, Debug)]
pub struct AgoraCmd { pub command: Option<String> }

#[cfg(not(feature = "agora"))]
pub enum AgoraSubcommand { Thread { action: ThreadCmd } }

#[cfg(not(feature = "agora"))]
pub enum ThreadCmd { Create, Post, Cursor } 