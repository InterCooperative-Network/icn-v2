use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::convert::TryInto;

/// Represents a Decentralized Identifier, currently supporting did:key with Ed25519.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Did {
    public_key_bytes: Vec<u8>,
}

impl Did {
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
    pub fn to_verifying_key(&self) -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
        let key_bytes: &[u8; 32] = self.public_key_bytes[..].try_into().map_err(|_| ed25519_dalek::SignatureError::new())?;
        VerifyingKey::from_bytes(key_bytes)
    }
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
} 