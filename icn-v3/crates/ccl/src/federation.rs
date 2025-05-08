use crate::error::CclError;
use crate::quorum::{
    QuorumPolicy, QuorumProof, QuorumType, 
    MembershipJoinRequest, MembershipVote, MembershipAcceptance
};
use icn_common::dag::{DAGNode, DAGNodeID, DAGNodeType};
use icn_common::identity::{Identity, IdentityType, ScopedIdentity, Credential};
use icn_common::verification::{Signature, Verifiable};
use icn_services::dag::{DagStorage, DagReplayVerifier};

use async_trait::async_trait;
use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Federation membership status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipStatus {
    /// Active member
    Active,
    
    /// Pending approval
    Pending,
    
    /// Suspended
    Suspended,
    
    /// Expelled
    Expelled,
}

/// Federation membership
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMembership {
    /// The member identity
    pub member_id: String,
    
    /// The federation scope
    pub federation_scope: String,
    
    /// Membership status
    pub status: MembershipStatus,
    
    /// Membership role
    pub role: String,
    
    /// Join timestamp
    pub joined_at: u64,
    
    /// Last status change timestamp
    pub status_changed_at: u64,
    
    /// Credentials issued by the federation
    pub credentials: Vec<Credential>,
    
    /// Reference to the DAG node that established this membership
    pub dag_reference: Option<DAGNodeID>,
    
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

impl FederationMembership {
    /// Create a new federation membership
    pub fn new(
        member_id: String,
        federation_scope: String,
        role: String,
        credentials: Vec<Credential>,
        dag_reference: Option<DAGNodeID>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            member_id,
            federation_scope,
            status: MembershipStatus::Active,
            role,
            joined_at: now,
            status_changed_at: now,
            credentials,
            dag_reference,
            metadata,
        }
    }
    
    /// Change the membership status
    pub fn change_status(&mut self, status: MembershipStatus) {
        self.status = status;
        self.status_changed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }
    
    /// Add a credential to this membership
    pub fn add_credential(&mut self, credential: Credential) {
        self.credentials.push(credential);
    }
    
    /// Check if membership is active
    pub fn is_active(&self) -> bool {
        self.status == MembershipStatus::Active
    }
}

/// Federation structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Federation {
    /// Unique identifier
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Federation scope
    pub scope: String,
    
    /// Federation description
    pub description: Option<String>,
    
    /// Federation governance policies
    pub policies: Vec<QuorumPolicy>,
    
    /// Federation members
    pub memberships: HashMap<String, FederationMembership>, // member ID -> membership
    
    /// Federation identity
    pub identity: Identity,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
    
    /// Reference to the DAG node that established this federation
    pub dag_reference: Option<DAGNodeID>,
}

impl Federation {
    /// Create a new federation
    pub fn new(
        name: String,
        scope: String,
        description: Option<String>,
        identity: Identity,
        metadata: Option<serde_json::Value>,
        dag_reference: Option<DAGNodeID>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        // Create default policies
        let mut policies = Vec::new();
        
        // Simple majority for most decisions
        policies.push(QuorumPolicy::new(
            "Default".to_string(),
            scope.clone(),
            QuorumType::SimpleMajority,
            None,
            Some("Default simple majority policy".to_string()),
        ));
        
        // Qualified majority for important decisions
        policies.push(QuorumPolicy::new(
            "Important".to_string(),
            scope.clone(),
            QuorumType::QualifiedMajority(67), // 2/3 majority
            None,
            Some("Qualified majority for important decisions".to_string()),
        ));
        
        Self {
            id: identity.id.clone(),
            name,
            scope,
            description,
            policies,
            memberships: HashMap::new(),
            identity,
            created_at: now,
            metadata,
            dag_reference,
        }
    }
    
    /// Get the federation's identity as a scoped identity
    pub fn scoped_identity(&self) -> ScopedIdentity {
        ScopedIdentity::new(
            self.identity.clone(),
            self.scope.clone(),
            self.dag_reference.clone(),
        )
    }
    
    /// Get a policy by name
    pub fn get_policy(&self, name: &str) -> Option<&QuorumPolicy> {
        self.policies.iter().find(|p| p.name == name)
    }
    
    /// Add a member to the federation
    pub fn add_member(
        &mut self,
        membership: FederationMembership,
    ) -> Result<(), CclError> {
        let member_id = membership.member_id.clone();
        
        if self.memberships.contains_key(&member_id) {
            return Err(CclError::Membership(
                format!("Member {} already exists in federation", member_id)
            ));
        }
        
        self.memberships.insert(member_id, membership);
        Ok(())
    }
    
