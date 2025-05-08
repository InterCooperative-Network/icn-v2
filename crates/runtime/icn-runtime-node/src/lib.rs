use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::Duration;
use anyhow::{anyhow, bail, Result};
use futures::StreamExt; // Needed for select_next_some
use tokio::sync::mpsc; // Needed for RuntimeHandle
use std::collections::HashMap; // For FederationKeyResolver
use anyhow::Context; // For .context() method on Result

// --- libp2p imports --- START ---
use libp2p::{
    identity,
    PeerId,
    Swarm,
    swarm::{SwarmBuilder, SwarmEvent},
    gossipsub::{self, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, Topic, PublishError},
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
use icn_types::dag::signed::{
    SignedDagNode,
    DagNode, // Added DagNode for PolicyLoader if needed by real impl
    DagError as IcnDagError,
    KeyResolver
};
use icn_types::{Did, Cid, DagStore}; // Assuming DagStore trait is from icn-types root

use base64::{engine::general_purpose as b64_std, Engine as _};
use serde_ipld_dagcbor as dagcbor;
use icn_types::dag::memory::InMemoryDagStore; // Import the actual in-memory store
use icn_types::dag::DagStore; // Import the DagStore trait
use icn_types::SharedDagStore; // Import the actual SharedDagStore
use icn_types::dag::signed::DagPayload as ActualDagPayload; // Assuming this is the path
// --- icn-types and encoding imports --- END ---

use crate::icn_config_placeholder::FederationConfig; // Placeholder, assumes FederationConfig has `members` field

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
    SubmitDagNode(SignedDagNode),
    Shutdown,
}

// Define RuntimeEvent
pub enum RuntimeEvent {
    NodeAdded(Cid),
    NodeProcessingFailed { cid: Option<Cid>, error: String }, // Optional CID if it couldn't be derived
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
pub struct NetworkHandleLibp2p {
    pub peer_id: PeerId,
    pub federation_topic: Topic,
    // TODO: Add channels (e.g., mpsc::Sender) to send commands to the network task
    // (e.g., publish message, dial peer) or receive events from it.
}
// --- New Network Behaviour and Handle --- END ---

/// Initializes the DAG store based on configuration.
/// Opens or creates the store at the specified path, validates genesis if present.
pub async fn init_dag_store(config: &FederationConfig) -> anyhow::Result<Arc<SharedDagStore>> {
    // Remove direct use of icn_types_placeholder for SledDagStore and SharedDagStore here.
    // We will use the actual types from icn_types.

    let federation_did_str = config.federation_did.clone();
    tracing::info!(
        "Initializing InMemoryDagStore for federation: {}. Storage path from config will be ignored for InMemory store.",
        federation_did_str
    );

    // For InMemoryDagStore, path-based creation and genesis node loading from disk isn't applicable in the same way.
    // We'll create a new InMemoryDagStore.
    // If you had a way to serialize/deserialize InMemoryDagStore or load initial data, it would go here.
    let in_memory_store = InMemoryDagStore::new(); 

    let shared_store = Arc::new(SharedDagStore::new(Box::new(in_memory_store)));

    // With InMemoryDagStore, it will always be empty on fresh init unless you have a load mechanism.
    // The previous genesis check logic based on Sled might not directly apply or needs adaptation.
    // For now, we'll log that it's an in-memory store and typically starts empty.
    match shared_store.get_genesis_node().await { // Assumes SharedDagStore still has get_genesis_node
        Ok(Some(genesis_node)) => {
            tracing::info!("Found existing genesis node in InMemoryDagStore (this implies a pre-loaded store).");
            // If DagPayload is from icn_types::dag::signed::DagPayload, use it
            // This part depends on your actual SignedDagNode structure from icn-types
            // For example, if genesis_node.node.payload is the enum:
            // match genesis_node.node.payload { 
            //     ActualDagPayload::FederationGenesis(genesis_payload) => { ... }
            // }
            // The placeholder check might not work directly if types have changed significantly.
            // For now, skipping the detailed DID check for InMemory store, 
            // as its state is not typically persisted in the same way as Sled.
            tracing::warn!("Genesis node found in InMemoryStore. DID validation logic might need review.");
        }
        Ok(None) => {
            tracing::info!(
                "InMemoryDagStore initialized fresh for federation: {}. It is empty.",
                federation_did_str
            );
            // No bootstrapping error for an empty in-memory store usually.
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to query genesis node from InMemoryDagStore for federation '{}': {}",
                federation_did_str, e
            ));
        }
    }

    Ok(shared_store)
}


