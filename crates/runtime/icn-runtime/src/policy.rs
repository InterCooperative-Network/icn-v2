use icn_types::{Did, ScopePolicyConfig, PolicyError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Interface for tracking memberships of DIDs in federations, cooperatives, and communities
pub trait MembershipIndex: Send + Sync {
    /// Check if a DID is a member of the specified federation
    fn is_federation_member(&self, did: &Did, federation_id: &str) -> bool;
    
    /// Check if a DID is a member of the specified cooperative
    fn is_cooperative_member(&self, did: &Did, cooperative_id: &str) -> bool;
    
    /// Check if a DID is a member of the specified community
    fn is_community_member(&self, did: &Did, community_id: &str) -> bool;
    
    /// Check if a DID is a member of any federation (used for federation required checks)
    fn is_member_of_federation(&self, did: &Did, federation_id: &str) -> bool;
}

/// Interface for loading policy configurations
pub trait PolicyLoader: Send + Sync {
    /// Load policy configuration for a specific scope
    fn load_for_scope(&self, scope_type: &str, scope_id: &str) -> Result<ScopePolicyConfig, PolicyError>;
    
    /// Check if a DID is authorized to perform an action in a scope
    fn check_authorization(&self, scope_type: &str, scope_id: &str, action: &str, did: &Did) -> Result<(), PolicyError>;
}

/// Default implementation of the MembershipIndex trait
#[derive(Clone, Default)]
pub struct DefaultMembershipIndex {
    // Maps a DID to the set of federation_ids it belongs to
    did_to_federations: Arc<RwLock<HashMap<Did, Vec<String>>>>,
    // Maps a DID to the set of cooperative_ids it belongs to
    did_to_cooperatives: Arc<RwLock<HashMap<Did, Vec<String>>>>,
    // Maps a DID to the set of community_ids it belongs to
    did_to_communities: Arc<RwLock<HashMap<Did, Vec<String>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScopeType {
    Federation,
    Cooperative,
    Community,
}

impl DefaultMembershipIndex {
    /// Create a new empty membership index
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Register a DID as member of a federation
    pub fn add_federation_member(&self, did: Did, federation_id: String) {
        let mut federations = self.did_to_federations.write().unwrap();
        federations.entry(did).or_insert_with(Vec::new).push(federation_id);
    }
    
    /// Register a DID as member of a cooperative
    pub fn add_cooperative_member(&self, did: Did, cooperative_id: String) {
        let mut cooperatives = self.did_to_cooperatives.write().unwrap();
        cooperatives.entry(did).or_insert_with(Vec::new).push(cooperative_id);
    }
    
    /// Register a DID as member of a community
    pub fn add_community_member(&self, did: Did, community_id: String) {
        let mut communities = self.did_to_communities.write().unwrap();
        communities.entry(did).or_insert_with(Vec::new).push(community_id);
    }
}

impl MembershipIndex for DefaultMembershipIndex {
    /// Check if a DID is a member of the specified federation
    fn is_federation_member(&self, did: &Did, federation_id: &str) -> bool {
        let federations = self.did_to_federations.read().unwrap();
        federations.get(did)
            .map(|memberships| memberships.iter().any(|id| id == federation_id))
            .unwrap_or(false)
    }
    
    /// Check if a DID is a member of the specified cooperative
    fn is_cooperative_member(&self, did: &Did, cooperative_id: &str) -> bool {
        let cooperatives = self.did_to_cooperatives.read().unwrap();
        cooperatives.get(did)
            .map(|memberships| memberships.iter().any(|id| id == cooperative_id))
            .unwrap_or(false)
    }
    
    /// Check if a DID is a member of the specified community
    fn is_community_member(&self, did: &Did, community_id: &str) -> bool {
        let communities = self.did_to_communities.read().unwrap();
        communities.get(did)
            .map(|memberships| memberships.iter().any(|id| id == community_id))
            .unwrap_or(false)
    }
    
    /// Check if a DID is a member of any federation (used for federation required checks)
    fn is_member_of_federation(&self, did: &Did, federation_id: &str) -> bool {
        self.is_federation_member(did, federation_id)
    }
}

/// Default implementation of the PolicyLoader trait
#[derive(Clone, Default)]
pub struct DefaultPolicyLoader {
    // Maps (scope_type, scope_id) to policy
    policies: Arc<RwLock<HashMap<(String, String), ScopePolicyConfig>>>,
}

impl DefaultPolicyLoader {
    /// Create a new policy loader
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Store a policy configuration
    pub fn set_policy(&self, policy: ScopePolicyConfig) {
        let mut policies = self.policies.write().unwrap();
        let key = (format!("{:?}", policy.scope_type), policy.scope_id.clone());
        policies.insert(key, policy);
    }
}

impl PolicyLoader for DefaultPolicyLoader {
    /// Load policy configuration for a specific scope
    fn load_for_scope(&self, scope_type: &str, scope_id: &str) -> Result<ScopePolicyConfig, PolicyError> {
        let policies = self.policies.read().unwrap();
        let key = (scope_type.to_string(), scope_id.to_string());
        
        policies.get(&key)
            .cloned()
            .ok_or(PolicyError::PolicyNotFound)
    }
    
    /// Check if a DID is authorized to perform an action in a scope
    fn check_authorization(&self, scope_type: &str, scope_id: &str, action: &str, did: &Did) -> Result<(), PolicyError> {
        match self.load_for_scope(scope_type, scope_id) {
            Ok(policy) => evaluate_policy(&policy, action, did),
            Err(PolicyError::PolicyNotFound) => {
                // No policy defined for this scope, allow the operation by default
                // This can be changed to deny by default if desired
                Ok(())
            },
            Err(err) => Err(err),
        }
    }
}

/// Evaluate if a caller is authorized to perform an action based on policy
pub fn evaluate_policy(
    policy: &ScopePolicyConfig,
    action: &str,
    caller_did: &Did,
) -> Result<(), PolicyError> {
    // Find the rule for this action
    let rule = policy.allowed_actions.iter()
        .find(|r| r.action_type == action)
        .ok_or(PolicyError::ActionNotPermitted)?;
    
    // Check allowed DIDs list if specified
    if let Some(allowed) = &rule.allowed_dids {
        if !allowed.contains(caller_did) {
            return Err(PolicyError::DidNotInAllowlist);
        }
    }
    
    // If we pass all checks, the action is authorized
    Ok(())
} 