use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::Duration;
use anyhow::bail;
use futures::StreamExt; // Needed for select_next_some
use tokio::sync::mpsc; // Needed for RuntimeHandle

// --- libp2p imports --- START ---
use libp2p::{
    identity,
    PeerId,
    Swarm,
    swarm::{SwarmBuilder, SwarmEvent},
    gossipsub::{self, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, Topic},
    identify::{self, Identify, IdentifyConfig, IdentifyEvent},
    mdns::{Mdns, MdnsEvent},
    ping::{self, Ping, PingConfig, PingEvent},
    tcp::tokio::Transport as TcpTransport, // Explicit import
    yamux::YamuxConfig,
    mplex::MplexConfig,
    core::upgrade,
    noise::{NoiseAuthenticated, XX, Keypair as NoiseKeypair},
    Transport,
    Multiaddr,
    NetworkBehaviour
};
// --- libp2p imports --- END ---

// --- hyper imports --- START ---
use hyper::{
    Body, Request, Response, Server, Method, StatusCode,
    service::{make_service_fn, service_fn}
};
use std::net::SocketAddr;
// --- hyper imports --- END ---

// --- serde imports --- START ---
use serde::Deserialize;
// --- serde imports --- END ---

// --- icn-types and encoding imports --- START ---
use icn_types::{
    SignedDagNode,
    DagError, // Assuming DagError will be exported by icn-types
    // Cid, // Assuming Cid is re-exported or accessible via icn_types
    // Did, // Assuming Did is re-exported or accessible via icn_types
};
use base64::{engine::general_purpose, Engine as _};
use serde_ipld_dagcbor as dagcbor;
// --- icn-types and encoding imports --- END ---

// Assuming FederationConfig is defined in icn_config crate
// Use the actual import path once crates are properly set up
// For now, using the placeholder defined below.
use crate::icn_config_placeholder::FederationConfig;

// REMOVE THE ENTIRE icn_types_placeholder module.
// The user's diff indicates removing the block:
// mod icn_types_placeholder { ... contents ... }
// This was lines 33-162 in the previous version of the file.
// The edit tool should remove this block based on context.

// REMOVE OLD placeholder use:
// use icn_types_placeholder::{DagStore, SledDagStore, SharedDagStore, DagPayload, SignedDagNode};
// This was line 181. The new imports are already added above.

// ADD new DagSubmission struct definition globally
#[derive(Debug, Deserialize)]
pub struct DagSubmission {
    /// Base-64 string of DAG-CBOR-encoded `SignedDagNode`
    pub encoded: String,
}

// UPDATE RuntimeCommand enum definition
pub enum RuntimeCommand {
    SubmitDagNode(SignedDagNode), // Changed DagNode to SignedDagNode
    Shutdown,
}

// --- Existing Placeholder Service Handle Traits/Structs --- START ---
// These are kept from the previous step, defining the interfaces
// that init_dag_store doesn't directly use but other functions will.
// Placeholder for the main ICN Runtime component (from icn-runtime crate)
pub trait RuntimeServiceHandle: Send + Sync {}
struct DummyRuntimeServiceHandle;
impl RuntimeServiceHandle for DummyRuntimeServiceHandle {}

// Placeholder for an API server component
pub trait ApiServerHandle: Send + Sync {}
struct DummyApiServerHandle;
impl ApiServerHandle for DummyApiServerHandle {}

// Placeholder for a Network component (e.g., libp2p Swarm manager)
pub trait NetworkHandle: Send + Sync {}
struct DummyNetworkHandle;
impl NetworkHandle for DummyNetworkHandle {}
// --- Existing Placeholder Service Handle Traits/Structs --- END ---


// --- New Network Behaviour and Handle --- START ---

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MyBehaviourEvent")] // Define an OutEvent for the behaviour
struct MyBehaviour {
    gossipsub: Gossipsub,
    identify: Identify,
    // Optional: Add Kademlia, Ping, etc.
    // kademlia: Kademlia<MemoryStore>,
    // ping: Ping,
    mdns: Mdns, // Add mdns
}

// Define the event enum that MyBehaviour emits
#[derive(Debug)]
enum MyBehaviourEvent {
    Gossipsub(GossipsubEvent),
    Identify(IdentifyEvent),
    Mdns(MdnsEvent),
    // Ping(PingEvent),
    // Kademlia(KademliaEvent),
}

