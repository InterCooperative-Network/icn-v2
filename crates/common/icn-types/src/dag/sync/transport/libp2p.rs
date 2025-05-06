#![cfg(feature = "networking")]

use crate::cid::Cid;
use crate::dag::sync::{DAGSyncBundle, FederationPeer, SyncError};
use crate::dag::sync::transport::{DAGSyncMessage, DAGSyncTransport, TransportConfig, DAG_SYNC_PROTOCOL_ID};
use crate::identity::Did;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;
use std::sync::{Arc, Mutex};

use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{self, Gossipsub, GossipsubEvent, MessageId, Topic},
    identify,
    identity::Keypair,
    kad::{self, Kademlia, KademliaEvent},
    mdns::{Mdns, MdnsEvent},
    noise,
    request_response::{
        self, ProtocolSupport, RequestResponse, RequestResponseEvent, RequestResponseMessage,
        ResponseChannel,
    },
    swarm::{SwarmEvent, NetworkBehaviour},
    tcp, Multiaddr, PeerId, Swarm,
};
use tokio::sync::mpsc;

// The behavior type for libp2p
#[derive(NetworkBehaviour)]
struct DagSyncBehaviour {
    gossipsub: Gossipsub,
    kademlia: Kademlia,
    request_response: RequestResponse<DagSyncCodec>,
    mdns: Mdns,
    identify: identify::Behaviour,
}

/// Custom codec for DAG sync request/response
#[derive(Clone)]
struct DagSyncCodec;

impl request_response::Codec for DagSyncCodec {
    type Protocol = upgrade::Version<&'static str>;
    type Request = DAGSyncMessage;
    type Response = DAGSyncMessage;

    fn upgrade(&self) -> Self::Protocol {
        upgrade::Version::V1(DAG_SYNC_PROTOCOL_ID)
    }

    fn decode_request<B>(
        &mut self,
        bytes: B,
    ) -> Result<Self::Request, request_response::DecodeError>
    where
        B: AsRef<[u8]>,
    {
        serde_json::from_slice(bytes.as_ref())
            .map_err(|e| request_response::DecodeError::from(format!("Error decoding request: {}", e)))
    }

    fn decode_response<B>(
        &mut self,
        bytes: B,
    ) -> Result<Self::Response, request_response::DecodeError>
    where
        B: AsRef<[u8]>,
    {
        serde_json::from_slice(bytes.as_ref())
            .map_err(|e| request_response::DecodeError::from(format!("Error decoding response: {}", e)))
    }

    fn encode_request(
        &mut self,
        request: Self::Request,
    ) -> Result<Vec<u8>, request_response::EncodeError> {
        serde_json::to_vec(&request)
            .map_err(|e| request_response::EncodeError::from(format!("Error encoding request: {}", e)))
    }

    fn encode_response(
        &mut self,
        response: Self::Response,
    ) -> Result<Vec<u8>, request_response::EncodeError> {
        serde_json::to_vec(&response)
            .map_err(|e| request_response::EncodeError::from(format!("Error encoding response: {}", e)))
    }
}

// Collection of inbound message channels
struct InboundChannels {
    // Channel for receiving bundles from peers
    bundle_rx: mpsc::Receiver<(String, DAGSyncBundle)>,
    // Queue of bundles waiting to be processed
    pending_bundles: VecDeque<(String, DAGSyncBundle)>,
    // Channel for responses to offer messages
    offer_rx: mpsc::Receiver<(String, HashSet<Cid>)>,
    // Pending offer responses keyed by peer ID
    pending_offers: HashMap<String, Option<HashSet<Cid>>>,
    // Channel for responses to request messages
    request_rx: mpsc::Receiver<(String, DAGSyncBundle)>,
    // Pending request responses keyed by peer ID
    pending_requests: HashMap<String, Option<DAGSyncBundle>>,
    // Channel for discovered peers
    discovery_rx: mpsc::Receiver<Vec<FederationPeer>>,
    // Recently discovered peers
    discovered_peers: Vec<FederationPeer>,
}

