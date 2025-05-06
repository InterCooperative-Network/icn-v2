use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH
};
use serde::{Deserialize, Serialize};
// use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE; // Removed unused
// use base64::Engine; // Removed unused
use std::fmt; // Import fmt
use std::convert::TryInto; // Import TryInto
use thiserror::Error; // Import thiserror
use multibase::Base; // Import Base
use rand::rngs::OsRng; // Import OsRng

// REMOVED Did struct and impl

// Keep DidKey struct if it was here (assuming it wasn't, as it's in core)

// Keep other identity-related items if any 

// --- Added DidKeyError --- 
#[derive(Error, Debug)]
pub enum DidKeyError {
    #[error("Signature verification failed")]
    VerificationError(#[from] ed25519_dalek::SignatureError),
    #[error("Invalid key bytes length: expected {expected}, got {got}")]
    InvalidKeyBytesLength { expected: usize, got: usize },
    #[error("Invalid DID string: {0}")]
    InvalidDidString(String),
    #[error("Unsupported DID method: {0}")]
    UnsupportedDidMethod(String),
    #[error("Invalid multibase encoding: {0}")]
    InvalidMultibase(#[from] multibase::Error),
    #[error("Invalid multicodec prefix: expected 0xed01, got {0:?}")]
    InvalidMulticodecPrefix(Vec<u8>),
}

// --- Added DidKey struct --- 
/// Manages an Ed25519 keypair (SigningKey + VerifyingKey) associated with a DID.
#[derive(Debug)]
pub struct DidKey {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    did: Did, // Use Did from this crate
}

impl DidKey {
    /// Multicodec prefix for Ed25519 public keys (0xed01)
    const ED25519_MULTICODEC_PREFIX: &'static [u8] = &[0xed, 0x01];

    /// Generate a new DidKey using OS randomness.
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        // Use Did::new from this crate (needs to be defined or re-imported)
        // For now, assume Did struct is available here or re-exported
        let did = Did::new(&verifying_key); 
        DidKey { signing_key, verifying_key: verifying_key.clone(), did }
    }

    /// Get the DID associated with this keypair.
    pub fn did(&self) -> &Did {
        &self.did
    }

    /// Get the verifying key (public key).
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Sign a message using the secret key.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Verify a signature against the public key.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), DidKeyError> {
        self.verifying_key.verify(message, signature).map_err(DidKeyError::VerificationError)
    }

    /// Get the DID string representation (did:key:z...).
    pub fn to_did_string(&self) -> String {
       self.did.to_string() // Use Did::to_string from this crate
    }

    /// Create a verifier (VerifyingKey) from a did:key string.
    pub fn verifying_key_from_did(did_str: &str) -> Result<VerifyingKey, DidKeyError> {
        if !did_str.starts_with("did:key:") {
            return Err(DidKeyError::UnsupportedDidMethod(did_str.to_string()));
        }
        let encoded_key = &did_str[8..];

        let (base, decoded_bytes) = multibase::decode(encoded_key)?;
        if base != Base::Base58Btc {
            return Err(DidKeyError::InvalidDidString("Expected base58btc encoding (prefix 'z')".to_string()));
        }

        if !decoded_bytes.starts_with(Self::ED25519_MULTICODEC_PREFIX) {
            return Err(DidKeyError::InvalidMulticodecPrefix(decoded_bytes.get(..2).unwrap_or_default().to_vec()));
        }

        let prefix_len = Self::ED25519_MULTICODEC_PREFIX.len();
        let key_bytes = decoded_bytes.get(prefix_len..)
            .ok_or(DidKeyError::InvalidKeyBytesLength { expected: PUBLIC_KEY_LENGTH + prefix_len, got: decoded_bytes.len() })?;

        let key_array: &[u8; PUBLIC_KEY_LENGTH] = key_bytes
            .try_into()
            .map_err(|_| DidKeyError::InvalidKeyBytesLength { expected: PUBLIC_KEY_LENGTH, got: key_bytes.len()})?;

        VerifyingKey::from_bytes(key_array).map_err(DidKeyError::VerificationError)
    }
}

// --- Need Did definition here --- 
// Re-add the Did struct definition temporarily until restructuring is complete
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