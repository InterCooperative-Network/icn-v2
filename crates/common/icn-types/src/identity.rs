use ed25519_dalek::PublicKey;
use serde::{Deserialize, Serialize};

/// Represents a Decentralized Identifier, currently supporting did:key with Ed25519.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Did {
    // Store the public key bytes for serialization.
    // Consider using a dedicated DID type library in the future.
    public_key_bytes: Vec<u8>,
    // Method is implicitly "key" for now
}

impl Did {
    pub fn new(public_key: &PublicKey) -> Self {
        Did {
            public_key_bytes: public_key.to_bytes().to_vec(),
        }
    }

    // Basic string representation (not full did:key spec)
    pub fn to_string(&self) -> String {
        format!("did:key:z{}", base64::encode(&self.public_key_bytes)) // Needs proper multicodec prefix
    }

    // Method to get the raw public key bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key_bytes
    }
} 