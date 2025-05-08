use serde::Deserialize;
use std::path::PathBuf;

// Updated FederationConfig structure
#[derive(Deserialize, Debug, Clone)]
pub struct FederationConfig {
    pub federation_did: String, // Canonical federation DID
    pub storage_path: Option<PathBuf>, // Optional base path for storage
    pub metadata: FederationMetadata,
    pub node: NodeConfig,
    pub network: NetworkConfig,
    pub dag_store: DagStoreConfig,
    // Add other sections like runtime, api, etc.
}

#[derive(Deserialize, Debug, Clone)]
pub struct FederationMetadata {
    pub name: String,
    // Removed 'did' field from here, using top-level federation_did
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeConfig {
    pub keys_path: Option<PathBuf>, // Path to node's keypair for libp2p, etc.
    // Other node-specific settings
}

#[derive(Deserialize, Debug, Clone)]
pub struct NetworkConfig {
    pub listen_address: String, // e.g., /ip4/0.0.0.0/tcp/0
    pub bootstrap_peers: Vec<String>,
    // Other network settings like pubsub topics, Kademlia config
}

#[derive(Deserialize, Debug, Clone)]
pub struct DagStoreConfig {
    pub path: PathBuf, // Path for RocksDB or other persistent store
    // Other store-specific settings
}


// Minimal CliArgs for icn-node main.rs, expand as needed
// If icn-node uses clap directly, this might be more complex.
#[derive(Debug)]
pub struct CliArgs {
    pub config_path: String,
    // Other global CLI args for the node service if any
}

impl CliArgs {
    pub fn parse() -> Self {
        // Basic parsing, ideally use clap or similar if icn-node has its own CLI args
        // For this iteration, we'll keep it simple as in the main.rs placeholder.
        let config_path = std::env::var("ICN_NODE_CONFIG_PATH")
            .unwrap_or_else(|_| "federation_icn.toml".to_string());
        Self { config_path }
    }
}

// Basic config loader
pub fn load_federation_config(path: &str) -> anyhow::Result<FederationConfig> {
    let config_content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file from {}: {}", path, e))?;
    let config: FederationConfig = toml::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse TOML config from {}: {}", path, e))?;
    Ok(config)
}

// Example of how a fixture config file might look (tests/fixtures/federation_icn.toml)
/*
[metadata]
name = "TestFederation"
did = "did:example:testfed123"

[node]
keys_path = "node_keys.json"

[network]
listen_address = "/ip4/0.0.0.0/tcp/0"
bootstrap_peers = []

[dag_store]
path = "./test_federation_data/dag_store"
*/ 