#[derive(Debug, Subcommand)]
pub enum FederationCommands {
    /// Create a new federation
    #[command(name = "create")]
    Create {
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Description of the federation
        #[arg(long)]
        description: Option<String>,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Vote on a scope's request to join the federation
    #[command(name = "vote-join")]
    VoteJoin {
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Join request CID
        #[arg(long)]
        request_cid: String,
        
        /// Vote (yes/no)
        #[arg(long)]
        vote: String,
        
        /// Optional comment or reason for the vote
        #[arg(long)]
        reason: Option<String>,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
    
    /// Finalize a join request after quorum is reached
    #[command(name = "finalize-join")]
    FinalizeJoin {
        /// Federation ID
        #[arg(long)]
        federation_id: String,
        
        /// Join request CID
        #[arg(long)]
        request_cid: String,
        
        /// Path to the signing key file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        key: PathBuf,
        
        /// Optional path to DAG storage directory
        #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
        dag_dir: Option<PathBuf>,
    },
}

pub async fn handle_federation_command(command: &FederationCommands, ctx: &mut CliContext) -> CliResult<()> {
    match command {
        // ... existing command handlers ...
        
        FederationCommands::VoteJoin { federation_id, request_cid, vote, reason, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the CID
            let request_cid_obj = cid_from_string(request_cid)?;
            
            // Get the join request node
            let request_node = dag_store.get_node(&request_cid_obj).await?;
            
            // Verify it's actually a join request
            if let DagPayload::Json(json_data) = &request_node.node.payload {
                if let Some(request_type) = json_data.get("type").and_then(|v| v.as_str()) {
                    if !request_type.contains("JoinRequest") {
                        return Err(CliError::ValidationError(
                            format!("Node {} is not a join request", request_cid)));
                    }
                }
            }
            
            // Create a vote node
            let payload = DagPayload::Json(json!({
                "type": "FederationJoinVote",
                "vote": vote,
                "reason": reason,
                "votedAt": chrono::Utc::now().to_rfc3339(),
                "voter": did.to_string(),
                "requestCid": request_cid,
            }));
            
            let node = DagNodeBuilder::new()
                .with_payload(payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("FederationJoinVote".to_string())
                .with_parent(request_cid_obj)
                .build()?;
            
            let signed_node = ctx.sign_dag_node(node, &did_key)?;
            let cid = dag_store.add_node(signed_node).await?;
            
            println!("Recorded vote '{}' on join request {} with CID {}", 
                vote, request_cid, cid);
            
            Ok(())
        },
        
        FederationCommands::FinalizeJoin { federation_id, request_cid, key, dag_dir } => {
            let dag_store = ctx.get_dag_store(dag_dir.as_deref())?;
            let did_key = ctx.load_did_key(key)?;
            let did = Did::from_string(&did_key.to_did_string())?;
            
            // Parse the CID
            let request_cid_obj = cid_from_string(request_cid)?;
            
            // Get the join request node
            let request_node = dag_store.get_node(&request_cid_obj).await?;
            
            // Parse the request data
            let (scope_type, scope_id, scope_genesis_cid_str) = if let DagPayload::Json(json_data) = &request_node.node.payload {
                // Determine if this is a cooperative or community join request
                let request_type = json_data.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CliError::ValidationError(
                        "Invalid join request: missing type".to_string()))?;
                
                let scope_type = if request_type.contains("Cooperative") {
                    NodeScope::Cooperative
                } else if request_type.contains("Community") {
                    NodeScope::Community
                } else {
                    return Err(CliError::ValidationError(
                        format!("Invalid join request type: {}", request_type)));
                };
                
                // Get scope ID and genesis CID based on scope type
                let (id_field, cid_field) = if scope_type == NodeScope::Cooperative {
                    ("cooperativeId", "cooperativeGenesisCid") 
                } else {
                    ("communityId", "communityGenesisCid")
                };
                
                let scope_id = json_data.get(id_field)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CliError::ValidationError(
                        format!("Invalid join request: missing {}", id_field)))?;
                
                let scope_genesis_cid = json_data.get(cid_field)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CliError::ValidationError(
                        format!("Invalid join request: missing {}", cid_field)))?;
                
                (scope_type, scope_id.to_string(), scope_genesis_cid.to_string())
            } else {
                return Err(CliError::ValidationError(
                    "Invalid join request: not a JSON payload".to_string()));
            };
            
            // Get all votes on this request
            let mut vote_nodes = Vec::new();
            let all_nodes = dag_store.get_ordered_nodes().await?;
            
            for node in all_nodes {
                if let DagPayload::Json(json_data) = &node.node.payload {
                    if let Some(node_type) = json_data.get("type").and_then(|v| v.as_str()) {
                        if node_type == "FederationJoinVote" {
                            if let Some(vote_req_cid) = json_data.get("requestCid").and_then(|v| v.as_str()) {
                                if vote_req_cid == request_cid {
                                    vote_nodes.push(node);
                                }
                            }
                        }
                    }
                }
            }
            
            // Process votes and build QuorumProof
            let total_members = 10; // TODO: Get actual federation membership count
            let threshold = 6; // TODO: Get actual threshold from federation config
            let mut eligible_voters = Vec::new(); // TODO: Get from federation members
            
            // For demo purposes, we'll add the federation creator as eligible
            eligible_voters.push(did.clone());
            
            let mut quorum_proof = icn_types::attestation::QuorumProof::new(
                total_members,
                threshold,
                eligible_voters.clone(),
            );
            
            // Process all votes
            let mut vote_cids = Vec::new();
            for vote_node in &vote_nodes {
                if let DagPayload::Json(json_data) = &vote_node.node.payload {
                    let voter = json_data.get("voter")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| CliError::ValidationError(
                            "Invalid vote: missing voter".to_string()))?;
                            
                    let vote_value = json_data.get("vote")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| CliError::ValidationError(
                            "Invalid vote: missing vote".to_string()))?;
                        
                    let voter_did = Did::from_string(voter)?;
                    let vote_choice = vote_value.to_lowercase() == "yes";
                    
                    // Record the vote (this would fail if voter is ineligible)
                    // Since this is a demo, we'll assume all voters are eligible
                    match quorum_proof.add_vote(voter_did, vote_choice) {
                        Ok(_) => {
                            // Record the vote CID
                            vote_cids.push(vote_node.ensure_cid()?);
                        },
                        Err(e) => {
                            eprintln!("Warning: Vote from {} not counted: {}", voter, e);
                        }
                    }
                }
            }
            
