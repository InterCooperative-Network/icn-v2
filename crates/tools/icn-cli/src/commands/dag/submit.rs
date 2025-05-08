use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::SystemTime, time::UNIX_EPOCH};
use tokio;

// Use the actual icn_types crate
use icn_types::dag::signed::{
    DagNode, SignedDagNode, DagPayload, DagError, // Assuming DagError is needed for error handling
};
use icn_types::{Did, Cid}; // Assuming Cid and Did are top-level exports from icn-types

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc; // For timestamp

// Placeholder for a local representation of the input JSON file
#[derive(Debug, Serialize, Deserialize)]
struct RawDagNodeInput {
    payload: serde_json::Value, // Flexible for now, could be a specific enum (e.g. base64 encoded string for RawData)
    author_did_override: Option<String>, // Optional: if not provided, DID from signing key is used
    // Parents might be specified as an array of CID strings
    // parents: Option<Vec<String>>,
}

#[derive(Args, Debug)]
pub struct DagSubmitArgs {
    /// Path to the JSON file defining the DAG node content.
    #[clap(long, short = 'n', value_parser)]
    node_content: PathBuf,

    /// URL of the ICN node's HTTP API endpoint.
    #[clap(long, short = 'u', default_value = "http://127.0.0.1:8080")]
    url: String,

    /// Path to the Ed25519 private key file (32 bytes, raw binary format).
    /// If not provided, a new key will be generated for development/testing.
    #[clap(long, short = 'k')]
    key_path: Option<PathBuf>,
}

/// Represents the JSON payload sent to the /dag/submit endpoint.
#[derive(Debug, Serialize)]
struct HttpSubmitPayload {
    encoded: String,
}

/// Helper to get current Unix timestamp in milliseconds.
fn now_ts_millis() -> i64 {
    Utc::now().timestamp_millis()
}

/// Placeholder for constructing a DID from a public key.
/// This should ideally come from icn-identity or icn-core-types.
fn did_from_verifying_key(key: &ed25519_dalek::VerifyingKey) -> Did {
    // Example: "did:key:z" + multibase_encoded_public_key
    // This is a simplified placeholder.
    let pk_bytes = key.as_bytes();
    let did_string = format!("did:key:z{}", multibase::Base::Base58Btc.encode(pk_bytes));
    Did::parse(&did_string).expect("Failed to parse placeholder DID from key") // Assumes Did::parse exists
}

pub async fn handle_dag_submit(args: DagSubmitArgs) -> Result<()> {
    println!(
        "Submitting node from file: {:?} to URL: {}",
        args.node_content, args.url
    );

    // 1. Read and parse the node content file
    let node_file_content = fs::read_to_string(&args.node_content)
        .with_context(|| format!("Failed to read node content file: {:?}", args.node_content))?;
    
    let raw_input: RawDagNodeInput = serde_json::from_str(&node_file_content)
        .with_context(|| format!("Failed to parse JSON from node content file: {:?}", args.node_content))?;

    // 2. Load/Generate SigningKey
    let sk: SigningKey = if let Some(ref key_path) = args.key_path {
        let key_bytes = fs::read(key_path)
            .with_context(|| format!("Failed to read private key file: {:?}", key_path))?;
        SigningKey::from_bytes(&key_bytes.try_into().map_err(|_| 
            anyhow!("Invalid private key length: expected 32 bytes, found {}", key_bytes.len())
        )?)
    } else {
        println!("Warning: No key path provided. Generating a new Ed25519 key for this submission (DEV ONLY).");
        SigningKey::generate(&mut OsRng)
    };
    let vk = sk.verifying_key();

    // 3. Determine Signer DID
    // Use author_did_override if provided, otherwise derive from the signing key.
    let signer_did: Did = match raw_input.author_did_override {
        Some(did_str) => Did::parse(&did_str)
            .map_err(|e| anyhow!("Invalid author_did_override '{}': {}", did_str, e))?,
        None => did_from_verifying_key(&vk), // Use our placeholder helper
    };
    println!("Node will be signed by DID: {}", signer_did);

    // 4. Construct DagNode (from icn-types)
    // This needs to map raw_input.payload to an appropriate DagPayload variant.
    // For this example, assuming RawData with base64 encoded bytes in the JSON.
    let payload_bytes = if let Some(s) = raw_input.payload.as_str() {
        base64::engine::general_purpose::STANDARD.decode(s)
            .with_context(|| format!("Payload string is not valid base64: {}", s))?
    } else {
        // Default to serializing the JSON value directly as bytes if not a string
        // This might not be what you want for structured payloads, adjust as needed.
        raw_input.payload.to_string().into_bytes()
    };
    
    let dag_node_payload = DagPayload::RawData { bytes: payload_bytes };
    
    let dag_node = DagNode {
        payload: dag_node_payload,
        author: signer_did.clone(), // Author is the signer in this simple case
        timestamp: now_ts_millis(),
    };
    println!("Constructed DagNode: {:?}", dag_node);

    // 5. Sign the DagNode to create SignedDagNode
    let signed_node = SignedDagNode::sign(dag_node, &sk, signer_did)
        .map_err(|e| anyhow!("Failed to sign DagNode: {:?}", e))?;
    println!("Constructed SignedDagNode, CID: {}", signed_node.cid);

    // 6. Serialize SignedDagNode to DAG-CBOR
    let cbor_bytes = serde_ipld_dagcbor::to_vec(&signed_node)
        .context("Failed to serialize SignedDagNode to DAG-CBOR")?;

    // 7. Base64 encode the CBOR bytes
    let encoded_payload_str = base64::engine::general_purpose::STANDARD.encode(&cbor_bytes);

    // 8. Prepare HTTP request payload
    let http_payload = HttpSubmitPayload {
        encoded: encoded_payload_str,
    };

    // 9. Make the HTTP POST request
    let client = Client::new();
    let endpoint = format!("{}/dag/submit", args.url.trim_end_matches('/'));
    
    println!("Sending POST request to: {}", endpoint);

    let response = client
        .post(&endpoint)
        .json(&http_payload)
        .send()
        .await
        .with_context(|| format!("Failed to send request to {}", endpoint))?;

    let response_status = response.status();
    let response_text = response
        .text()
        .await
        .with_context("Failed to read response text")?;

    println!("Response Status: {}", response_status);
    println!("Response Body: {}", response_text);

    if response_status.is_success() {
        Ok(())
    } else {
        Err(anyhow!(
            "Server responded with error {}: {}",
            response_status,
            response_text
        ))
    }
}

// The placeholder icn_types module has been removed as we are using the actual crate.

// TODO: Implement `load_private_key` and associated types/logic
// fn load_private_key(key_path: Option<PathBuf>) -> Result<Box<dyn Signer>> { ... }
// trait Signer {
//     fn sign(&self, data: &[u8]) -> Result<Vec<u8>>;
//     fn to_public_did(&self) -> Result<Did>;
// } 