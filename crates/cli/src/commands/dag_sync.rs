use crate::config::Config;
use clap::{Args, Subcommand};
use icn_types::{
    dag::{DagStore, RocksDbDagStore},
    dag::sync::{
        NetworkDagSyncService, SyncPolicy, TransportConfig,
        transport::libp2p::Libp2pDagTransport,
    },
    identity::Did,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Args)]
pub struct DagSyncArgs {
    #[command(subcommand)]
    command: DagSyncCommands,
}

#[derive(Subcommand)]
pub enum DagSyncCommands {
    /// Connect to a specific peer
    Connect {
        /// The peer multiaddress to connect to
        #[arg(long)]
        peer: String,
        
        /// The federation ID to use
        #[arg(long)]
        federation: String,
    },
    
    /// Start auto-sync mode to discover and sync with peers
    AutoSync {
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Enable mDNS discovery
        #[arg(long, default_value = "true")]
        mdns: bool,
        
        /// Enable Kademlia DHT discovery
        #[arg(long, default_value = "false")]
        kad_dht: bool,
        
        /// Comma-separated list of bootstrap peers
        #[arg(long, value_delimiter = ',')]
        bootstrap_peers: Option<Vec<String>>,
        
        /// Comma-separated list of authorized DIDs
        #[arg(long, value_delimiter = ',')]
        authorized_dids: Option<Vec<String>>,
        
        /// Minimum number of peers required for quorum
        #[arg(long, default_value = "1")]
        min_quorum: usize,
    },
    
    /// Offer nodes to a peer
    Offer {
        /// The peer ID to offer nodes to
        #[arg(long)]
        peer: String,
        
        /// The federation ID to use
        #[arg(long)]
        federation: String,
        
        /// Maximum number of nodes to offer
        #[arg(long, default_value = "100")]
        max_nodes: usize,
    },
}