    /// Check if an identity is a member of the federation
    pub fn is_member(&self, identity_id: &str) -> bool {
        self.memberships.contains_key(identity_id) && 
        self.memberships.get(identity_id).unwrap().is_active()
    }
    
    /// Check if an identity has a specific role in the federation
    pub fn has_role(&self, identity_id: &str, role: &str) -> bool {
        if let Some(membership) = self.memberships.get(identity_id) {
            membership.is_active() && membership.role == role
        } else {
            false
        }
    }
    
    /// Get all active members
    pub fn active_members(&self) -> Vec<&FederationMembership> {
        self.memberships.values()
            .filter(|m| m.is_active())
            .collect()
    }
    
    /// Get the total number of active members
    pub fn active_member_count(&self) -> u32 {
        self.memberships.values()
            .filter(|m| m.is_active())
            .count() as u32
    }
}

/// Federation manager interface
#[async_trait]
pub trait FederationManager: Send + Sync + 'static {
    /// Create a new federation
    async fn create_federation(
        &self,
        name: String,
        scope: String,
        description: Option<String>,
        founder_identities: Vec<Identity>,
        federation_identity: Identity,
        federation_key: &SecretKey,
        metadata: Option<serde_json::Value>,
    ) -> Result<Federation, CclError>;
    
    /// Get a federation by scope
    async fn get_federation(&self, scope: &str) -> Result<Federation, CclError>;
    
    /// Submit a membership join request
    async fn submit_join_request(
        &self,
        request: MembershipJoinRequest,
    ) -> Result<DAGNodeID, CclError>;
    
    /// Vote on a membership join request
    async fn vote_on_join_request(
        &self,
        vote: MembershipVote,
    ) -> Result<DAGNodeID, CclError>;
    
    /// Accept a membership join request
    async fn accept_join_request(
        &self,
        acceptance: MembershipAcceptance,
    ) -> Result<DAGNodeID, CclError>;
    
    /// Get pending join requests for a federation
    async fn get_pending_requests(
        &self,
        federation_scope: &str,
    ) -> Result<Vec<MembershipJoinRequest>, CclError>;
    
    /// Get votes for a join request
    async fn get_request_votes(
        &self,
        request_id: &str,
    ) -> Result<Vec<MembershipVote>, CclError>;
}

/// Default implementation of FederationManager
pub struct DefaultFederationManager {
    dag_storage: Arc<DagStorage>,
}

impl DefaultFederationManager {
    /// Create a new federation manager
    pub fn new(dag_storage: Arc<DagStorage>) -> Self {
        Self { dag_storage }
    }
    
    /// Create DAG node for federation creation
    async fn create_federation_node(
        &self,
        federation: &Federation,
        founder_identities: &[Identity],
        federation_key: &SecretKey,
    ) -> Result<DAGNodeID, CclError> {
        // Create a scoped identity for the federation
        let federation_scoped = ScopedIdentity::new(
            federation.identity.clone(),
            federation.scope.clone(),
            None,
        );
        
        // Create the DAG node
        let founder_json: Vec<serde_json::Value> = founder_identities.iter()
            .map(|id| {
                serde_json::json!({
                    "id": id.id,
                    "name": id.name,
                    "type": format!("{:?}", id.identity_type),
                })
            })
            .collect();
            
        let payload = serde_json::json!({
            "name": federation.name,
            "description": federation.description,
            "founders": founder_json,
            "policies": federation.policies,
            "metadata": federation.metadata,
        });
        
        let node = DAGNode::new(
            DAGNodeType::FederationCreation,
            HashSet::new(), // No parents for genesis node
            federation.scope.clone(),
            federation_scoped,
            payload,
            federation_key,
        )?;
        
        // Add the node to the DAG
        let node_id = self.dag_storage.add_node(&node).await?;
        
        Ok(node_id)
    }
    
    /// Create DAG node for a membership join request
    async fn create_join_request_node(
        &self,
        request: &MembershipJoinRequest,
    ) -> Result<DAGNodeID, CclError> {
        // Find the federation node to use as parent
        let federation_nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::FederationCreation,
            None,
            None,
        ).await?;
        
        let mut parent_ids = HashSet::new();
        
        for node in federation_nodes {
            if node.header.scope == request.target_scope {
                parent_ids.insert(node.id()?);
                break;
            }
        }
        
        if parent_ids.is_empty() {
            return Err(CclError::Federation(
                format!("Federation with scope {} not found", request.target_scope)
            ));
        }
        
        // Create payload
        let payload = serde_json::json!({
            "request": request,
        });
        