// Add Clone implementation for InboundChannels
impl Clone for InboundChannels {
    fn clone(&self) -> Self {
        // Create a new receiver by using watch channels
        // Note: In a real implementation, we would use a proper shareable channel type
        // or move to using Arc<Mutex<>> for sharing the receivers
        Self {
            bundle_rx: mpsc::channel(100).1,
            pending_bundles: self.pending_bundles.clone(),
            offer_rx: mpsc::channel(100).1,
            pending_offers: self.pending_offers.clone(),
            request_rx: mpsc::channel(100).1,
            pending_requests: self.pending_requests.clone(),
            discovery_rx: mpsc::channel(100).1,
            discovered_peers: self.discovered_peers.clone(),
        }
    }
}

// Collection of outbound message channels
struct OutboundChannels {
    // Channel for sending bundles to the swarm
    bundle_tx: mpsc::Sender<(String, DAGSyncBundle)>,
    // Channel for sending offers to the swarm
    offer_tx: mpsc::Sender<(String, Vec<Cid>)>,
    // Channel for sending requests to the swarm
    request_tx: mpsc::Sender<(String, Vec<Cid>)>,
}

// Add Clone implementation for OutboundChannels
impl Clone for OutboundChannels {
    fn clone(&self) -> Self {
        Self {
            bundle_tx: self.bundle_tx.clone(),
            offer_tx: self.offer_tx.clone(),
            request_tx: self.request_tx.clone(),
        }
    }
}

// The main libp2p transport implementation
pub struct Libp2pDagTransport {
    // Libp2p keypair
    keypair: Keypair,
    // Local peer ID
    local_peer_id: String,
    // Local federation ID
    federation_id: String,
    // Local DID
    local_did: Option<Did>,
    // Network swarm task handle
    _swarm_task: Option<tokio::task::JoinHandle<()>>,
    // Connected peers
    connected_peers: Arc<Mutex<HashSet<String>>>,
    // Inbound message channels
    inbound: InboundChannels,
    // Outbound message channels
    outbound: OutboundChannels,
    // Topic for bundle announcements
    bundle_topic: Topic,
}

impl Libp2pDagTransport {
    /// Create a new libp2p transport with the given configuration
    pub async fn new(config: TransportConfig) -> Result<Self, SyncError> {
        // Generate a libp2p keypair
        let keypair = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public()).to_string();

        // Create channels for communication with the swarm
        let (bundle_tx, bundle_rx) = mpsc::channel(100);
        let (offer_tx, offer_rx) = mpsc::channel(100);
        let (request_tx, request_rx) = mpsc::channel(100);
        let (discovery_tx, discovery_rx) = mpsc::channel(100);
        let (inbound_bundle_tx, inbound_bundle_rx) = mpsc::channel(100);
        let (inbound_offer_tx, inbound_offer_rx) = mpsc::channel(100);
        let (inbound_request_tx, inbound_request_rx) = mpsc::channel(100);
        
        // Create the bundle topic using the federation ID
        let bundle_topic = Topic::new(format!("/icn/dag-sync/{}/bundles", config.federation_id));
        
        // Track connected peers
        let connected_peers = Arc::new(Mutex::new(HashSet::new()));
        let swarm_peers = connected_peers.clone();
        
