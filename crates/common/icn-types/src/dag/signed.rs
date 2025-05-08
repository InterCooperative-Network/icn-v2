use crate::{Cid, Did}; // Assuming Cid and Did are re-exported or defined in icn-types' lib.rs
use ed25519_dalek::{VerifyingKey, SigningKey, Signature, Signer, Verifier};
use multihash::{Code, MultihashDigest};
use serde::{Serialize, Deserialize};
use thiserror::Error; // For DagError

// Assuming a codec constant like this might exist in your crate root or a codec module
// If not, we'll use 0x71 directly in compute_cid.
// pub const DAG_CBOR_CODEC: u64 = 0x71;

#[derive(Error, Debug)]
pub enum DagError {
    #[error("CID mismatch")]
    CidMismatch,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid key")]
    InvalidKey,
    #[error("Serialization/Deserialization error: {0}")]
    Serde(String),
    #[error("Key resolution failed for DID: {0}")]
    KeyResolutionFailed(String),
    // Add other error variants as needed
}

impl From<serde_ipld_dagcbor::Error> for DagError {
    fn from(e: serde_ipld_dagcbor::Error) -> Self {
        DagError::Serde(e.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DagNode {
    pub payload: DagPayload,
    pub author: Did,
    pub timestamp: i64, // millis
                        // ... any other governance attrs
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DagPayload {
    RawData { bytes: Vec<u8> },
    // Proposal { ... }
    // Receipt  { ... }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignedDagNode {
    pub node: DagNode,
    pub cid: Cid,
    pub signer: Did,
    pub signature: Vec<u8>,
}

impl DagNode {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DagError> {
        Ok(serde_ipld_dagcbor::to_vec(self)?)
    }

    pub fn compute_cid(&self) -> Result<Cid, DagError> {
        let bytes = self.canonical_bytes()?;
        let hash = Code::Sha2_256.digest(&bytes);
        // Assuming crate::codec::DAG_CBOR or similar exists. If not, use 0x71.
        // For now, directly using the common DAG-CBOR codec ID 0x71.
        Ok(Cid::new_v1(0x71, hash))
    }
}

impl SignedDagNode {
    /// Create-and-sign in one shot.
    pub fn sign(node: DagNode, sk: &SigningKey, signer_did: Did) -> Result<Self, DagError> {
        let cid = node.compute_cid()?;
        // The original snippet signed cid.hash().digest(). 
        // Typically, the signature is over the message itself (canonical_bytes or its hash), 
        // or in some IPLD contexts, over the CID bytes directly. 
        // Signing the CID's hash digest is a common pattern.
        let message_to_sign = cid.hash().digest(); 
        let sig = sk.sign(message_to_sign);
        Ok(Self {
            node,
            cid,
            signer: signer_did,
            signature: sig.to_bytes().into(),
        })
    }

    pub fn verify_cid(&self) -> Result<(), DagError> {
        let expected = self.node.compute_cid()?;
        if expected == self.cid {
            Ok(())
        } else {
            Err(DagError::CidMismatch)
        }
    }

    pub fn verify_signature(&self, resolver: &impl KeyResolver) -> Result<(), DagError> {
        self.verify_cid()?; // First, ensure the CID matches the node data.
        let pk_bytes = resolver.resolve(&self.signer)?;
        let vk = VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|_| DagError::InvalidKey)?;
        let sig = Signature::from_bytes(&self.signature)
            .map_err(|_| DagError::InvalidSignature)?;
        
        // Verify the signature against the same message that was signed.
        // Following the .sign() method, this is cid.hash().digest().
        let message_to_verify = self.cid.hash().digest();
        vk.verify(message_to_verify, &sig)
            .map_err(|_| DagError::InvalidSignature)
    }
}

/// trait so CLI, wallet, or node can plug in DID→pubkey resolution
pub trait KeyResolver {
    fn resolve(&self, did: &Did) -> Result<[u8; 32], DagError>;
} 