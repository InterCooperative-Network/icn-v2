use ed25519_dalek::VerifyingKey; // Use VerifyingKey instead of PublicKey
use serde::{Deserialize, Serialize};
// use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE; // Removed unused
// use base64::Engine; // Removed unused
use std::fmt; // Import fmt
use std::convert::TryInto; // Import TryInto

/// Represents a Decentralized Identifier, currently supporting did:key with Ed25519.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Did {
    // Store the public key bytes for serialization.
    // Consider using a dedicated DID type library in the future.
    public_key_bytes: Vec<u8>,
    // Method is implicitly "key" for now
}

impl Did {
    pub fn new(verifying_key: &VerifyingKey) -> Self {
        Did {
            public_key_bytes: verifying_key.to_bytes().to_vec(),
        }
    }

    // Basic string representation (did:key spec)
    pub fn to_string(&self) -> String {
        // Correct did:key format requires multicodec prefix for ed25519 pub key (0xed)
        // followed by the public key bytes, then multibase encoded (base58btc -> 'z')
        let mut prefixed_key = vec![0xed, 0x01]; // ed25519-pub multicodec prefix (variable length 0xed, 0x01 = 34 bytes key)
        prefixed_key.extend_from_slice(&self.public_key_bytes);
        format!("did:key:z{}", multibase::encode(multibase::Base::Base58Btc, prefixed_key))
    }

    // Method to get the raw public key bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key_bytes
    }

    // Add a method to attempt conversion back to VerifyingKey
    pub fn to_verifying_key(&self) -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
        let key_bytes: &[u8; 32] = self.public_key_bytes[..]
            .try_into()
            .map_err(|_| ed25519_dalek::SignatureError::new())?; // Convert slice to array, handle potential length error
        VerifyingKey::from_bytes(key_bytes)
    }
}

// Implement Display trait for use in errors etc.
impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
} 