        // Create and spawn the swarm task
        let swarm_task = tokio::spawn(async move {
            // Create the transport
            let transport = tcp::tokio::Transport::default()
                .upgrade(upgrade::Version::V1)
                .authenticate(noise::Config::new(&keypair).expect("Failed to create noise config"))
                .multiplex(libp2p::yamux::Config::default())
                .boxed();
            
            // Create the swarm components
            let mut swarm = {
                // Set up gossipsub
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .expect("Valid gossipsub config");
                    
                let gossipsub = Gossipsub::new(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config,
                )
                .expect("Failed to create gossipsub");
                
                // Set up Kademlia
                let mut kademlia = Kademlia::new(
                    PeerId::from(keypair.public()),
                    kad::store::MemoryStore::new(PeerId::from(keypair.public())),
                );
                
                // Set up request/response
                let request_response = RequestResponse::new(
                    DagSyncCodec {},
                    vec![(DAG_SYNC_PROTOCOL_ID, ProtocolSupport::Full)],
                    Default::default(),
                );
                
                // Set up mDNS
                let mdns = Mdns::new(Default::default())
                    .await
                    .expect("Failed to create mDNS");
                    
                // Set up identify
                let identify = identify::Behaviour::new(identify::Config::new(
                    "/icn/dag-sync/1.0.0".to_string(),
                    keypair.public(),
                ));
                
                // Create the behaviour
                let behaviour = DagSyncBehaviour {
                    gossipsub,
                    kademlia,
                    request_response,
                    mdns,
                    identify,
                };
                
                // Create the swarm
                Swarm::with_tokio_executor(transport, behaviour, PeerId::from(keypair.public()))
            };
            
            // Subscribe to the bundle topic
            swarm.behaviour_mut().gossipsub.subscribe(&bundle_topic).expect("Failed to subscribe to bundle topic");
            
            // Set up listening addresses
            for addr_str in &config.listen_addresses {
                match addr_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        if let Err(e) = swarm.listen_on(addr) {
                            eprintln!("Failed to listen on {}: {:?}", addr, e);
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to parse multiaddr {}: {:?}", addr_str, e);
                    }
                }
            }
            
            // Connect to bootstrap peers
            for peer_str in &config.bootstrap_peers {
                match peer_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        if let Err(e) = swarm.dial(addr) {
                            eprintln!("Failed to dial {}: {:?}", peer_str, e);
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to parse bootstrap peer {}: {:?}", peer_str, e);
                    }
                }
            }
            