// --- Placeholder implementations for other service functions --- START ---
// Keep these placeholders from the previous step for now

pub async fn spawn_runtime(
    dag_store: Arc<SharedDagStore>,
    config: &FederationConfig,
) -> Result<(RuntimeHandle, mpsc::UnboundedReceiver<Cid>)> {
    let (tx_commands, mut rx_commands) = mpsc::unbounded_channel::<RuntimeCommand>();
    let (tx_node_added, rx_node_added) = mpsc::unbounded_channel::<Cid>(); // Unbounded for simplicity

    let federation_id = config.federation_did.clone();
    let ds_clone = dag_store.clone(); // Clone for the async block
    
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(cmd) = rx_commands.recv() => {
                    match cmd {
                        RuntimeCommand::SubmitDagNode(signed_node) => {
                            let node_cid = signed_node.cid.clone(); // Clone CID for logging/event
                            tracing::debug!(cid = %node_cid, "Runtime received SubmitDagNode");
                            match ds_clone.add_node(signed_node).await { 
                                Ok(stored_cid) => {
                                    // Sanity check - should match node_cid if add_node doesn't recalculate/alter
                                    if stored_cid != node_cid {
                                         tracing::warn!(expected_cid = %node_cid, stored_cid = %stored_cid, "CID mismatch after storing node!");
                                    }
                                    tracing::info!(cid = %stored_cid, "Node stored successfully.");
                                    // Send the *stored* CID to the network task
                                    if let Err(e) = tx_node_added.send(stored_cid.clone()) {
                                        tracing::error!(cid = %stored_cid, "Failed to send NodeAdded event: {}", e);
                                    }
                                }
                                Err(e) => {
                                    // Avoid sending event on error
                                    tracing::error!(cid = %node_cid, "Failed to add node to DagStore: {:?}", e);
                                }
                            }
                        }
                        RuntimeCommand::Shutdown => {
                            tracing::info!("Runtime received Shutdown. Exiting task.");
                            break;
                        }
                    }
                }
                else => { break; } // Channel closed
            }
        }
        tracing::info!("Runtime event loop task finished.");
    });

    let handle = RuntimeHandle {
        dag_store,
        federation_id,
        tx_commands,
    };
    Ok((handle, rx_node_added)) // Return handle and the CID event receiver
}

pub async fn start_api_server(
    runtime_handle: Arc<RuntimeHandle>,
    key_resolver: Arc<dyn KeyResolver + Send + Sync>,
    config: &FederationConfig,
) -> anyhow::Result<Arc<dyn ApiServerHandle>> {
    let addr: SocketAddr = config.api.listen_address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid API listen address '{}': {}", config.api.listen_address, e))?;

    let cmd_tx_runtime = runtime_handle.tx_commands.clone();

    let make_svc = make_service_fn(move |_conn| {
        let cmd_tx_clone = cmd_tx_runtime.clone();
        let kr_clone = key_resolver.clone();
        async { Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| handle_request(req, cmd_tx_clone.clone(), kr_clone.clone()))) }
    });

    let server = Server::bind(&addr).serve(make_svc);
    tracing::info!("API server listening on http://{}", addr);

    tokio::spawn(async move {
        if let Err(e) = server.await {
            tracing::error!("API server error: {}", e);
        }
    });

    Ok(Arc::new(DummyApiServerHandle))
}

/// Handles individual incoming HTTP requests.
async fn handle_request(
    req: Request<Body>,
    tx_runtime_command: mpsc::UnboundedSender<RuntimeCommand>,
    key_resolver: Arc<dyn KeyResolver + Send + Sync>,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/dag/submit") => {
            let body = hyper::body::to_bytes(req.into_body()).await?;
            let submission: DagSubmission = match serde_json::from_slice(&body) {
                Ok(s) => s,
                Err(_) => return Ok(bad_request("Malformed JSON")),
            };
            let raw = match b64_std.decode(&submission.encoded) {
                Ok(b) => b,
                Err(_) => return Ok(bad_request("Base64 decode failed")),
            };
            let signed: SignedDagNode = match dagcbor::from_slice(&raw) {
                Ok(n) => n,
                Err(_) => return Ok(bad_request("Invalid DAG-CBOR payload")),
            };
            if let Err(e) = signed.verify_signature(&*key_resolver) {
                return Ok(bad_request(&format!("Signature check failed: {e}")));
            }
            if tx_runtime_command.send(RuntimeCommand::SubmitDagNode(signed)).is_err() {
                return Ok(server_error("Runtime not available"));
            }
            return Ok(Response::new("accepted\n".into()));
        }

        (&Method::GET, "/health") => {
            let mut response = Response::new(Body::from("{\"status\": \"ok\"}"));
            response.headers_mut().insert(hyper::header::CONTENT_TYPE, "application/json".parse().unwrap());
            Ok(response)
        }

        _ => {
            Ok(not_found())
        }
    }
}