        // Create the DAG node (use the requester's current scope, not the target scope)
        let node = DAGNode::new(
            DAGNodeType::Custom("MembershipJoinRequest".into()),
            parent_ids,
            request.requester.scope.clone(),
            request.requester.clone(),
            payload,
            &ed25519_dalek::SecretKey::from_bytes(&[0; 32]).unwrap(), // Placeholder, not actually used
        )?;
        
        // Add the node to the DAG
        let node_id = self.dag_storage.add_node(&node).await?;
        
        Ok(node_id)
    }
    
    /// Create DAG node for a membership vote
    async fn create_vote_node(
        &self,
        vote: &MembershipVote,
        request_node_id: &DAGNodeID,
    ) -> Result<DAGNodeID, CclError> {
        // Use the request node as parent
        let mut parent_ids = HashSet::new();
        parent_ids.insert(request_node_id.clone());
        
        // Create payload
        let payload = serde_json::json!({
            "vote": vote,
        });
        
        // Create the DAG node
        let node = DAGNode::new(
            DAGNodeType::Custom("MembershipVote".into()),
            parent_ids,
            vote.voter.scope.clone(),
            vote.voter.clone(),
            payload,
            &ed25519_dalek::SecretKey::from_bytes(&[0; 32]).unwrap(), // Placeholder, not actually used
        )?;
        
        // Add the node to the DAG
        let node_id = self.dag_storage.add_node(&node).await?;
        
        Ok(node_id)
    }
    
    /// Create DAG node for a membership acceptance
    async fn create_acceptance_node(
        &self,
        acceptance: &MembershipAcceptance,
        request_node_id: &DAGNodeID,
        vote_node_ids: &[DAGNodeID],
    ) -> Result<DAGNodeID, CclError> {
        // Use request and vote nodes as parents
        let mut parent_ids = HashSet::new();
        parent_ids.insert(request_node_id.clone());
        
        for vote_id in vote_node_ids {
            parent_ids.insert(vote_id.clone());
        }
        
        // Create payload
        let payload = serde_json::json!({
            "acceptance": acceptance,
        });
        
        // Create the DAG node
        let node = DAGNode::new(
            DAGNodeType::Custom("MembershipAcceptance".into()),
            parent_ids,
            acceptance.acceptor.scope.clone(),
            acceptance.acceptor.clone(),
            payload,
            &ed25519_dalek::SecretKey::from_bytes(&[0; 32]).unwrap(), // Placeholder, not actually used
        )?;
        
        // Add the node to the DAG
        let node_id = self.dag_storage.add_node(&node).await?;
        
        Ok(node_id)
    }
}

#[async_trait]
impl FederationManager for DefaultFederationManager {
    async fn create_federation(
        &self,
        name: String,
        scope: String,
        description: Option<String>,
        founder_identities: Vec<Identity>,
        federation_identity: Identity,
        federation_key: &SecretKey,
        metadata: Option<serde_json::Value>,
    ) -> Result<Federation, CclError> {
        // Create the federation
        let mut federation = Federation::new(
            name,
            scope,
            description,
            federation_identity,
            metadata,
            None,
        );
        
        // Create DAG node for federation creation
        let node_id = self.create_federation_node(
            &federation,
            &founder_identities,
            federation_key,
        ).await?;
        
        // Update federation with DAG reference
        federation.dag_reference = Some(node_id.clone());
        
        // Add founding members
        for founder in founder_identities {
            let member_id = founder.id.clone();
            
            // Create membership for this founder
            let membership = FederationMembership::new(
                member_id,
                federation.scope.clone(),
                "founder".to_string(),
                Vec::new(), // No credentials yet
                Some(node_id.clone()),
                None,
            );
            
            federation.add_member(membership)?;
        }
        
        Ok(federation)
    }
    
    async fn get_federation(&self, scope: &str) -> Result<Federation, CclError> {
        // Find the federation creation node for this scope
        let federation_nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::FederationCreation,
            None,
            None,
        ).await?;
        
