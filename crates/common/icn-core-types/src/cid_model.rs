use cid::Cid as ExternalCid; // This should refer to the external crate
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
    // Add other Cid related errors if needed
}

/// A wrapper around the `cid::Cid` type to provide Serialize/Deserialize implementations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cid(ExternalCid);

impl Cid {
    /// Create a CID from raw bytes using a default hashing algorithm (SHA-256) and codec (Raw).
    pub fn from_bytes(data: &[u8]) -> Result<Self, CidError> {
        use cid::multihash::{Code, MultihashDigest}; // from external cid crate
        use cid::Version; // from external cid crate
        use cid::Codec;   // from external cid crate - no :: needed now due to module rename
        
        let hash = Code::Sha2_256.digest(data);
        let cid = ExternalCid::new(Version::V1, Codec::Raw, hash) // Use imported Codec
            .map_err(|e| CidError::ParseError(e.to_string()))?; 
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

// Implement TryFrom<&[u8]> for convenience if the underlying cid crate supports it
impl TryFrom<&[u8]> for Cid {
    type Error = CidError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        ExternalCid::try_from(bytes)
            .map(Cid)
            .map_err(|e| CidError::ParseError(e.to_string()))
    }
}

// Implement TryFrom<String> or &str if needed, parsing the string representation

// --- Serde Implementations ---
impl Serialize for Cid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.to_bytes())
    }
}

impl<'de> Deserialize<'de> for Cid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
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