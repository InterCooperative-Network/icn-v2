use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer, Signature, Verifier};
use icn_types::Did;
use rand::rngs::OsRng;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DidKeyError {
    #[error("Signature verification failed")]
    VerificationError,
    #[error("Invalid key bytes: {0}")]
    InvalidKeyBytes(String),
}

/// Manages an Ed25519 keypair associated with a DID.
#[derive(Debug)] // Avoid Clone for keypairs
pub struct DidKey {
    keypair: Keypair,
    did: Did,
}

impl DidKey {
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
        self.keypair.sign(message)
    }

    /// Verify a signature against the public key.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), DidKeyError> {
        self.keypair
            .public
            .verify(message, signature)
            .map_err(|_| DidKeyError::VerificationError)
    }

    // Add methods for loading/saving keypairs securely if needed
}

// Implement necessary traits like Serialize/Deserialize carefully for key material 