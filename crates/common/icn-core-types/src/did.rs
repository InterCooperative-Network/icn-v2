use ed25519_dalek::{VerifyingKey, PUBLIC_KEY_LENGTH};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::convert::TryInto;
use std::str::FromStr;
use multibase::Base;
use thiserror::Error;
use multibase::encode;

// Define a specific error for parsing
#[derive(Error, Debug)]
pub enum DidParseError {
    #[error("Unsupported DID method: {0}")]
    UnsupportedDidMethod(String),
    #[error("Invalid multibase format: {0}")]
    InvalidMultibase(#[from] multibase::Error),
    #[error("Unsupported base encoding for did:key: expected Base58Btc, got {0}")]
    UnsupportedBaseForDidKey(String),
    #[error("Invalid multicodec prefix: expected 0xed01, got {0:?}")]
    InvalidMulticodecPrefix(Vec<u8>),
    #[error("Invalid key bytes length: expected {expected}, got {got}")]
    InvalidKeyBytesLength { expected: usize, got: usize },
}

/// Represents a Decentralized Identifier, currently supporting did:key with Ed25519.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Did {
    public_key_bytes: Vec<u8>,
}

impl Did {
    const ED25519_MULTICODEC_PREFIX: &'static [u8] = &[0xed, 0x01];

    pub fn new(verifying_key: &VerifyingKey) -> Self {
        Did { public_key_bytes: verifying_key.to_bytes().to_vec() }
    }
    pub fn to_string(&self) -> String {
        let mut prefixed_key = vec![0xed, 0x01]; 
        prefixed_key.extend_from_slice(&self.public_key_bytes);
        format!("did:key:z{}", multibase::encode(multibase::Base::Base58Btc, prefixed_key))
    }
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key_bytes
    }
    pub fn to_verifying_key(&self) -> Option<VerifyingKey> {
        let s = self.to_string();
        if !s.starts_with("did:key:") { return None; }
        let (_prefix, b58) = s.split_at(8);
        let decoded_multibase = multibase::decode(b58).ok()?;
        let (_base, data_bytes) = decoded_multibase;
        
        if data_bytes.len() != 34 || data_bytes[0] != 0xED || data_bytes[1] != 0x01 { return None; }
        
        VerifyingKey::from_bytes(data_bytes[2..].try_into().ok()?).ok()
    }

    /// Parse a DID string (e.g., did:key:z...) into a Did object
    pub fn from_string(did_str: &str) -> Result<Self, DidParseError> {
        if !did_str.starts_with("did:key:") {
            return Err(DidParseError::UnsupportedDidMethod(did_str.to_string()));
        }
        let encoded_key = &did_str[8..];

        let (base, decoded_bytes) = multibase::decode(encoded_key)?;
        if base != Base::Base58Btc {
            return Err(DidParseError::UnsupportedBaseForDidKey(format!("{:?}", base)));
        }

        if !decoded_bytes.starts_with(Self::ED25519_MULTICODEC_PREFIX) {
            return Err(DidParseError::InvalidMulticodecPrefix(decoded_bytes.get(..2).unwrap_or_default().to_vec()));
        }

        let prefix_len = Self::ED25519_MULTICODEC_PREFIX.len();
        let key_bytes = decoded_bytes.get(prefix_len..)
            .ok_or(DidParseError::InvalidKeyBytesLength { expected: PUBLIC_KEY_LENGTH + prefix_len, got: decoded_bytes.len() })?;

        if key_bytes.len() != PUBLIC_KEY_LENGTH {
             return Err(DidParseError::InvalidKeyBytesLength { expected: PUBLIC_KEY_LENGTH, got: key_bytes.len() });
        }

        Ok(Did { public_key_bytes: key_bytes.to_vec() })
    }

    /// Construct `did:key:z...` from a raw 32-byte Ed25519 public key
    pub fn from_verifying_key(vk: &VerifyingKey) -> Self {
        let mut data = vec![0xED, 0x01];
        data.extend_from_slice(vk.as_bytes());
        let mb = encode(Base::Base58Btc, data);
        format!("did:key:{}", mb).parse().expect("valid DID from verifying key construction")
    }
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

// Implement FromStr for Did to allow "foo".parse::<Did>() and String → Did conversion
impl FromStr for Did {
    type Err = DidParseError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Did::from_string(s)
    }
}

// Implement Default for Did to support unwrap_or_default() calls
impl Default for Did {
    fn default() -> Self {
        // Create a Did with all zeros (32 bytes) as the public key
        // This is safe because:
        // 1. It's deterministic
        // 2. It's not a valid key that someone would use in production
        // 3. It's clearly identifiable as a default value
        Did {
            public_key_bytes: vec![0u8; PUBLIC_KEY_LENGTH],
        }
    }
}

// Add From<String> implementation for convenience
impl From<String> for Did {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or_default()
    }
}

// Add From<&str> implementation for convenience with string literals
impl From<&str> for Did {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_default()
    }
} 