// Implement conversions from specific behaviour events to MyBehaviourEvent
impl From<GossipsubEvent> for MyBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self { MyBehaviourEvent::Gossipsub(event) }
}
impl From<IdentifyEvent> for MyBehaviourEvent {
    fn from(event: IdentifyEvent) -> Self { MyBehaviourEvent::Identify(event) }
}
impl From<MdnsEvent> for MyBehaviourEvent {
    fn from(event: MdnsEvent) -> Self { MyBehaviourEvent::Mdns(event) }
}
// impl From<PingEvent> for MyBehaviourEvent { ... }
// impl From<KademliaEvent> for MyBehaviourEvent { ... }

/// Handle returned by `connect_network` to interact with the network task.
pub struct NetworkHandle {
    pub peer_id: PeerId,
    pub federation_topic: Topic,
    // TODO: Add channels (e.g., mpsc::Sender) to send commands to the network task
    // (e.g., publish message, dial peer) or receive events from it.
}
// --- New Network Behaviour and Handle --- END ---

/// Initializes the DAG store based on configuration.
/// Opens or creates the store at the specified path, validates genesis if present.
pub async fn init_dag_store(config: &FederationConfig) -> anyhow::Result<Arc<SharedDagStore>> {
    // Use placeholders defined above for now
    use crate::icn_types_placeholder::SledDagStore;
    use crate::icn_types_placeholder::{SharedDagStore, DagPayload};

    // Use federation_did for uniqueness, fallback to name if needed
    // Default path construction using safe_id_fragment helper
    let default_base_path = PathBuf::from("./data");
    let federation_fragment = safe_id_fragment(&config.federation_did);
    let default_storage_path = default_base_path.join(&federation_fragment).join("dag_store");

    // Prefer storage_path from config if provided, otherwise use default derived path
    let store_path = config.storage_path.clone().unwrap_or(default_storage_path);

    // Ensure the directory exists
    if let Some(parent) = store_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| 
            anyhow::anyhow!("Failed to create DAG store directory '{}': {}", parent.display(), e)
        )?;
    }
    
    let store_path_str = store_path.to_str().ok_or_else(|| 
        anyhow::anyhow!("Invalid non-UTF8 path for DAG store: {}", store_path.display())
    )?;

    tracing::info!("Initializing DAG store at: {}", store_path_str);
    
    // Assume SledDagStore::open_or_create exists and returns Result<impl DagStore, Error>
    let raw_store = SledDagStore::open_or_create(store_path_str).await
        .map_err(|e| anyhow::anyhow!("Failed to open/create DAG store at '{}': {}", store_path_str, e))?;
        
    // Assume SharedDagStore::new wraps a Box<dyn DagStore>
    let shared_store = Arc::new(SharedDagStore::new(Box::new(raw_store)));

    // Assume SharedDagStore implements get_genesis_node
    match shared_store.get_genesis_node().await {
        Ok(Some(genesis_node)) => {
            tracing::info!("Found existing genesis node in DAG store.");
            // Assume genesis_node.payload() and DagPayload::FederationGenesis exist
            match genesis_node.payload() {
                DagPayload::FederationGenesis(genesis_payload) => {
                    // Validate federation DID
                    if genesis_payload.federation_did != config.federation_did {
                        anyhow::bail!(
                            "Genesis node DID mismatch! Store at '{}' belongs to '{}', but config expects '{}'.",
                            store_path_str,
                            genesis_payload.federation_did,
                            config.federation_did
                        );
                    }
                    tracing::info!("Genesis node DID matches configuration: {}", config.federation_did);
                }
                other_payload => {
                    anyhow::bail!(
                        "Invalid genesis node payload type found in store at '{}'. Expected FederationGenesis, found: {:?}",
                        store_path_str,
                        other_payload
                    );
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                "DAG store at '{}' is empty or genesis node is missing. Federation may need bootstrapping.",
                store_path_str
            );
            // Consider returning an error here if bootstrap is always required before starting?
            // For now, allowing startup with an empty DAG.
        }
        Err(e) => {
            // Propagate errors encountered while trying to read the genesis node
            return Err(anyhow::anyhow!("Failed to query genesis node from store at '{}': {}", store_path_str, e));
        }
    }

    Ok(shared_store)
}


// --- Placeholder implementations for other service functions --- START ---
// Keep these placeholders from the previous step for now

pub async fn spawn_runtime(
    _dag_store: Arc<SharedDagStore>, // Updated type to match init_dag_store return
    config: &FederationConfig,
) -> anyhow::Result<Arc<dyn RuntimeServiceHandle>> {
    tracing::info!("Spawning ICN runtime for federation: {}", config.metadata.name);
    Ok(Arc::new(DummyRuntimeServiceHandle))
}

