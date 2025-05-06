// Re-exporting the Cid type from the cid crate for now.
// We might create a wrapper struct later if we need custom logic.
pub use cid::Cid as ExternalCid;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::ops::Deref;
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with CIDs
#[derive(Error, Debug)]
pub enum CidError {
    #[error("Failed to parse CID from bytes: {0}")]
    ParseError(String),
}

/// A wrapper around the `cid::Cid` type to provide Serialize/Deserialize implementations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cid(ExternalCid);

impl Cid {
    /// Create a temporary placeholder CID from bytes
    /// 
    /// Note: This is a temporary implementation. In a production system,
    /// we would use a proper content-addressed hashing scheme.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CidError> {
        // Hash the data using SHA-256 via the multihash crate
        // In a real implementation, we would use IPLD properly
        use multihash::{Code, MultihashDigest};
        
        // Generate a multihash using SHA-256
        let hash = Code::Sha2_256.digest(data);
        
        // Create a CIDv1 with the hash using the RAW codec (0x55)
        let cid = ExternalCid::new_v1(0x55, hash);
        
        Ok(Cid(cid))
    }
    
    /// Get the raw bytes of this CID
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

// --- Deref to access inner Cid methods --- 
impl Deref for Cid {
    type Target = ExternalCid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// --- Conversions ---
impl From<ExternalCid> for Cid {
    fn from(cid: ExternalCid) -> Self {
        Cid(cid)
    }
}

impl From<Cid> for ExternalCid {
    fn from(cid: Cid) -> Self {
        cid.0
    }
}

// --- Serde Implementations ---

impl Serialize for Cid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as bytes
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de> Deserialize<'de> for Cid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize from bytes
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        ExternalCid::try_from(bytes)
            .map(Cid)
            .map_err(serde::de::Error::custom)
    }
}

// Implement Display to format the CID as a string
impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
} 