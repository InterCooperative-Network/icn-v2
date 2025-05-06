use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH
};
use icn_core_types::Did;
use rand::rngs::OsRng;
use thiserror::Error;
use multibase::Base;

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

/// Manages an Ed25519 keypair (SigningKey + VerifyingKey) associated with a DID.
#[derive(Debug)]
pub struct DidKey {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    did: Did,
}

impl DidKey {
    /// Multicodec prefix for Ed25519 public keys (0xed01)
    const ED25519_MULTICODEC_PREFIX: &'static [u8] = &[0xed, 0x01];

    /// Generate a new DidKey using OS randomness.
    pub fn new() -> Self {
        let _csprng = OsRng; // Corrected line
        let signing_key: SigningKey = SigningKey::generate(&mut OsRng); // Corrected line
        let verifying_key = signing_key.verifying_key();
        let did = Did::new(&verifying_key);
        Self { signing_key, verifying_key, did }
    }

    pub fn from_signing_key(signing_key: SigningKey) -> Self {
        let _csprng = OsRng; // Also correct this one for consistency, though it wasn't warned
        let verifying_key = signing_key.verifying_key();
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

    /// Returns a reference to the underlying Ed25519 signing key.
    /// Use with care—this exposes private key material for direct signing.
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
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
       self.did.to_string() // Use the implementation from icn-types
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

    // TODO: Add methods for secure serialization/deserialization of the Keypair
    // e.g., using formats like PKCS#8 or JWK, potentially password-protected.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_key_generation_sign_verify() {
        let did_key = DidKey::new();
        let message = b"Test message for signing";
        let signature = did_key.sign(message);
        // Verify using the key itself
        assert!(did_key.verify(message, &signature).is_ok());
        // Verify using the verifying key directly
        assert!(did_key.verifying_key().verify(message, &signature).is_ok());
    }

    #[test]
    fn test_did_string_conversion_and_verification() {
        let did_key = DidKey::new();
        let did_string = did_key.to_did_string();

        assert!(did_string.starts_with("did:key:z"));

        let recovered_vk = DidKey::verifying_key_from_did(&did_string).expect("Failed to recover public key from DID");
        assert_eq!(did_key.verifying_key(), &recovered_vk);

        let message = b"Another test";
        let signature = did_key.sign(message);
        assert!(recovered_vk.verify(message, &signature).is_ok());
    }

     #[test]
    fn test_invalid_did_parsing() {
        assert!(DidKey::verifying_key_from_did("did:example:123").is_err());
        assert!(DidKey::verifying_key_from_did("did:key:abc").is_err()); // Invalid multibase prefix
        let invalid_encoded = "z".to_string() + &multibase::encode(multibase::Base::Base58Btc, &[0x01, 0x02]);
        assert!(DidKey::verifying_key_from_did(&format!("did:key:{}", invalid_encoded)).is_err());
    }
} 