pub async fn start_api_server(
    _runtime_handle: Arc<dyn RuntimeServiceHandle>,
    config: &FederationConfig,
) -> anyhow::Result<Arc<dyn ApiServerHandle>> {
    tracing::info!("Starting API server for federation: {}", config.metadata.name);
    Ok(Arc::new(DummyApiServerHandle))
}

/// Connects the node to the ICN P2P network.
/// Initializes libp2p Swarm, starts listening, and spawns the event loop.
pub async fn connect_network(
    _runtime_handle: Arc<dyn RuntimeServiceHandle>, // Keep for future use
    config: &FederationConfig,
) -> anyhow::Result<NetworkHandle> {
    // 1. Create or Load Node Identity
    // TODO: Load from config.node.keys_path or persist generated key
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    tracing::info!("Local Peer ID: {}", peer_id);

    // 2. Build Transport Layer
    // Using development transport for simplicity (includes TCP, DNS, WS, Noise, Yamux, Mplex)
    let transport = libp2p::tokio_development_transport(id_keys.clone()).await
        .map_err(|e| anyhow::anyhow!("Failed to create libp2p transport: {}", e))?;

    // 3. Create Gossipsub Topic
    let federation_topic = Topic::new(format!("icn/{}", config.federation_did));

    // 4. Configure and Create Gossipsub Behaviour
    // Use a fixed message id function for deterministic propagation
    let message_id_fn = |message: &gossipsub::Message| {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&message.data);
        gossipsub::MessageId::from(hasher.finalize().as_bytes().to_vec())
    };
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict) // Enforce validation
        .message_id_fn(message_id_fn) // Use stable message IDs
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build gossipsub config: {}", e))?;

    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(id_keys.clone()), gossipsub_config)
        .map_err(|e| anyhow::anyhow!("Failed to create gossipsub behaviour: {}", e))?;

    // Subscribe to the federation topic
    gossipsub.subscribe(&federation_topic)
        .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic '{}': {}", federation_topic, e))?;
    tracing::info!("Subscribed to Gossipsub topic: {}", federation_topic);

    // 5. Create other Behaviours (Identify, mDNS)
    let identify_config = IdentifyConfig::new("/icn/1.0.0".to_string(), id_keys.public());
    let identify = Identify::new(identify_config);

    let mdns = if config.network.enable_mdns.unwrap_or(true) {
        Mdns::new(Default::default()).await
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS: {}", e))?
    } else {
        // If mDNS is disabled, create a disabled Mdns behaviour
        // This requires a bit more setup, or we can conditionally compile it.
        // For now, let's assume it's enabled or handle the error if creation fails.
        // A cleaner way might be to use Option<Mdns> in MyBehaviour if mdns is optional.
        Mdns::new(Default::default()).await? // Simplified: Assume enabled for now if creation works
    };

    // 6. Combine Behaviours
    let behaviour = MyBehaviour {
        gossipsub,
        identify,
        mdns,
    };

    // 7. Build the Swarm
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();

    // 8. Configure Listening Address
    let listen_addr: Multiaddr = config.network.listen_address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid listen address '{}': {}", config.network.listen_address, e))?;
    Swarm::listen_on(&mut swarm, listen_addr.clone())
        .map_err(|e| anyhow::anyhow!("Failed to listen on '{}': {}", listen_addr, e))?;

    // 9. Dial Static Peers (if any)
    if let Some(peers) = &config.network.static_peers {
        for addr_str in peers {
            match addr_str.parse::<Multiaddr>() {
                Ok(addr) => {
                    tracing::info!("Dialing static peer: {}", addr);
                    Swarm::dial(&mut swarm, addr).map_err(|e| anyhow::anyhow!("Failed to dial '{}': {}", addr_str, e))?;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse static peer address '{}': {}", addr_str, e);
                }
            }
        }
    }

    // 10. Spawn the Swarm Event Loop
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            tracing::info!("Node listening on {}", address);
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => {
                            match event {
                                MdnsEvent::Discovered(list) => {
                                    for (peer_id, multiaddr) in list {
                                        tracing::debug!("mDNS discovered: {} {}", peer_id, multiaddr);
                                        // Automatically add discovered peers to gossipsub
                                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                    }
                                }
                                MdnsEvent::Expired(list) => {
                                    for (peer_id, multiaddr) in list {
                                        tracing::debug!("mDNS expired: {} {}", peer_id, multiaddr);
                                        if !swarm.behaviour_mut().mdns.has_node(&peer_id) {
                                             swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                        }
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(event)) => {
                            match event {
                                GossipsubEvent::Message { propagation_source, message_id, message } => {
                                    tracing::info!(
                                        "Got gossipsub message with id: {} from peer: {}: Topic: {}",
                                        message_id,
                                        propagation_source,
                                        message.topic
                                    );
                                    // TODO: Handle received message (e.g., deserialize, validate, pass to DAG store/runtime)
                                    // let node: Result<SignedDagNode, _> = serde_ipld_dagcbor::from_slice(&message.data);
                                }
                                GossipsubEvent::Subscribed { peer_id, topic } => {
                                    tracing::debug!("Peer {} subscribed to topic {}", peer_id, topic);
                                }
                                GossipsubEvent::Unsubscribed { peer_id, topic } => {
                                    tracing::debug!("Peer {} unsubscribed from topic {}", peer_id, topic);
                                }
                                _ => {}
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                             tracing::debug!("Identify event: {:?}", event);
                             // Handle Identify events if needed, e.g., add addresses to Kademlia if using it.
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            tracing::info!("Connection established with peer: {} ({:?})", peer_id, endpoint.get_remote_address());
                            // Add connected peer to gossipsub (might be redundant if discovered via mDNS)
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            tracing::warn!("Connection closed with peer: {} ({:?})", peer_id, cause);
                            // Remove peer from gossipsub if connection drops
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                        SwarmEvent::Dialing(peer_id) => {
                             tracing::debug!("Dialing peer: {:?}", peer_id);
                        }
                        // Handle other swarm events as needed
                        _ => {
                           // tracing::trace!("Unhandled Swarm Event: {:?}", event);
                        }
                    }
                }
                // Add other branches to the select! macro if needed, e.g., for command channels
            }
        }
    });

    // 11. Return the Network Handle
    Ok(NetworkHandle {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic: federation_topic,
    })
}


