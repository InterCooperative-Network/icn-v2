use serde::{Serialize, Deserialize};
use crate::dag::NodeScope;
use crate::Did;

/// Configuration for policy enforcement within a specific scope
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScopePolicyConfig {
    /// Type of scope this policy applies to
    pub scope_type: NodeScope,
    
    /// Identifier of the scope (e.g., "coop-abc")
    pub scope_id: String,
    
    /// List of rules defining allowed actions within this scope
    pub allowed_actions: Vec<PolicyRule>
}

impl ScopePolicyConfig {
    /// Generate a canonical byte representation for consistent hashing and signing
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        // Use canonical JSON serialization for consistent representation
        serde_json::to_vec(self)
            .map_err(|e| format!("Failed to serialize policy config: {}", e))
    }
    
    /// Convert to a JSON string representation for storage in DAG
    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize policy config to JSON string: {}", e))
    }
    
    /// Create a policy from a JSON string representation
    pub fn from_json_string(json: &str) -> Result<Self, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Failed to deserialize policy config from JSON: {}", e))
    }
}

/// Rule defining who can perform a specific action
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PolicyRule {
    /// Type of action being regulated (e.g., "submit_proposal", "mint_token")
    pub action_type: String,
    
    /// Optional federation membership requirement
    pub required_membership: Option<String>,
    
    /// Optional explicit list of allowed DIDs
    pub allowed_dids: Option<Vec<Did>>,
}

/// Error types for policy enforcement operations
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PolicyError {
    #[error("The requested action is not permitted by policy")]
    ActionNotPermitted,
    
    #[error("Unauthorized access to scope")]
    UnauthorizedScopeAccess,
    
    #[error("DID not in allowlist")]
    DidNotInAllowlist,
    
    #[error("Policy not found for scope")]
    PolicyNotFound,
    
    #[error("Internal error: {0}")]
    InternalError(String),
} 