            // Main event loop
            loop {
                tokio::select! {
                    // Handle events from the swarm
                    event = swarm.select_next_some() => {
                        match event {
                            SwarmEvent::Behaviour(behaviour_event) => {
                                match behaviour_event {
                                    // Handle gossipsub events
                                    libp2p::swarm::NetworkBehaviourEvent::Gossipsub(gossip_event) => {
                                        if let GossipsubEvent::Message { 
                                            propagation_source, 
                                            message_id: _, 
                                            message 
                                        } = gossip_event {
                                            // Process bundle messages
                                            match serde_json::from_slice::<DAGSyncMessage>(&message.data) {
                                                Ok(DAGSyncMessage::Bundle(bundle)) => {
                                                    let _ = inbound_bundle_tx.send((
                                                        propagation_source.to_string(), 
                                                        bundle
                                                    )).await;
                                                },
                                                Err(e) => {
                                                    eprintln!("Failed to deserialize gossip message: {:?}", e);
                                                },
                                                _ => {}
                                            }
                                        }
                                    },
                                    
                                    // Handle request/response events
                                    libp2p::swarm::NetworkBehaviourEvent::RequestResponse(req_resp_event) => {
                                        match req_resp_event {
                                            // Handle incoming requests
                                            RequestResponseEvent::Message { 
                                                peer, 
                                                message: RequestResponseMessage::Request { 
                                                    request, 
                                                    channel, .. 
                                                } 
                                            } => {
                                                match request {
                                                    // Handle offer requests
                                                    DAGSyncMessage::Offer { cids } => {
                                                        let _ = inbound_offer_tx.send((peer.to_string(), cids.into_iter().collect())).await;
                                                        // Respond with empty set for now - will be replaced by proper handling
                                                        let _ = swarm.behaviour_mut().request_response.send_response(
                                                            channel,
                                                            DAGSyncMessage::OfferResponse { cids: HashSet::new() }
                                                        );
                                                    },
                                                    // Handle node requests
                                                    DAGSyncMessage::Request { cids } => {
                                                        let _ = inbound_request_tx.send((peer.to_string(), cids.clone())).await;
                                                        // Respond with empty bundle for now - will be replaced by proper handling
                                                        let _ = swarm.behaviour_mut().request_response.send_response(
                                                            channel,
                                                            DAGSyncMessage::Bundle(DAGSyncBundle {
                                                                nodes: vec![],
                                                                federation_id: config.federation_id.clone(),
                                                                source_peer: Some(local_peer_id.clone()),
                                                                timestamp: chrono::Utc::now(),
                                                            })
                                                        );
                                                    },
                                                    _ => {}
                                                }
                                            },
                                            // Handle incoming responses
                                            RequestResponseEvent::Message {
                                                peer,
                                                message: RequestResponseMessage::Response {
                                                    response,
                                                    ..
                                                }
                                            } => {
                                                match response {
                                                    // Handle offer responses
                                                    DAGSyncMessage::OfferResponse { cids } => {
                                                        let _ = inbound_offer_tx.send((peer.to_string(), cids)).await;
                                                    },
                                                    // Handle bundle responses
                                                    DAGSyncMessage::Bundle(bundle) => {
                                                        let _ = inbound_request_tx.send((peer.to_string(), bundle)).await;
                                                    },
                                                    _ => {}
                                                }
                                            },
                                            // Handle outbound failures
                                            RequestResponseEvent::OutboundFailure { peer, request_id: _, error } => {
                                                eprintln!("Outbound request to {} failed: {:?}", peer, error);
                                            },
                                            // Handle inbound failures
                                            RequestResponseEvent::InboundFailure { peer, request_id: _, error } => {
                                                eprintln!("Inbound request from {} failed: {:?}", peer, error);
                                            },
                                            // Handle response send errors
                                            RequestResponseEvent::ResponseSent { peer, request_id: _ } => {
                                                // Response sent successfully
                                            }
                                        }
                                    },
                                    
                                    // Handle mDNS events for peer discovery
                                    libp2p::swarm::NetworkBehaviourEvent::Mdns(mdns_event) => {
                                        match mdns_event {
                                            MdnsEvent::Discovered(list) => {
                                                let mut newly_discovered = vec![];
                                                for (peer_id, multiaddr) in list {
                                                    // Add address to Kademlia
                                                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr.clone());
                                                    
                                                    // Add to discovered peers
                                                    let peer = FederationPeer {
                                                        id: peer_id.to_string(),
                                                        endpoint: multiaddr.to_string(),
                                                        federation_id: config.federation_id.clone(), // Assume same federation for now
                                                        metadata: None,
                                                    };
                                                    newly_discovered.push(peer);
                                                }
                                                
                                                if !newly_discovered.is_empty() {
                                                    let _ = discovery_tx.send(newly_discovered).await;
                                                }
                                            },
                                            MdnsEvent::Expired(list) => {
                                                for (peer_id, _) in list {
                                                    if let Ok(mut peers) = swarm_peers.lock() {
                                                        peers.remove(&peer_id.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    
                                    // Ignore other events for now
                                    _ => {}
                                }
                            },
                            
                            // Handle connection established events
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                if let Ok(mut peers) = swarm_peers.lock() {
                                    peers.insert(peer_id.to_string());
                                }
                            },
                            
                            // Handle connection closed events
                            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                if let Ok(mut peers) = swarm_peers.lock() {
                                    peers.remove(&peer_id.to_string());
                                }
                            },
                            
                            // Handle other events
                            _ => {}
                        }
                    },
                    
                    // Handle outbound bundle messages
                    Some((peer_id, bundle)) = bundle_rx.recv() => {
                        // Publish to gossipsub
                        let message = DAGSyncMessage::Bundle(bundle);
                        match serde_json::to_vec(&message) {
                            Ok(encoded) => {
                                match swarm.behaviour_mut().gossipsub.publish(bundle_topic.clone(), encoded) {
                                    Ok(_) => {
                                        // Published successfully
                                    },
                                    Err(e) => {
                                        eprintln!("Failed to publish bundle: {:?}", e);
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("Failed to encode bundle: {:?}", e);
                            }
                        }
                    },
                    
                    // Handle outbound offer messages
                    Some((peer_id, cids)) = offer_tx.recv() => {
                        // Send request to peer
                        if let Ok(peer_id) = peer_id.parse::<PeerId>() {
                            let message = DAGSyncMessage::Offer { cids };
                            match swarm.behaviour_mut().request_response.send_request(&peer_id, message) {
                                Ok(_) => {
                                    // Request sent successfully
                                },
                                Err(e) => {
                                    eprintln!("Failed to send offer request to {}: {:?}", peer_id, e);
                                }
                            }
                        }
                    },
                    
                    // Handle outbound request messages
                    Some((peer_id, cids)) = request_tx.recv() => {
                        // Send request to peer
                        if let Ok(peer_id) = peer_id.parse::<PeerId>() {
                            let message = DAGSyncMessage::Request { cids };
                            match swarm.behaviour_mut().request_response.send_request(&peer_id, message) {
                                Ok(_) => {
                                    // Request sent successfully
                                },
                                Err(e) => {
                                    eprintln!("Failed to send node request to {}: {:?}", peer_id, e);
                                }
                            }
                        }
                    }
                }
            }
        });
        
        // Create the inbound and outbound channels
        let inbound = InboundChannels {
            bundle_rx: inbound_bundle_rx,
            pending_bundles: VecDeque::new(),
            offer_rx: inbound_offer_rx,
            pending_offers: HashMap::new(),
            request_rx: inbound_request_rx,
            pending_requests: HashMap::new(),
            discovery_rx,
            discovered_peers: Vec::new(),
        };
        
        let outbound = OutboundChannels {
            bundle_tx,
            offer_tx,
            request_tx,
        };
        
        // Return the transport
        Ok(Self {
            keypair,
            local_peer_id,
            federation_id: config.federation_id,
            local_did: config.local_did,
            _swarm_task: Some(swarm_task),
            connected_peers,
            inbound,
            outbound,
            bundle_topic,
        })
    }
}

// Add Clone implementation
impl Clone for Libp2pDagTransport {
    fn clone(&self) -> Self {
        // Note: this is a simplistic clone that would require the caller to reconnect
        // In a production implementation, we would want to share the underlying
        // network connection state between clones
        Self {
            keypair: self.keypair.clone(),
            local_peer_id: self.local_peer_id.clone(),
            federation_id: self.federation_id.clone(),
            local_did: self.local_did.clone(),
            _swarm_task: None, // The cloned instance doesn't own the swarm task
            connected_peers: self.connected_peers.clone(),
            inbound: InboundChannels {
                bundle_rx: self.inbound.bundle_rx.clone(),
                pending_bundles: self.inbound.pending_bundles.clone(),
                offer_rx: self.inbound.offer_rx.clone(),
                pending_offers: self.inbound.pending_offers.clone(),
                request_rx: self.inbound.request_rx.clone(),
                pending_requests: self.inbound.pending_requests.clone(),
                discovery_rx: self.inbound.discovery_rx.clone(),
                discovered_peers: self.inbound.discovered_peers.clone(),
            },
            outbound: OutboundChannels {
                bundle_tx: self.outbound.bundle_tx.clone(),
                offer_tx: self.outbound.offer_tx.clone(),
                request_tx: self.outbound.request_tx.clone(),
            },
            bundle_topic: self.bundle_topic.clone(),
        }
    }
}

#[async_trait]
impl DAGSyncTransport for Libp2pDagTransport {
    async fn connect(&mut self, peer: &FederationPeer) -> Result<(), SyncError> {
        // Parse the endpoint as a multiaddr
        let addr = peer.endpoint.parse::<Multiaddr>()
            .map_err(|e| SyncError::NetworkError(format!("Failed to parse endpoint as multiaddr: {}", e)))?;
        
        // Send a request to the peer to check connection
        // (a real implementation would establish a connection via the swarm)
        
        // Add to connected peers if successful
        if let Ok(mut peers) = self.connected_peers.lock() {
            peers.insert(peer.id.clone());
        }
        
        Ok(())
    }
    
    async fn disconnect(&mut self, peer_id: &str) -> Result<(), SyncError> {
        // Remove from connected peers
        if let Ok(mut peers) = self.connected_peers.lock() {
            peers.remove(peer_id);
        }
        
        Ok(())
    }
    
    async fn is_connected(&self, peer_id: &str) -> Result<bool, SyncError> {
        if let Ok(peers) = self.connected_peers.lock() {
            Ok(peers.contains(peer_id))
        } else {
            Err(SyncError::NetworkError("Failed to check connection status".to_string()))
        }
    }
    
    async fn send_bundle(&mut self, peer_id: &str, bundle: DAGSyncBundle) -> Result<(), SyncError> {
        // Send via gossipsub to all peers
        self.outbound.bundle_tx.send((peer_id.to_string(), bundle)).await
            .map_err(|e| SyncError::NetworkError(format!("Failed to send bundle: {}", e)))
    }
    
    async fn receive_bundles(&mut self) -> Result<(String, DAGSyncBundle), SyncError> {
        // Check if we have any pending bundles
        if let Some((peer_id, bundle)) = self.inbound.pending_bundles.pop_front() {
            return Ok((peer_id, bundle));
        }
        
        // Wait for a bundle from the channel
        match self.inbound.bundle_rx.recv().await {
            Some((peer_id, bundle)) => Ok((peer_id, bundle)),
            None => Err(SyncError::NetworkError("Bundle channel closed".to_string())),
        }
    }
    
    async fn send_offer(&mut self, peer_id: &str, cids: &[Cid]) -> Result<HashSet<Cid>, SyncError> {
        // Clear any previous pending offers for this peer
        self.inbound.pending_offers.insert(peer_id.to_string(), None);
        
        // Send the offer
        self.outbound.offer_tx.send((peer_id.to_string(), cids.to_vec())).await
            .map_err(|e| SyncError::NetworkError(format!("Failed to send offer: {}", e)))?;
        
        // Wait for a response
        loop {
            // Check if we already have a response
            if let Some(Some(cids)) = self.inbound.pending_offers.remove(peer_id) {
                return Ok(cids);
            }
            
            // Wait for a response from the channel
            match self.inbound.offer_rx.recv().await {
                Some((resp_peer_id, cids)) => {
                    if resp_peer_id == peer_id {
                        return Ok(cids);
                    } else {
                        // Store the response for another peer
                        self.inbound.pending_offers.insert(resp_peer_id, Some(cids));
                    }
                },
                None => return Err(SyncError::NetworkError("Offer response channel closed".to_string())),
            }
        }
    }
    
    async fn request_nodes(&mut self, peer_id: &str, cids: &[Cid]) -> Result<DAGSyncBundle, SyncError> {
        // Clear any previous pending requests for this peer
        self.inbound.pending_requests.insert(peer_id.to_string(), None);
        
        // Send the request
        self.outbound.request_tx.send((peer_id.to_string(), cids.to_vec())).await
            .map_err(|e| SyncError::NetworkError(format!("Failed to send request: {}", e)))?;
        
        // Wait for a response
        loop {
            // Check if we already have a response
            if let Some(Some(bundle)) = self.inbound.pending_requests.remove(peer_id) {
                return Ok(bundle);
            }
            
            // Wait for a response from the channel
            match self.inbound.request_rx.recv().await {
                Some((resp_peer_id, bundle)) => {
                    if resp_peer_id == peer_id {
                        return Ok(bundle);
                    } else {
                        // Store the response for another peer
                        self.inbound.pending_requests.insert(resp_peer_id, Some(bundle));
                    }
                },
                None => return Err(SyncError::NetworkError("Request response channel closed".to_string())),
            }
        }
    }
    
    async fn discover_peers(&mut self) -> Result<Vec<FederationPeer>, SyncError> {
        // If we have discovered peers, return them
        if !self.inbound.discovered_peers.is_empty() {
            let peers = std::mem::take(&mut self.inbound.discovered_peers);
            return Ok(peers);
        }
        
        // Wait for discovered peers from the channel
        match self.inbound.discovery_rx.recv().await {
            Some(peers) => Ok(peers),
            None => Err(SyncError::NetworkError("Discovery channel closed".to_string())),
        }
    }
    
    fn local_peer_id(&self) -> String {
        self.local_peer_id.clone()
    }
    
    fn local_did(&self) -> Option<Did> {
        self.local_did.clone()
    }
} 