// TEMP helper — replace with a proper util in icn-types or icn-identity
fn safe_id_fragment(did: &str) -> String {
    // Simple implementation: take the part after the last colon.
    // Consider more robust handling for different DID methods if needed.
    did.rsplit(':').next().unwrap_or(did).to_string()
} 

// --- New Runtime Handle and Command --- START ---
/// Handle returned by `spawn_runtime` to interact with the runtime task.
// Make pub if needed by other crates (e.g., icn-node)
pub struct RuntimeHandle {
    pub dag_store: Arc<SharedDagStore>,
    pub federation_id: String,
    // Use Arc<SharedDagStore> instead of direct DagStore
    pub tx: mpsc::UnboundedSender<RuntimeCommand>,
}

/// Stub PolicyLoader for now
struct DefaultPolicyLoader;

impl PolicyLoader for DefaultPolicyLoader {
    fn load() -> Self {
        DefaultPolicyLoader
    }

    fn evaluate_policy(&self, _scope: &str, _node: &DagNode) -> bool {
        tracing::debug!("(DefaultPolicyLoader) Allowing all nodes.");
        true // Accept all for now
    }
}
// --- New Runtime Handle and Command --- END ---

/// Spawns the main ICN runtime service task.
/// Initializes the RuntimeEngine and runs its event loop in a background task.
/// Returns a handle for interacting with the runtime.
pub async fn spawn_runtime(
    dag_store: Arc<SharedDagStore>, // Use the Arc<SharedDagStore>
    config: &FederationConfig,
) -> anyhow::Result<RuntimeHandle> {
    // Use placeholders
    use crate::icn_runtime_placeholder::{RuntimeEngine, PolicyLoader};

    let (tx, mut rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let federation_id = config.federation_did.clone();

    tracing::info!("Initializing RuntimeEngine for federation: {}", federation_id);
    
    // Assume RuntimeEngine::new exists and takes Arc<SharedDagStore> and Arc<dyn PolicyLoader>
    let runtime_engine = RuntimeEngine::new(
        federation_id.clone(),
        dag_store.clone(), // Clone Arc for the engine
        Arc::new(DefaultPolicyLoader::load()), // Use stub PolicyLoader
    )?;

    tracing::info!("Spawning runtime event loop task...");
    tokio::spawn(async move {
        // Use the moved runtime_engine here
        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    match cmd {
                        RuntimeCommand::SubmitDagNode(node) => {
                            tracing::info!("Runtime received SubmitDagNode command.");
                            // Assume runtime_engine.process_node exists
                            if let Err(e) = runtime_engine.process_node(node).await {
                                tracing::error!("Runtime processing failed: {:?}", e);
                                // TODO: Implement error handling / reporting strategy
                            }
                        }
                        RuntimeCommand::Shutdown => {
                            tracing::info!("Runtime received Shutdown command.");
                            break; // Exit the loop
                        }
                    }
                }
                // Add other select arms if the runtime needs to react to other events
                // e.g., _ = dag_store.watch_for_new_nodes() => { ... }
                else => {
                     tracing::info!("Runtime command channel closed. Exiting loop.");
                     break; // Exit loop if channel closes
                }
            }
        }
        tracing::info!("Runtime event loop task finished.");
    });

    tracing::info!("Runtime service spawned successfully.");
    Ok(RuntimeHandle {
        dag_store, // Move the original Arc here
        federation_id,
        tx, // Give the sender back to the caller
    })
}