            // Check if quorum has been reached and vote is approved
            if !quorum_proof.is_quorum_reached() {
                return Err(CliError::ValidationError(
                    format!("Quorum not reached: {}/{} votes required", 
                        quorum_proof.votes_received, threshold)));
            }
            
            if !quorum_proof.is_approved() {
                return Err(CliError::ValidationError(
                    "Join request was rejected by federation members".to_string()));
            }
            
            // Create a FederationMembershipAttestation
            let scope_genesis_cid = cid_from_string(&scope_genesis_cid_str)?;
            let federation_genesis_cid = request_node.node.parents[0]; // First parent is federation genesis
            
            let membership_attestation = icn_types::attestation::FederationMembershipAttestation::new(
                scope_type,
                &scope_id,
                scope_genesis_cid,
                federation_id,
                federation_genesis_cid,
                request_cid_obj,
                vote_cids,
                quorum_proof,
                Some(format!("Approved join request for {} to federation {}", scope_id, federation_id)),
            );
            
            // Add federation signature to the attestation
            let federation_sig = icn_types::attestation::ScopeSignature {
                signer: did.clone(),
                scope: NodeScope::Federation,
                scope_id: Some(federation_id.clone()),
                signature: did_key.sign(&membership_attestation.canonical_bytes()?),
                timestamp: chrono::Utc::now(),
            };
            
            let mut signed_attestation = membership_attestation;
            signed_attestation.add_signature(federation_sig);
            
            // Create a DAG node for the attestation
            let attestation_json = serde_json::to_value(&signed_attestation)?;
            let attestation_payload = DagPayload::Json(attestation_json);
            
            let attestation_node = DagNodeBuilder::new()
                .with_payload(attestation_payload)
                .with_author(did.clone())
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("FederationMembershipAttestation".to_string())
                .with_parent(request_cid_obj)
                .build()?;
            
            let signed_attestation_node = ctx.sign_dag_node(attestation_node, &did_key)?;
            let attestation_cid = dag_store.add_node(signed_attestation_node).await?;
            
            // Create a LineageAttestation to link the federation and scope DAGs
            let lineage_attestation = icn_types::attestation::LineageAttestation::new_join_attestation(
                federation_id,
                federation_genesis_cid,
                scope_type,
                &scope_id,
                scope_genesis_cid,
                attestation_cid,
            );
            
            // Add federation signature to the lineage attestation
            let lineage_federation_sig = icn_types::attestation::ScopeSignature {
                signer: did.clone(),
                scope: NodeScope::Federation,
                scope_id: Some(federation_id.clone()),
                signature: did_key.sign(&lineage_attestation.canonical_bytes()?),
                timestamp: chrono::Utc::now(),
            };
            
            let mut signed_lineage = lineage_attestation;
            signed_lineage.add_signature(lineage_federation_sig);
            
            // Create a DAG node for the lineage attestation
            let lineage_json = serde_json::to_value(&signed_lineage)?;
            let lineage_payload = DagPayload::Json(lineage_json);
            
            let lineage_node = DagNodeBuilder::new()
                .with_payload(lineage_payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("LineageAttestation".to_string())
                .with_parent(request_cid_obj)
                .with_parent(attestation_cid)
                .build()?;
            
            let signed_lineage_node = ctx.sign_dag_node(lineage_node, &did_key)?;
            let lineage_cid = dag_store.add_node(signed_lineage_node).await?;
            
            // Create approval node to finalize the join process
            let approval_payload = DagPayload::Json(json!({
                "type": "FederationJoinApproval",
                "scopeType": format!("{:?}", scope_type),
                "scopeId": scope_id,
                "federationId": federation_id,
                "requestCid": request_cid,
                "attestationCid": attestation_cid.to_string(),
                "lineageCid": lineage_cid.to_string(),
                "approvedAt": chrono::Utc::now().to_rfc3339(),
                "approver": did.to_string(),
            }));
            
            let approval_node = DagNodeBuilder::new()
                .with_payload(approval_payload)
                .with_author(did)
                .with_federation_id(federation_id.clone())
                .with_scope(NodeScope::Federation)
                .with_label("FederationJoinApproval".to_string())
                .with_parent(request_cid_obj)
                .with_parent(attestation_cid)
                .with_parent(lineage_cid)
                .build()?;
            
            let signed_approval_node = ctx.sign_dag_node(approval_node, &did_key)?;
            let approval_cid = dag_store.add_node(signed_approval_node).await?;
            
            println!("Finalized join for {} '{}' to federation '{}'", 
                if scope_type == NodeScope::Cooperative { "cooperative" } else { "community" },
                scope_id, federation_id);
            println!("Approval CID: {}", approval_cid);
            println!("Attestation CID: {}", attestation_cid);
            println!("Lineage CID: {}", lineage_cid);
            
            Ok(())
        }
    }
}

// ... existing helper functions ... 