fn bad_request(msg: &str) -> Response<Body> {
    let body = format!("{{"error": "Bad Request: {}"}}", msg);
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"error": "Not Found"}"))
        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
}

fn server_error(msg: &str) -> Response<Body> {
    let mut res = Response::new(Body::from(msg));
    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    res
}

/// Connects the node to the ICN P2P network.
pub async fn connect_network(
    runtime_handle: Arc<RuntimeHandle>, // Pass full RuntimeHandle
    config: &FederationConfig,
    mut node_added_rx: mpsc::UnboundedReceiver<Cid>, // Receiver for CIDs from local runtime
) -> anyhow::Result<NetworkHandleLibp2p> {
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    tracing::info!("Local Peer ID: {}", peer_id);
    let transport = libp2p::tokio_development_transport(id_keys.clone()).await?;
    let topic_string = format!("icn/2/federation/{}/dag/events", config.federation_did);
    let federation_topic = Topic::new(topic_string);
    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validate_messages()
        .message_id_fn(|msg: &GossipsubEvent| GossipsubEvent::MessageId::from(blake3::hash(&msg.data).as_bytes().to_vec()))
        .build().map_err(|e| anyhow::anyhow!("Build gossipsub config: {}", e))?;
    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(id_keys.clone()), gossipsub_config)?;
    gossipsub.subscribe(&federation_topic)?;
    let behaviour = MyBehaviour {
        gossipsub,
        identify: Identify::new(IdentifyConfig::new("/icn/2.0.0".into(), id_keys.public())), 
        mdns: Mdns::new(Default::default()).await?,
    };
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
    Swarm::listen_on(&mut swarm, config.node.p2p_listen_address.parse()?)?;
    if let Some(static_peers) = &config.node.static_peers {
        for addr_str in static_peers { 
            if let Ok(addr) = addr_str.parse() { 
                if let Err(e) = swarm.dial(addr) { 
                    tracing::warn!("Failed to dial static peer {}: {:?}", addr_str, e);
                }
            } 
        }
    }
    
    let (command_tx, mut command_rx) = mpsc::unbounded_channel::<NetworkCommand>();
    let gossip_topic_clone = federation_topic.clone();
    let dag_store_clone = runtime_handle.dag_store.clone();
    let runtime_command_tx_clone = runtime_handle.tx_commands.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(cid_to_publish) = node_added_rx.recv() => {
                    tracing::debug!("NetListen: CID {} from runtime, publishing to gossipsub topic {}", cid_to_publish, gossip_topic_clone);
                    let cid_bytes = cid_to_publish.to_bytes(); 
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(gossip_topic_clone.clone(), cid_bytes) {
                        tracing::error!("Gossipsub publish error for CID {}: {:?}", cid_to_publish, e);
                    }
                }

                Some(command) = command_rx.recv() => {
                    match command {
                        NetworkCommand::Publish{topic, data} => {
                            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                tracing::error!("Explicit gossipsub publish error: {:?}", e);
                            }
                        }
                    }
                }

                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => tracing::info!("P2P listening on {}", address),
                        
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(GossipsubEvent::Message { 
                            propagation_source, message_id, message 
                        })) => {
                             tracing::info!(
                                 "Gossipsub RX: message id {} from {} on topic \'{}\'", 
                                 message_id, propagation_source, message.topic
                             );
                             
                             if message.topic == gossip_topic_clone {
                                match Cid::try_from(message.data) {
                                    Ok(received_cid) => {
                                        tracing::debug!("Received CID announcement for {}", received_cid);
                                        match dag_store_clone.get_node(&received_cid).await {
                                            Ok(Some(_)) => {
                                                tracing::trace!("Already have node {}, skipping fetch.", received_cid);
                                            }
                                            Ok(None) => {
                                                tracing::info!("Received CID {} for a node we don't have. Needs fetching.", received_cid);
                                                // ======================================================
                                                // TODO: Implement mechanism to FETCH the full SignedDagNode for received_cid
                                                // ======================================================
                                                // Example Placeholder: Directly submit the CID (which won't work, need full node)
                                                // let placeholder_fetch_result = Err(anyhow!("Node fetching not implemented"));
                                                // if let Ok(fetched_node) = placeholder_fetch_result {
                                                //      // TODO: Verify signature of fetched_node using a KeyResolver accessible here
                                                //      if let Err(e) = runtime_command_tx_clone.send(RuntimeCommand::SubmitDagNode(fetched_node)) {
                                                //          tracing::error!("Failed to submit fetched node {} to runtime: {}", received_cid, e);
                                                //      }
                                                // }
                                            }
                                            Err(e) => {
                                                tracing::error!("Error checking local DagStore for CID {}: {:?}", received_cid, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to parse gossip message data as CID: {:?}", e);
                                    }
                                }
                            } else {
                                tracing::debug!("Received gossip message on unexpected topic: {}", message.topic);
                            }
                        }
                        
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns_event)) => { /* ... */ }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(id_event)) => { /* ... */ }
                        SwarmEvent::ConnectionEstablished { .. } => { /* ... */ }
                        SwarmEvent::ConnectionClosed { .. } => { /* ... */ }
                        _ => { /* Optional: Log other events */ }
                    }
                }
            }
        }
    });

    Ok(NetworkHandleLibp2p {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic,
        command_tx,
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
    pub tx_commands: mpsc::UnboundedSender<RuntimeCommand>,
}

/// Stub PolicyLoader for now
struct DefaultPolicyLoader;

impl icn_runtime::PolicyLoader for DefaultPolicyLoader {
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
    dag_store: Arc<icn_types::SharedDagStore>, // Use the Arc<SharedDagStore>
    config: &FederationConfig,
) -> anyhow::Result<(RuntimeHandle, mpsc::UnboundedReceiver<Cid>)> {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<RuntimeCommand>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Cid>(); // Channel for runtime events
    
    let federation_id_clone = config.federation_did.clone();
    let ds_clone = dag_store.clone(); // Clone Arc<SharedDagStore> for the task
    let node_added_tx_clone = event_tx; // Clone the sender for the task
    let policy_loader = Arc::new(DefaultPolicyLoader::load()); // Example policy loader

    tracing::info!("Spawning runtime event loop task for federation: {}", federation_id_clone);
    tokio::spawn(async move {
        // This loop simulates the RuntimeEngine's core behavior
        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => { // Use rx_commands from spawn_runtime scope
                    match cmd {
                        RuntimeCommand::SubmitDagNode(signed_node) => {
                            tracing::info!("Runtime Task: Received SubmitDagNode for CID: {}", signed_node.cid);

                            // 1. Store the node
                            // Use the cloned DagStore Arc
                            match ds_clone.add_node(signed_node.clone()).await { 
                                Ok(stored_cid) => {
                                    tracing::info!("Runtime Task: Node {} added successfully.", stored_cid);
                                    
                                    // Sanity check CID consistency (optional but good)
                                    if stored_cid != signed_node.cid {
                                        tracing::warn!(
                                            "Runtime Task: CID mismatch after store! Submitted: {}, Stored: {}. Broadcasting stored CID.",
                                            signed_node.cid, stored_cid
                                        );
                                    }

                                    // 2. Send CID on the event channel
                                    // Use the cloned sender
                                    if let Err(e) = node_added_tx_clone.send(stored_cid.clone()) { // Send the *stored* CID
                                        tracing::error!("Runtime Task: Failed to send NodeAdded event for CID {}: {}", stored_cid, e);
                                    }
                                }
                                Err(e) => {
                                    // Check if the error indicates the node already exists
                                    // This depends on DagStore implementation and DagError variants
                                    // Example using a hypothetical DagError::NodeExists variant:
                                    // match e {
                                    //     icn_types::DagError::NodeExists(cid) => { 
                                    //         tracing::warn!("Runtime Task: Attempted to add existing node {}. Ignoring store failure.", cid);
                                    //         // Decide if you still want to broadcast the CID even if it already existed.
                                    //         // Maybe not, as other nodes likely already have it.
                                    //     },
                                    //     _ => { // Handle other errors
                                            tracing::error!("Runtime Task: Failed to add node {} to DagStore: {:?}", signed_node.cid, e);
                                    //     }
                                    // }
                                }
                            }
                        }
                        RuntimeCommand::Shutdown => {
                            tracing::info!("Runtime Task: Received Shutdown command.");
                            break; // Exit the loop
                        }
                    }
                }
                else => {
                     tracing::info!("Runtime Task: Command channel closed. Exiting loop.");
                     break; // Exit loop if command channel closes
                }
            }
        }
        tracing::info!("Runtime event loop task finished.");
    });

    let runtime_handle = RuntimeHandle {
        dag_store, // Move the original Arc here
        federation_id: config.federation_did.clone(),
        tx_commands: cmd_tx,    // Give the sender back to the caller
    };

    tracing::info!("Runtime service spawned successfully for federation: {}", config.federation_did);
    Ok((runtime_handle, event_rx))
}