/// Starts the API server (e.g., HTTP) to interact with the node.
pub async fn start_api_server(
    runtime_handle: Arc<RuntimeHandle>,
    config: &FederationConfig,
) -> anyhow::Result<ApiHandle> {
    // Parse the listen address
    let addr: SocketAddr = config.api.listen_address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid API listen address '{}': {}", config.api.listen_address, e))?;

    // Clone the sender handle for the runtime command channel
    let tx_runtime = runtime_handle.tx.clone();

    // Define the service factory
    let make_svc = make_service_fn(move |_conn| {
        // Clone the sender for each connection
        let tx = tx_runtime.clone();
        async { Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| handle_request(req, tx.clone()))) }
    });

    // Build the server
    let server = Server::bind(&addr).serve(make_svc);
    tracing::info!("API server listening on http://{}", addr);

    // Spawn the server task
    tokio::spawn(async move {
        // Add graceful shutdown handling later if needed
        if let Err(e) = server.await {
            tracing::error!("API server error: {}", e);
        }
    });

    // Return the handle (currently empty)
    Ok(ApiHandle)
}

/// Handles individual incoming HTTP requests.
async fn handle_request(
    req: Request<Body>,
    tx_runtime: mpsc::UnboundedSender<RuntimeCommand>,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // POST /dag/submit - Accepts a DAG node submission
        (&Method::POST, "/dag/submit") => {
            // 1. read body
            let body = hyper::body::to_bytes(req.into_body()).await?;
            let submission: DagSubmission = match serde_json::from_slice(&body) {
                Ok(s) => s,
                Err(_) => return Ok(bad_request("Malformed JSON")),
            };

            // 2. decode base64 → raw bytes
            let raw = match general_purpose::STANDARD.decode(&submission.encoded) {
                Ok(b) => b,
                Err(_) => return Ok(bad_request("Base64 decode failed")),
            };

            // 3. DAG-CBOR → SignedDagNode
            let signed: SignedDagNode = match dagcbor::from_slice(&raw) {
                Ok(n) => n,
                Err(_) => return Ok(bad_request("Invalid DAG-CBOR payload")),
            };

            // 4. verify CID
            // Ensure SignedDagNode::verify_cid() is implemented in icn-types
            if let Err(e) = signed.verify_cid() {
                return Ok(bad_request(&format!("CID error: {e}")));
            }

            // 5. verify signature (optional, but recommended)
            // Ensure SignedDagNode::verify_signature() is implemented in icn-types
            if let Err(e) = signed.verify_signature() {
                return Ok(bad_request(&format!("Signature error: {e}")));
            }

            // 6. send to runtime
            if tx_runtime
                .send(RuntimeCommand::SubmitDagNode(signed))
                .is_err()
            {
                return Ok(server_error("Runtime not available"));
            }

            return Ok(Response::new("accepted\n".into())); // Diff had "accepted"
        }

        // GET /health - Basic health check
        (&Method::GET, "/health") => {
            let mut response = Response::new(Body::from("{\"status\": \"ok\"}"));
            response.headers_mut().insert(hyper::header::CONTENT_TYPE, "application/json".parse().unwrap());
            Ok(response)
        }

        // Catch-all for other paths
        _ => {
            Ok(not_found())
        }
    }
}

// Helper for 400 Bad Request responses
fn bad_request(msg: &str) -> Response<Body> {
    let body = format!("{{"error": "Bad Request: {}"}}", msg);
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
}

