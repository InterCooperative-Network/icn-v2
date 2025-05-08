#![allow(unused_imports)] // Allow while prototyping
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc},
};
use icn_core_types::Did;

// Use conditional compilation based on the 'agora' feature
#[cfg(feature = "agora")]
use {
    agoranet_core::{
        forward_anchor,
        message::{Body, CommentBody, Message},
        storage::{AsyncStorage, InMemoryStorage, StorageBackend},
        thread::{AgoraThread, ThreadOperations},
    },
    icn_core_types::Cid,
};

// Dependencies needed for default_store
#[cfg(feature = "persistence")]
use {
    agoranet_core::storage::rocks::RocksDbStorage,
    once_cell::sync::Lazy,
    dirs,
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

// --- Storage Initialization --- 

// Global store, initialized once using Lazy.
// Chooses RocksDB if 'persistence' feature is enabled, otherwise InMemory.

#[cfg(all(feature = "agora", feature = "persistence"))]
fn default_store() -> Arc<StorageBackend> {
    static DB: Lazy<Arc<StorageBackend>> = Lazy::new(|| {
        let home = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".")) // Fallback to current dir if needed
            .join("icn")
            .join("agora_threads");
        println!("Using RocksDB store at: {}", home.display()); // Log path
        let rocks = RocksDbStorage::open(&home).expect("Failed to open RocksDB store");
        Arc::new(StorageBackend::Rocks(rocks))
    });
    DB.clone()
}

#[cfg(all(feature = "agora", not(feature = "persistence")))]
fn default_store() -> Arc<StorageBackend> {
    // If persistence is not enabled, use InMemory
    // Note: This InMemory store will be new for each CLI invocation.
    println!("Using ephemeral InMemory store.");
    let store = InMemoryStorage::new();
    Arc::new(StorageBackend::InMemory(store))
}

// --- Async Handlers --- 

#[cfg(feature = "agora")]
pub async fn handle_agora_cmd(subcommand: AgoraSubcommand) -> Result<()> {
    // Get the appropriate storage backend (RocksDB or InMemory)
    let store = default_store();

    match subcommand {
        AgoraSubcommand::Thread { action } => handle_thread_cmd(action, store).await,
        // Add other Agora subcommands here if any
    }
}

#[cfg(feature = "agora")]
async fn handle_thread_cmd(action: ThreadCmd, store: Arc<StorageBackend>) -> Result<()> {
    match action {
        ThreadCmd::Create { title } => {
            // 1. Generate Thread ID (same as before)
            let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let data = format!("{}:{}", title, timestamp);
            let thread_id = Cid::from_bytes(data.as_bytes())?;
            
            println!("Creating new AgoraNet thread...");
            println!("Title: {}", title);
            println!("Thread ID (CID): {}", thread_id);
            
            // No need to explicitly interact with the store here for creation itself.
            // The thread only exists conceptually until messages are added.
            // RocksDB store path is logged via default_store().

            // A real CLI might save a mapping of title -> thread_id locally.
            Ok(())
        }
        ThreadCmd::Post { thread_cid, body } => {
            let cid = Cid::from_str(&thread_cid)
                .map_err(|e| anyhow!("Invalid Thread CID: {}", e))?;
            
            // Create AgoraThread instance using the shared store
            let thread = AgoraThread::new(cid.clone(), store.clone()); 

            let msg_content = fs::read_to_string(body)?; // Read message file
            
            // Create message body
            let msg_body = CommentBody { text: msg_content };
            // Use the thread's post_comment method
            let (message_cid, anchor_cid_opt) = thread.post_comment(msg_body).await?;
            
            println!("Posted message to thread: {}", cid);
            println!("Message CID: {}", message_cid); // Print only the message CID
            
            // Check if an anchor was emitted and print it
            if let Some(anchor_cid) = anchor_cid_opt {
                println!("** Anchor emitted: {} **", anchor_cid);
                // Optional: Forward the anchor (if forward_anchor is available and needed here)
                // if let Err(e) = forward_anchor(&anchor_cid).await {
                //     eprintln!("Warning: Failed to forward anchor: {}", e);
                // }
            }
            Ok(())
        }
        ThreadCmd::Cursor { thread_cid, from } => {
            let cid = Cid::from_str(&thread_cid)
                .map_err(|e| anyhow!("Invalid Thread CID: {}", e))?;

            // Create AgoraThread instance using the shared store
            let thread = AgoraThread::new(cid.clone(), store.clone());
            
            println!("Messages in thread {}:", cid);
            // Get messages using the thread instance
            // NOTE: This relies on AgoraThread::append *persisting* the message CID list somewhere
            // If AgoraThread::append only adds to an in-memory list within the struct, 
            // this cursor won't work across CLI calls without further changes to AgoraThread.
            // Assuming for now that append *does* update persistent state accessible via get_message_cids.
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

// Handler for when 'agora' feature is NOT enabled
#[cfg(not(feature = "agora"))]
pub async fn handle_agora_cmd(_subcommand: AgoraSubcommand) -> Result<()> {
    Err(anyhow!("AgoraNet feature not enabled during compilation."))
}

// Dummy structs needed if feature is not enabled, to satisfy cli.rs
#[cfg(not(feature = "agora"))]
#[derive(Parser, Debug, Clone)]
pub struct AgoraCmd {}

#[cfg(not(feature = "agora"))]
#[derive(Subcommand, Debug, Clone)]
pub enum AgoraSubcommand { Thread }

#[cfg(not(feature = "agora"))]
#[derive(Subcommand, Debug, Clone)]
pub enum ThreadCmd { Create, Post, Cursor } 