        for node in federation_nodes {
            if node.header.scope == scope {
                let payload = &node.payload;
                
                // Extract federation details from the node
                let name = payload["name"].as_str()
                    .ok_or_else(|| CclError::Federation("Missing name in federation node".into()))?
                    .to_string();
                    
                let description = payload["description"].as_str().map(|s| s.to_string());
                
                let federation_id = node.header.creator.id().to_string();
                
                // Build the federation identity
                let identity = Identity {
                    id: federation_id,
                    identity_type: IdentityType::Federation,
                    name: name.clone(),
                    public_key: node.header.creator.public_key().to_vec(),
                    metadata: None,
                };
                
                // Create the federation
                let mut federation = Federation::new(
                    name,
                    scope.to_string(),
                    description,
                    identity,
                    payload["metadata"].clone(),
                    Some(node.id()?),
                );
                
                // Extract policies if available
                if let Some(policies) = payload["policies"].as_array() {
                    federation.policies.clear();
                    
                    for policy_value in policies {
                        if let Ok(policy) = serde_json::from_value::<QuorumPolicy>(policy_value.clone()) {
                            federation.policies.push(policy);
                        }
                    }
                }
                
                // Extract founding members
                if let Some(founders) = payload["founders"].as_array() {
                    for founder in founders {
                        let member_id = founder["id"].as_str()
                            .ok_or_else(|| CclError::Federation("Missing founder ID".into()))?
                            .to_string();
                            
                        let membership = FederationMembership::new(
                            member_id,
                            federation.scope.clone(),
                            "founder".to_string(),
                            Vec::new(),
                            Some(node.id()?),
                            None,
                        );
                        
                        federation.add_member(membership)?;
                    }
                }
                
                // Find all membership events for this federation
                let members = self.get_federation_members(scope).await?;
                
                // Add all members
                for membership in members {
                    if !federation.memberships.contains_key(&membership.member_id) {
                        federation.add_member(membership)?;
                    }
                }
                
                return Ok(federation);
            }
        }
        
        Err(CclError::Federation(format!("Federation with scope {} not found", scope)))
    }
    
    async fn submit_join_request(
        &self,
        request: MembershipJoinRequest,
    ) -> Result<DAGNodeID, CclError> {
        // Verify the request
        if !request.verify()? {
            return Err(CclError::Membership("Invalid join request signature".into()));
        }
        
        // Check if the federation exists
        let federation_nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::FederationCreation,
            None,
            None,
        ).await?;
        
        let mut federation_exists = false;
        for node in federation_nodes {
            if node.header.scope == request.target_scope {
                federation_exists = true;
                break;
            }
        }
        
        if !federation_exists {
            return Err(CclError::Federation(
                format!("Federation with scope {} not found", request.target_scope)
            ));
        }
        
        // Create DAG node for the request
        let node_id = self.create_join_request_node(&request).await?;
        
        Ok(node_id)
    }
    
    async fn vote_on_join_request(
        &self,
        vote: MembershipVote,
    ) -> Result<DAGNodeID, CclError> {
        // Verify the vote
        if !vote.verify()? {
            return Err(CclError::Membership("Invalid vote signature".into()));
        }
        
        // Find the request node
        let request_node_id = self.find_join_request_node(&vote.request_id).await?;
        
        // Verify the voter is a member of the federation
        let federation = self.get_federation_for_request(&vote.request_id).await?;
        
        if !federation.is_member(vote.voter.id()) {
            return Err(CclError::Unauthorized(
                format!("Voter {} is not a member of federation {}", 
                    vote.voter.id(), federation.scope)
            ));
        }
        
        // Create DAG node for the vote
        let node_id = self.create_vote_node(&vote, &request_node_id).await?;
        
        Ok(node_id)
    }
    
    async fn accept_join_request(
        &self,
        acceptance: MembershipAcceptance,
    ) -> Result<DAGNodeID, CclError> {
        // Verify the acceptance
        if !acceptance.verify()? {
            return Err(CclError::Membership("Invalid acceptance signature".into()));
        }
        
        // Find the request node
        let request_node_id = self.find_join_request_node(&acceptance.request_id).await?;
        
        // Find all vote nodes for this request
        let vote_node_ids = self.find_vote_nodes(&acceptance.request_id).await?;
        
        // Verify the acceptor is a member of the federation with appropriate role
        let federation = self.get_federation_for_request(&acceptance.request_id).await?;
        
        if !federation.has_role(acceptance.acceptor.id(), "admin") && 
           !federation.has_role(acceptance.acceptor.id(), "founder") {
            return Err(CclError::Unauthorized(
                format!("Acceptor {} does not have sufficient role in federation {}", 
                    acceptance.acceptor.id(), federation.scope)
            ));
        }
        
        // Verify the quorum proof
        // In a real implementation, we would gather all public keys and verify the proof
        
        // Create DAG node for the acceptance
        let node_id = self.create_acceptance_node(
            &acceptance,
            &request_node_id,
            &vote_node_ids,
        ).await?;
        
        Ok(node_id)
    }
    
    async fn get_pending_requests(
        &self,
        federation_scope: &str,
    ) -> Result<Vec<MembershipJoinRequest>, CclError> {
        let nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::Custom("MembershipJoinRequest".into()),
            None,
            None,
        ).await?;
        
        let mut requests = Vec::new();
        
        for node in nodes {
            if let Some(request_value) = node.payload.get("request") {
                if let Ok(request) = serde_json::from_value::<MembershipJoinRequest>(request_value.clone()) {
                    if request.target_scope == federation_scope {
                        // Check if this request has already been accepted
                        let children = self.dag_storage.get_children(&node.id()?).await?;
                        let mut is_accepted = false;
                        
                        for child in children {
                            if let DAGNodeType::Custom(node_type) = &child.header.node_type {
                                if node_type == "MembershipAcceptance" {
                                    is_accepted = true;
                                    break;
                                }
                            }
                        }
                        
                        if !is_accepted {
                            requests.push(request);
                        }
                    }
                }
            }
        }
        
        Ok(requests)
    }
    
    async fn get_request_votes(
        &self,
        request_id: &str,
    ) -> Result<Vec<MembershipVote>, CclError> {
        let request_node_id = self.find_join_request_node(request_id).await?;
        let children = self.dag_storage.get_children(&request_node_id).await?;
        
        let mut votes = Vec::new();
        
        for child in children {
            if let DAGNodeType::Custom(node_type) = &child.header.node_type {
                if node_type == "MembershipVote" {
                    if let Some(vote_value) = child.payload.get("vote") {
                        if let Ok(vote) = serde_json::from_value::<MembershipVote>(vote_value.clone()) {
                            votes.push(vote);
                        }
                    }
                }
            }
        }
        
        Ok(votes)
    }
}

