use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use sha2::{Sha256, Digest};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventIdError {
    #[error("Invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("Invalid length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub [u8; 32]); // SHA-256 hash

impl EventId {
    /// Create a new EventId by hashing the provided bytes
    pub fn new(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut array = [0u8; 32];
        array.copy_from_slice(&result);
        EventId(array)
    }
    
    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Convert to a hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl Default for EventId {
    fn default() -> Self {
        // Create a default zero-filled EventId
        EventId([0u8; 32])
    }
}

impl FromStr for EventId {
    type Err = EventIdError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse from hex string
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(EventIdError::InvalidLength(bytes.len()));
        }
        
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(EventId(array))
    }
}

// Add From<String> implementation
impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or_default()
    }
}

// Add From<&str> implementation
impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_default()
    }
}

impl fmt::Debug for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventId({})", self.to_hex())
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl From<[u8; 32]> for EventId {
    fn from(bytes: [u8; 32]) -> Self {
        EventId(bytes)
    }
} 