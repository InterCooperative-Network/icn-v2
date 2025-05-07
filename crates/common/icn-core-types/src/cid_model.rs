// extern crate cid; // Removed unnecessary extern crate

use cid::{Cid as ExternalCid, Version}; // Removed Codec from import
use multihash::Multihash;
use sha2::{Sha256, Digest};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::ops::Deref;
use std::str::FromStr;
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with CIDs
#[derive(Error, Debug)]
pub enum CidError {
    #[error("Failed to parse CID from bytes: {0}")]
    ParseError(String),
    #[error("Failed to parse CID from string: {0}")]
    FromStrError(String),
    // Add other Cid related errors if needed
}

/// A wrapper around the `cid::Cid` type to provide Serialize/Deserialize implementations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cid(ExternalCid);

impl Cid {
    /// Create a CID from raw bytes using a default hashing algorithm (SHA-256) and codec (Raw).
    pub fn from_bytes(data: &[u8]) -> Result<Self, CidError> {
        // Calculate hash using sha2 crate
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();
        
        // Wrap digest in a Multihash object
        // 0x12 is the multicodec code for sha2-256
        let mh = Multihash::wrap(0x12, &digest)
            .map_err(|e| CidError::ParseError(format!("Multihash wrap error: {}", e)))?; 
            
        // Use raw u64 code for Codec::Raw (0x55)
        let raw_codec_code = 0x55u64; 
        
        // Note: Cid::new expects MultihashGeneric<64>, check if Multihash::wrap provides compatible type
        let cid = ExternalCid::new(Version::V1, raw_codec_code, mh) 
            .map_err(|e| CidError::ParseError(e.to_string()))?; 
        Ok(Cid(cid))
    }
    
    /// Get the raw bytes of this CID
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

// Implement FromStr for Cid to allow "foo".parse::<Cid>() and String → Cid conversion
impl FromStr for Cid {
    type Err = CidError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle multibase prefixes if present
        let bytes = if s.starts_with("b") {
            // May be multibase-encoded, try to decode
            match multibase::decode(s) {
                Ok((_, bytes)) => bytes,
                Err(e) => return Err(CidError::FromStrError(format!("Multibase decode error: {}", e))),
            }
        } else {
            // Assume it's a standard CID string representation
            match ExternalCid::from_str(s) {
                Ok(cid) => return Ok(Cid(cid)),
                Err(e) => return Err(CidError::FromStrError(format!("CID parse error: {}", e))),
            }
        };
        
        // Try to parse from decoded bytes
        ExternalCid::try_from(bytes)
            .map(Cid)
            .map_err(|e| CidError::FromStrError(format!("Failed to parse CID from bytes: {}", e)))
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

// Implement Default for Cid
impl Default for Cid {
    fn default() -> Self {
        // Create a default CID using an empty byte array
        // This should only be used for default struct instantiation
        let bytes = [0u8; 32];
        Self::from_bytes(&bytes).unwrap_or_else(|_| {
            // Fallback in case of hash generation failure
            panic!("Failed to create default Cid, which should never happen")
        })
    }
}

// Add From<String> implementation for convenience
impl From<String> for Cid {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or_default()
    }
}

// Add From<&str> implementation for convenience with string literals
impl From<&str> for Cid {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_default()
    }
}

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