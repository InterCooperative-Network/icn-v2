use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer, Signature, Verifier, SIGNATURE_LENGTH, PUBLIC_KEY_LENGTH};
use icn_types::Did;
use rand::rngs::OsRng;
use thiserror::Error;
use multibase;

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

/// Manages an Ed25519 keypair associated with a DID.
#[derive(Debug)] // Avoid Clone for keypairs
pub struct DidKey {
    keypair: Keypair,
    did: Did,
}

impl DidKey {
    /// Multicodec prefix for Ed25519 public keys (0xed01)
    const ED25519_MULTICODEC_PREFIX: &'static [u8] = &[0xed, 0x01];

    /// Generate a new DidKey using OS randomness.
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        let did = Did::new(&keypair.public);
        DidKey { keypair, did }
    }

    /// Get the DID associated with this keypair.
    pub fn did(&self) -> &Did {
        &self.did
    }

    /// Get the public key.
    pub fn public_key(&self) -> &PublicKey {
        &self.keypair.public
    }

    /// Sign a message using the secret key.
    pub fn sign(&self, message: &[u8]) -> Signature {
        // Use sign_prehashed for larger messages if needed
        self.keypair.sign(message)
    }

    /// Verify a signature against the public key.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), DidKeyError> {
        self.keypair.public.verify(message, signature).map_err(DidKeyError::VerificationError)
    }

    /// Get the DID string representation (did:key:z...).
    pub fn to_did_string(&self) -> String {
       self.did.to_string() // Use the implementation from icn-types
    }

    /// Create a verifier (PublicKey) from a did:key string.
    pub fn public_key_from_did(did_str: &str) -> Result<PublicKey, DidKeyError> {
        if !did_str.starts_with("did:key:") {
            return Err(DidKeyError::UnsupportedDidMethod(did_str.to_string()));
        }
        let encoded_key = &did_str[8..]; // Skip "did:key:"
        if !encoded_key.starts_with('z') { // Check for base58btc encoding
             return Err(DidKeyError::InvalidDidString("Expected base58btc encoding (prefix 'z')".to_string()));
        }

        let decoded_bytes = multibase::decode(encoded_key)?;

        if !decoded_bytes.starts_with(Self::ED25519_MULTICODEC_PREFIX) {
            return Err(DidKeyError::InvalidMulticodecPrefix(decoded_bytes[..2].to_vec()));
        }

        let key_bytes = &decoded_bytes[Self::ED25519_MULTICODEC_PREFIX.len()..];

        PublicKey::from_bytes(key_bytes).map_err(|e| DidKeyError::VerificationError(e))

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
        assert!(did_key.verify(message, &signature).is_ok());
    }

    #[test]
    fn test_did_string_conversion_and_verification() {
        let did_key = DidKey::new();
        let did_string = did_key.to_did_string();

        assert!(did_string.starts_with("did:key:z"));

        let recovered_pk = DidKey::public_key_from_did(&did_string).expect("Failed to recover public key from DID");
        assert_eq!(did_key.public_key(), &recovered_pk);

        // Verify a signature using the recovered key
        let message = b"Another test";
        let signature = did_key.sign(message);
        assert!(recovered_pk.verify(message, &signature).is_ok());
    }

     #[test]
    fn test_invalid_did_parsing() {
        assert!(DidKey::public_key_from_did("did:example:123").is_err());
        assert!(DidKey::public_key_from_did("did:key:abc").is_err()); // Invalid multibase prefix
        // Invalid multicodec (needs correct length + prefix)
        let invalid_encoded = "z" .to_string() + &multibase::encode(multibase::Base::Base58Btc, &[0x01, 0x02]);
        assert!(DidKey::public_key_from_did(&format!("did:key:{}", invalid_encoded)).is_err());
    }
} 