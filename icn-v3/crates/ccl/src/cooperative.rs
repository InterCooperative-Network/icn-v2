use crate::error::CclError;
use icn_common::identity::{Identity, ScopedIdentity};

/// Cooperative structure
#[derive(Debug, Clone)]
pub struct Cooperative {
    /// Unique identifier
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Cooperative scope
    pub scope: String,
    
    /// Identity for this cooperative
    pub identity: Identity,
    
    // Additional fields would be implemented here
}

impl Cooperative {
    /// Create a new cooperative
    pub fn new(name: String, scope: String, identity: Identity) -> Self {
        Self {
            id: identity.id.clone(),
            name,
            scope,
            identity,
        }
    }
    
    /// Get the cooperative's identity as a scoped identity
    pub fn scoped_identity(&self) -> ScopedIdentity {
        ScopedIdentity::new(
            self.identity.clone(),
            self.scope.clone(),
            None,
        )
    }
}

/// Cooperative manager trait
pub trait CooperativeManager: Send + Sync + 'static {
    /// Create a new cooperative
    fn create_cooperative(
        &self,
        name: String,
        scope: String,
        identity: Identity,
    ) -> Result<Cooperative, CclError>;
    
    /// Get a cooperative by ID
    fn get_cooperative(&self, id: &str) -> Result<Cooperative, CclError>;
} 