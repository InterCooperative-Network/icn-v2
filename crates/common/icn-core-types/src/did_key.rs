use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH
};
use crate::did::Did; // Use Did from this crate
use thiserror::Error;
use multibase::Base;
use rand::rngs::OsRng;
use std::convert::TryInto;

// --- DidKeyError --- 
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

// --- DidKey struct --- 
/// Manages an Ed25519 keypair (SigningKey + VerifyingKey) associated with a DID.
#[derive(Debug)]
pub struct DidKey {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    did: Did, 
}

impl DidKey {
    const ED25519_MULTICODEC_PREFIX: &'static [u8] = &[0xed, 0x01];

    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let did = Did::new(&verifying_key); 
        DidKey { signing_key, verifying_key: verifying_key.clone(), did }
    }

    pub fn did(&self) -> &Did {
        &self.did
    }

    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), DidKeyError> {
        self.verifying_key.verify(message, signature).map_err(DidKeyError::VerificationError)
    }

    pub fn to_did_string(&self) -> String {
       self.did.to_string() 
    }

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