impl DagSyncArgs {
    pub async fn execute(&self, config: &Config) -> anyhow::Result<()> {
        match &self.command {
            DagSyncCommands::Connect { peer, federation } => {
                // Create storage path
                let storage_path = config.storage_dir.join("dag").join(federation);
                std::fs::create_dir_all(&storage_path)?;
                
                // Create DAG store
                let store = Arc::new(RocksDbDagStore::new(storage_path)?);
                
                // Create transport config
                let transport_config = TransportConfig {
                    peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                    federation_id: federation.clone(),
                    local_did: config.identity.did.clone(),
                    listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                    bootstrap_peers: vec![peer.clone()],
                    enable_mdns: true,
                    enable_kad_dht: false,
                    max_message_size: 1024 * 1024, // 1MB
                    request_timeout: 30, // 30 seconds
                };
                
                // Create the transport
                let transport = Libp2pDagTransport::new(transport_config).await?;
                
                // Create the sync service
                let sync_service = NetworkDagSyncService::new(
                    transport,
                    store,
                    federation.clone(),
                    config.identity.did.clone(),
                );
                
                // Parse the peer string into a FederationPeer
                let peer_parts: Vec<&str> = peer.split('/').collect();
                let peer_id = peer_parts.last().unwrap_or(&"").to_string();
                
                let federation_peer = icn_types::dag::sync::FederationPeer {
                    id: peer_id,
                    endpoint: peer.clone(),
                    federation_id: federation.clone(),
                    metadata: None,
                };
                
                // Connect to the peer
                sync_service.connect_peer(&federation_peer).await?;
                println!("Connected to peer: {}", peer);
                
                // Start background sync
                sync_service.start_background_sync().await?;
                println!("Background sync started");
                
                // Keep the process running
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                
                #[allow(unreachable_code)]
                Ok(())
            },
            
            DagSyncCommands::AutoSync { 
                federation, 
                mdns, 
                kad_dht, 
                bootstrap_peers,
                authorized_dids,
                min_quorum,
            } => {
                // Create storage path
                let storage_path = config.storage_dir.join("dag").join(federation);
                std::fs::create_dir_all(&storage_path)?;
                
                // Create DAG store
                let store = Arc::new(RocksDbDagStore::new(storage_path)?);
                
                // Create transport config
                let transport_config = TransportConfig {
                    peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                    federation_id: federation.clone(),
                    local_did: config.identity.did.clone(),
                    listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                    bootstrap_peers: bootstrap_peers.clone().unwrap_or_default(),
                    enable_mdns: *mdns,
                    enable_kad_dht: *kad_dht,
                    max_message_size: 1024 * 1024, // 1MB
                    request_timeout: 30, // 30 seconds
                };
                
                // Create the transport
                let transport = Libp2pDagTransport::new(transport_config).await?;
                
                // Create sync policy
                let mut policy = SyncPolicy::default();
                policy.min_quorum = *min_quorum;
                
                // Set authorized DIDs if provided
                if let Some(dids) = authorized_dids {
                    let did_set: HashSet<Did> = dids.iter()
                        .filter_map(|d| Did::from_str(d).ok())
                        .collect();
                    
                    if !did_set.is_empty() {
                        policy.authorized_dids = Some(did_set);
                    }
                }
                
                // Create the sync service with the policy
                let sync_service = NetworkDagSyncService::new(
                    transport,
                    store,
                    federation.clone(),
                    config.identity.did.clone(),
                ).with_policy(policy);
                
                // Start background sync
                sync_service.start_background_sync().await?;
                println!("Background sync started");
                
                // Discover peers periodically
                let sync_service_clone = sync_service.clone();
                tokio::spawn(async move {
                    loop {
                        match sync_service_clone.discover_peers().await {
                            Ok(peers) => {
                                println!("Discovered {} peers", peers.len());
                                for peer in peers {
                                    if let Err(e) = sync_service_clone.connect_peer(&peer).await {
                                        eprintln!("Failed to connect to peer {}: {:?}", peer.id, e);
                                    } else {
                                        println!("Connected to peer: {}", peer.id);
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("Error discovering peers: {:?}", e);
                            }
                        }
                        
                        // Wait before next discovery
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    }
                });
                
                // Keep the process running
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                
                #[allow(unreachable_code)]
                Ok(())
            },
            
            DagSyncCommands::Offer { peer, federation, max_nodes } => {
                // Create storage path
                let storage_path = config.storage_dir.join("dag").join(federation);
                
                // Create DAG store
                let store = Arc::new(RocksDbDagStore::new(storage_path)?);
                
                // Create transport config
                let transport_config = TransportConfig {
                    peer_id: uuid::Uuid::new_v4().to_string(), // Generate random peer ID
                    federation_id: federation.clone(),
                    local_did: config.identity.did.clone(),
                    listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()], // Random port
                    bootstrap_peers: vec![],
                    enable_mdns: true,
                    enable_kad_dht: false,
                    max_message_size: 1024 * 1024, // 1MB
                    request_timeout: 30, // 30 seconds
                };
                
                // Create the transport
                let transport = Libp2pDagTransport::new(transport_config).await?;
                
                // Create the sync service
                let sync_service = NetworkDagSyncService::new(
                    transport,
                    store.clone(),
                    federation.clone(),
                    config.identity.did.clone(),
                );
                
                // Get nodes from the store (limited by max_nodes)
                let cids = store.list_cids(*max_nodes).await?;
                
                if cids.is_empty() {
                    println!("No nodes available to offer");
                    return Ok(());
                }
                
                println!("Offering {} nodes to peer {}", cids.len(), peer);
                
                // Offer nodes to the peer
                match sync_service.offer_nodes(peer, &cids).await {
                    Ok(requested_cids) => {
                        println!("Peer requested {} nodes", requested_cids.len());
                        
                        if !requested_cids.is_empty() {
                            // Fetch the requested nodes from our store
                            let mut nodes = Vec::new();
                            for cid in &requested_cids {
                                if let Ok(Some(node)) = store.get(cid).await {
                                    nodes.push(node);
                                }
                            }
                            
                            // Create a bundle and send it
                            if !nodes.is_empty() {
                                sync_service.broadcast_nodes(&nodes).await?;
                                println!("Sent {} nodes to peer", nodes.len());
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to offer nodes: {:?}", e);
                    }
                }
                
                Ok(())
            },
        }
    }
}

// Helper to parse a Did from a string
impl Did {
    fn from_str(s: &str) -> Result<Self, String> {
        // Simple validation - in a real implementation we'd do more
        if s.starts_with("did:") {
            Ok(Did::new(s.to_string()))
        } else {
            Err(format!("Invalid DID format: {}", s))
        }
    }
} 