/// Starts the API server (e.g., HTTP) to interact with the node.
pub async fn start_api_server(
    runtime_handle: Arc<RuntimeHandle>,
    key_resolver: Arc<dyn KeyResolver + Send + Sync>, // Added key_resolver argument
    config: &FederationConfig,
) -> anyhow::Result<Arc<dyn ApiServerHandle>> {
    // Parse the listen address
    let addr: SocketAddr = config.api.listen_address.parse()
        .map_err(|e| anyhow::anyhow!("Invalid API listen address '{}': {}", config.api.listen_address, e))?;

    // Clone the sender handle for the runtime command channel
    let cmd_tx_runtime = runtime_handle.tx_commands.clone();

    // Define the service factory
    let make_svc = make_service_fn(move |_conn| {
        // Clone the sender for each connection
        let cmd_tx_clone = cmd_tx_runtime.clone();
        let kr_clone = key_resolver.clone(); // Clone Arc for KeyResolver
        async { Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| handle_request(req, cmd_tx_clone.clone(), kr_clone.clone()))) }
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
    Ok(Arc::new(DummyApiServerHandle))
}

/// Handles individual incoming HTTP requests.
async fn handle_request(
    req: Request<Body>,
    tx_runtime_command: mpsc::UnboundedSender<RuntimeCommand>,
    key_resolver: Arc<dyn KeyResolver + Send + Sync>,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // POST /dag/submit - Accepts a DAG node submission
        (&Method::POST, "/dag/submit") => {
            let body = hyper::body::to_bytes(req.into_body()).await?;
            let submission: DagSubmission = match serde_json::from_slice(&body) {
                Ok(s) => s,
                Err(_) => return Ok(bad_request("Malformed JSON")),
            };
            let raw = match b64_std.decode(&submission.encoded) {
                Ok(b) => b,
                Err(_) => return Ok(bad_request("Base64 decode failed")),
            };
            let signed: SignedDagNode = match dagcbor::from_slice(&raw) {
                Ok(n) => n,
                Err(_) => return Ok(bad_request("Invalid DAG-CBOR payload")),
            };
            if let Err(e) = signed.verify_signature(&*key_resolver) {
                return Ok(bad_request(&format!("Signature check failed: {e}")));
            }
            if tx_runtime_command.send(RuntimeCommand::SubmitDagNode(signed)).is_err() {
                return Ok(server_error("Runtime not available"));
            }
            return Ok(Response::new("accepted\n".into()));
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
) -> anyhow::Result<NetworkHandleLibp2p> {
    // ... (Implementation from previous step remains the same) ...
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    tracing::info!("Local Peer ID: {}", peer_id);
    let transport = libp2p::tokio_development_transport(id_keys.clone()).await?;
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
    gossipsub.subscribe(&federation_topic)?;
    let identify = Identify::new(IdentifyConfig::new("/icn/1.0.0".to_string(), id_keys.public()));
    let mdns = Mdns::new(Default::default()).await?;
    let behaviour = MyBehaviour { gossipsub, identify, mdns };
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
    let listen_addr: Multiaddr = config.network.listen_address.parse()?;
    Swarm::listen_on(&mut swarm, listen_addr)?;
    tokio::spawn(async move { loop { swarm.select_next_some().await; } });

    Ok(NetworkHandleLibp2p {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic,
    })
} 