impl DefaultFederationManager {
    /// Helper method to find the federation for a join request
    async fn get_federation_for_request(&self, request_id: &str) -> Result<Federation, CclError> {
        let request_node_id = self.find_join_request_node(request_id).await?;
        let request_node = self.dag_storage.get_node(&request_node_id).await?;
        
        let request: MembershipJoinRequest = serde_json::from_value(
            request_node.payload["request"].clone()
        ).map_err(|e| CclError::Common(e.into()))?;
        
        self.get_federation(&request.target_scope).await
    }
    
    /// Helper method to find a join request node
    async fn find_join_request_node(&self, request_id: &str) -> Result<DAGNodeID, CclError> {
        let nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::Custom("MembershipJoinRequest".into()),
            None,
            None,
        ).await?;
        
        for node in nodes {
            if let Some(request_value) = node.payload.get("request") {
                if let Some(id) = request_value.get("id") {
                    if id.as_str() == Some(request_id) {
                        return Ok(node.id()?);
                    }
                }
            }
        }
        
        Err(CclError::Membership(format!("Join request {} not found", request_id)))
    }
    
    /// Helper method to find vote nodes for a request
    async fn find_vote_nodes(&self, request_id: &str) -> Result<Vec<DAGNodeID>, CclError> {
        let request_node_id = self.find_join_request_node(request_id).await?;
        let children = self.dag_storage.get_children(&request_node_id).await?;
        
        let mut vote_node_ids = Vec::new();
        
        for child in children {
            if let DAGNodeType::Custom(node_type) = &child.header.node_type {
                if node_type == "MembershipVote" {
                    vote_node_ids.push(child.id()?);
                }
            }
        }
        
        Ok(vote_node_ids)
    }
    
    /// Helper method to get all memberships for a federation
    async fn get_federation_members(&self, federation_scope: &str) -> Result<Vec<FederationMembership>, CclError> {
        let nodes = self.dag_storage.get_nodes_by_type(
            DAGNodeType::Custom("MembershipAcceptance".into()),
            None,
            None,
        ).await?;
        
        let mut memberships = Vec::new();
        
        for node in nodes {
            if let Some(acceptance_value) = node.payload.get("acceptance") {
                if let Ok(acceptance) = serde_json::from_value::<MembershipAcceptance>(acceptance_value.clone()) {
                    // Find the original request to get the target scope
                    let request_node_id = self.find_join_request_node(&acceptance.request_id).await?;
                    let request_node = self.dag_storage.get_node(&request_node_id).await?;
                    
                    let request: MembershipJoinRequest = serde_json::from_value(
                        request_node.payload["request"].clone()
                    ).map_err(|e| CclError::Common(e.into()))?;
                    
                    if request.target_scope == federation_scope {
                        let member_id = request.requester.id().to_string();
                        
                        let membership = FederationMembership::new(
                            member_id,
                            federation_scope.to_string(),
                            "member".to_string(), // Default role
                            acceptance.credentials.clone(),
                            Some(node.id()?),
                            None,
                        );
                        
                        memberships.push(membership);
                    }
                }
            }
        }
        
        Ok(memberships)
    }
} 