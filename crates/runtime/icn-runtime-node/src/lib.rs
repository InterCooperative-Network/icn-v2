                    match swarm.behaviour_mut().gossipsub.publish(network_task_topic.clone(), cid_bytes) {
                        Ok(message_id) => {
                            tracing::info!("Published CID {} via gossipsub, message ID: {}", cid_to_publish, message_id);
                        }
                        Err(PublishError::InsufficientPeers) => {
                            tracing::warn!("Failed to publish CID {}: Insufficient peers in topic {}", cid_to_publish, network_task_topic);
                        }
                        Err(e) => {
                            tracing::error!("Failed to publish CID {} to topic {}: {:?}", cid_to_publish, network_task_topic, e);
                        }
                    }
                }

                // Existing Swarm event handling
                event = swarm.select_next_some() => {
                     match event {
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(GossipsubEvent::Message { message, .. })) => {
                             tracing::info!("Received gossip message on topic {}", message.topic);
                             // TODO: Process received gossip messages (step 3)
                             match Cid::try_from(message.data) {
                                 Ok(cid) => tracing::info!("Gossipped message parsed as CID: {}", cid),
                                 Err(e) => tracing::warn!("Could not parse gossipped message as CID: {:?}", e),
                             }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(NetworkHandleLibp2p {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic: federation_topic, // Return original topic name
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
    pub events_tx: mpsc::UnboundedSender<Cid>,
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
        events_tx: event_tx,
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
    runtime_handle: Arc<RuntimeHandle>, // Pass the full handle
    mut events_rx: UnboundedReceiver<Cid>, // Pass the event receiver separately
    config: &FederationConfig,
) -> anyhow::Result<NetworkHandleLibp2p> {
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    tracing::info!("Local Peer ID for connect_network: {}", peer_id);
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
    let listen_addr_str = config.network.listen_address.clone();
    let listen_addr: Multiaddr = listen_addr_str.parse()?;
    Swarm::listen_on(&mut swarm, listen_addr)?;
    if let Some(peers) = &config.network.static_peers { /* ... dial peers ... */ }

    // Clone needed parts from runtime_handle for the task
    let dag_store_clone = runtime_handle.dag_store.clone();
    let runtime_command_tx_clone = runtime_handle.tx_commands.clone(); // Renamed tx_commands earlier
    let topic_clone = federation_topic.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Receive CIDs from runtime to publish
                Some(cid) = events_rx.recv() => {
                    tracing::debug!(%cid, "NetLoop: Received CID from runtime to publish");
                    let cid_bytes = cid.to_bytes();
                    match swarm.behaviour_mut().gossipsub.publish(topic_clone.clone(), cid_bytes) {
                        Ok(msg_id) => tracing::info!(%cid, %msg_id, "NetLoop: Published CID announcement"),
                        Err(PublishError::InsufficientPeers) => tracing::warn!(%cid, topic=%topic_clone, "NetLoop: Cannot publish CID, insufficient peers"),
                        Err(e) => tracing::error!(%cid, topic=%topic_clone, "NetLoop: Failed to publish CID: {:?}", e),
                    }
                }

                // Handle swarm events
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(GossipsubEvent::Message { message, .. })) => {
                            if message.topic == topic_clone {
                                match Cid::try_from(message.data) {
                                    Ok(received_cid) => {
                                        tracing::debug!(cid=%received_cid, "NetLoop: Received gossiped CID announcement");
                                        // Check if we already have the node
                                        let ds_check = dag_store_clone.clone();
                                        let rtx_check = runtime_command_tx_clone.clone();
                                        tokio::spawn(async move {
                                            match ds_check.get_node(&received_cid).await { // Use async get_node
                                                Ok(None) => {
                                                    tracing::info!(cid=%received_cid, "NetLoop: Need to fetch node for received CID.");
                                                    // TODO: Implement fetching mechanism (e.g., Req/Rep, HTTP, full gossip)
                                                    // Once node (fetched_node: SignedDagNode) is obtained:
                                                    // if let Err(e) = rtx_check.send(RuntimeCommand::SubmitDagNode(fetched_node)) {
                                                    //    tracing::error!("Failed to submit fetched node {} to runtime: {}", received_cid, e);
                                                    // }
                                                }
                                                Ok(Some(_)) => {
                                                    tracing::trace!(cid=%received_cid, "NetLoop: Already have node, ignoring announcement.");
                                                }
                                                Err(e) => {
                                                    tracing::error!(cid=%received_cid, "NetLoop: Error checking DagStore for received CID: {:?}", e);
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!("NetLoop: Failed to parse gossip data as CID: {:?}", e);
                                    }
                                }
                            }
                        }
                        // ... other SwarmEvent handlers (Mdns, Identify, Connection etc.) ...
                        SwarmEvent::NewListenAddr { .. } => { /* log */ }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(_)) => { /* handle discovery */ }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(_)) => { /* handle identify */ }
                        SwarmEvent::ConnectionEstablished { .. } => { /* add peer */ }
                        SwarmEvent::ConnectionClosed { .. } => { /* remove peer */ }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok(NetworkHandleLibp2p {
        peer_id: swarm.local_peer_id().clone(),
        federation_topic,
    })
} 