// Helper for 404 Not Found responses
fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"error": "Not Found"}"))
        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
}

// ADDED server_error helper
fn server_error(msg: &str) -> Response<Body> {
    let mut res = Response::new(Body::from(msg));
    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    res
}

/// Connects the node to the ICN P2P network.
pub async fn connect_network(
    _runtime_handle: Arc<RuntimeHandle>,
    config: &FederationConfig,
) -> anyhow::Result<NetworkHandle> {
    // ... (Implementation from previous step remains the same) ...
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    tracing::info!("Local Peer ID: {}", peer_id);
    let transport = libp2p::tokio_development_transport(id_keys.clone()).await
        .map_err(|e| anyhow::anyhow!("Failed to create libp2p transport: {}", e))?;
    let federation_topic = Topic::new(format!("icn/{}", config.federation_did));
    let message_id_fn = |message: &gossipsub::Message| {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&message.data);
        gossipsub::MessageId::from(hasher.finalize().as_bytes().to_vec())
    };
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build gossipsub config: {}", e))?;
    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(id_keys.clone()), gossipsub_config)
        .map_err(|e| anyhow::anyhow!("Failed to create gossipsub behaviour: {}", e))?;
    gossipsub.subscribe(&federation_topic)
        .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic '{}': {}", federation_topic, e))?;
    tracing::info!("Subscribed to Gossipsub topic: {}", federation_topic);
    let identify_config = IdentifyConfig::new("/icn/1.0.0".to_string(), id_keys.public());
    let identify = Identify::new(identify_config);
    let mdns = if config.network.enable_mdns.unwrap_or(true) {
        Mdns::new(Default::default()).await
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS: {}", e))?
    } else {
        Mdns::new(Default::default()).await?
    };
    let behaviour = MyBehaviour {
        gossipsub,
        identify,
        mdns,
    };
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
    let listen_addr: Multiaddr = config.network.listen_address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid listen address '{}': {}", config.network.listen_address, e))?;
    Swarm::listen_on(&mut swarm, listen_addr.clone())
        .map_err(|e| anyhow::anyhow!("Failed to listen on '{}': {}", listen_addr, e))?;
    if let Some(peers) = &config.network.static_peers {
        for addr_str in peers {
            match addr_str.parse::<Multiaddr>() {
                Ok(addr) => {
                    tracing::info!("Dialing static peer: {}", addr);
                    Swarm::dial(&mut swarm, addr).map_err(|e| anyhow::anyhow!("Failed to dial '{}': {}", addr_str, e))?;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse static peer address '{}': {}", addr_str, e);
                }
            }
        }
    }
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            tracing::info!("Node listening on {}", address);
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => {
                            match event {
                                MdnsEvent::Discovered(list) => {
                                    for (peer_id, multiaddr) in list {
                                        tracing::debug!("mDNS discovered: {} {}", peer_id, multiaddr);
                                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                    }
                                }
                                MdnsEvent::Expired(list) => {
                                    for (peer_id, multiaddr) in list {
                                        tracing::debug!("mDNS expired: {} {}", peer_id, multiaddr);
                                        if !swarm.behaviour_mut().mdns.has_node(&peer_id) {
                                             swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                        }
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(event)) => {
                            match event {
                                GossipsubEvent::Message { propagation_source, message_id, message } => {
                                    tracing::info!(
                                        "Got gossipsub message with id: {} from peer: {}: Topic: {}",
                                        message_id,
                                        propagation_source,
                                        message.topic
                                    );
                                }
                                GossipsubEvent::Subscribed { peer_id, topic } => {
                                    tracing::debug!("Peer {} subscribed to topic {}", peer_id, topic);
                                }
                                GossipsubEvent::Unsubscribed { peer_id, topic } => {
                                    tracing::debug!("Peer {} unsubscribed from topic {}", peer_id, topic);
                                }
                                _ => {}
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                             tracing::debug!("Identify event: {:?}", event);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            tracing::info!("Connection established with peer: {} ({:?})", peer_id, endpoint.get_remote_address());
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                            tracing::warn!("Connection closed with peer: {} ({:?})", peer_id, cause);
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                        SwarmEvent::Dialing(peer_id) => {
                             tracing::debug!("Dialing peer: {:?}", peer_id);
                        }
                        _ => {}
                    }
                }
            }
        }
    });
    Ok(NetworkHandle {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic: federation_topic,
    })
} 