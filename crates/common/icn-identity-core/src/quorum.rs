use icn_types::{Did, QuorumProof, Cid};
use thiserror::Error;
use ed25519_dalek::{PublicKey, Signature, Verifier};
use std::collections::{HashMap, HashSet};

#[derive(Error, Debug)]
pub enum QuorumError {
    #[error("Signature verification failed for DID {did}: {source}")]
    SignatureError { did: Did, source: ed25519_dalek::SignatureError },
    #[error("Policy evaluation failed: {0}")]
    PolicyError(String),
    #[error("Quorum not met: required {required}, found {found}")]
    QuorumNotMet { required: usize, found: usize },
    #[error("Public key not found for DID {did}")]
    PublicKeyNotFound { did: Did },
    #[error("Duplicate signature found for DID {did}")]
    DuplicateSignature { did: Did },
    #[error("Data hash mismatch for proof CID {cid}")]
    DataHashMismatch { cid: Cid },
}

/// Defines the requirements for a quorum.
pub enum QuorumPolicy {
    /// Requires signatures from a specific majority of the listed DIDs.
    Majority(Vec<Did>),
    /// Requires a minimum number of signatures from any valid member (M of N).
    /// Assumes the set of valid signers (N) is known contextually or via another mechanism.
    Threshold { required: usize },
    /// Requires signatures from all listed DIDs.
    All(Vec<Did>),
}

/// Validates QuorumProofs against defined policies.
#[derive(Default)]
pub struct QuorumValidator {
    // Can be extended with policy storage or DID resolver later
}

impl QuorumValidator {
    pub fn new() -> Self {
        QuorumValidator::default()
    }

    /// Verify a QuorumProof against a policy, given resolved public keys and the signed data.
    pub fn validate_quorum(
        &self,
        proof: &QuorumProof,
        policy: &QuorumPolicy,
        public_keys: &HashMap<Did, PublicKey>,
        signed_data: &[u8],
    ) -> Result<(), QuorumError> {
        // Optional: Verify proof.data_cid matches hash of signed_data
        // let calculated_cid = calculate_cid(signed_data)?;
        // if calculated_cid != proof.data_cid {
        //     return Err(QuorumError::DataHashMismatch { cid: proof.data_cid.clone() });
        // }

        let mut valid_signers = HashSet::new();

        for (did, signature) in &proof.signatures {
            // Prevent duplicate signatures from the same DID
            if !valid_signers.insert(did.clone()) {
                return Err(QuorumError::DuplicateSignature { did: did.clone() });
            }

            let public_key = public_keys.get(did)
                .ok_or_else(|| QuorumError::PublicKeyNotFound { did: did.clone() })?;

            public_key.verify(signed_data, signature)
                .map_err(|e| QuorumError::SignatureError { did: did.clone(), source: e })?;
        }

        // Evaluate the policy against the set of valid signers
        match policy {
            QuorumPolicy::Majority(members) => {
                let required = (members.len() / 2) + 1;
                let found = members.iter().filter(|d| valid_signers.contains(*d)).count();
                if found < required {
                    return Err(QuorumError::QuorumNotMet { required, found });
                }
            }
            QuorumPolicy::Threshold { required } => {
                let found = valid_signers.len();
                 if found < *required {
                    return Err(QuorumError::QuorumNotMet { required: *required, found });
                }
            }
            QuorumPolicy::All(members) => {
                let required = members.len();
                let found = members.iter().filter(|d| valid_signers.contains(*d)).count();
                 if found < required {
                    // Could also check if *all* members signed, not just count
                    return Err(QuorumError::QuorumNotMet { required, found });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::did::DidKey;
    use std::collections::HashMap;
    use cid;
    use std::convert::TryFrom;

    fn create_test_cid(data: &[u8]) -> Cid {
        let hash = cid::multihash::Code::Sha2_256.digest(data);
        Cid::new_v1(cid::Codec::DagProtobuf, hash)
    }

    #[test]
    fn test_quorum_threshold_met() {
        let validator = QuorumValidator::new();
        let policy = QuorumPolicy::Threshold { required: 2 };

        let key1 = DidKey::new();
        let key2 = DidKey::new();
        let key3 = DidKey::new();

        let data = b"Quorum test data";
        let cid = create_test_cid(data);

        let sig1 = key1.sign(data);
        let sig2 = key2.sign(data);

        let proof = QuorumProof {
            data_cid: cid,
            policy_id: "test-policy".to_string(),
            signatures: vec![(key1.did().clone(), sig1), (key2.did().clone(), sig2)],
            metadata: None,
        };

        let mut public_keys = HashMap::new();
        public_keys.insert(key1.did().clone(), *key1.public_key());
        public_keys.insert(key2.did().clone(), *key2.public_key());
        public_keys.insert(key3.did().clone(), *key3.public_key());

        assert!(validator.validate_quorum(&proof, &policy, &public_keys, data).is_ok());
    }

     #[test]
    fn test_quorum_threshold_not_met() {
        let validator = QuorumValidator::new();
        let policy = QuorumPolicy::Threshold { required: 2 };

        let key1 = DidKey::new();

        let data = b"Quorum test data";
        let cid = create_test_cid(data);

        let sig1 = key1.sign(data);

        let proof = QuorumProof {
            data_cid: cid,
            policy_id: "test-policy".to_string(),
            signatures: vec![(key1.did().clone(), sig1)],
            metadata: None,
        };

        let mut public_keys = HashMap::new();
        public_keys.insert(key1.did().clone(), *key1.public_key());

        let result = validator.validate_quorum(&proof, &policy, &public_keys, data);
        assert!(result.is_err());
        match result.err().unwrap() {
            QuorumError::QuorumNotMet { required, found } => {
                assert_eq!(required, 2);
                assert_eq!(found, 1);
            }
            _ => panic!("Incorrect error type"),
        }
    }

     #[test]
    fn test_quorum_duplicate_signature() {
         let validator = QuorumValidator::new();
        let policy = QuorumPolicy::Threshold { required: 1 };
        let key1 = DidKey::new();
        let data = b"Quorum test data";
        let cid = create_test_cid(data);
        let sig1 = key1.sign(data);

        let proof = QuorumProof {
            data_cid: cid,
            policy_id: "test-policy".to_string(),
            signatures: vec![(key1.did().clone(), sig1.clone()), (key1.did().clone(), sig1)], // Duplicate
            metadata: None,
        };
         let mut public_keys = HashMap::new();
        public_keys.insert(key1.did().clone(), *key1.public_key());
        let result = validator.validate_quorum(&proof, &policy, &public_keys, data);
        assert!(matches!(result, Err(QuorumError::DuplicateSignature { .. })));
    }

     #[test]
    fn test_quorum_invalid_signature() {
        let validator = QuorumValidator::new();
        let policy = QuorumPolicy::Threshold { required: 1 };
        let key1 = DidKey::new();
        let key2 = DidKey::new(); // Signer
        let data = b"Quorum test data";
        let wrong_data = b"Wrong data";
        let cid = create_test_cid(data);
        let sig_wrong_data = key2.sign(wrong_data);

        let proof = QuorumProof {
            data_cid: cid,
            policy_id: "test-policy".to_string(),
            signatures: vec![(key2.did().clone(), sig_wrong_data)],
            metadata: None,
        };
        let mut public_keys = HashMap::new();
        public_keys.insert(key1.did().clone(), *key1.public_key());
        public_keys.insert(key2.did().clone(), *key2.public_key());
        let result = validator.validate_quorum(&proof, &policy, &public_keys, data);
         assert!(matches!(result, Err(QuorumError::SignatureError { .. })));
    }

    #[test]
    fn test_quorum_all_policy() {
        let validator = QuorumValidator::new();
        let key1 = DidKey::new();
        let key2 = DidKey::new();
        let policy = QuorumPolicy::All(vec![key1.did().clone(), key2.did().clone()]);
        let data = b"Data requiring all signatures";
        let cid = create_test_cid(data);
        let sig1 = key1.sign(data);
        let sig2 = key2.sign(data);

        let mut public_keys = HashMap::new();
        public_keys.insert(key1.did().clone(), *key1.public_key());
        public_keys.insert(key2.did().clone(), *key2.public_key());

        // Test success
        let proof_ok = QuorumProof {
            data_cid: cid.clone(),
            policy_id: "all-policy".to_string(),
            signatures: vec![(key1.did().clone(), sig1), (key2.did().clone(), sig2)],
            metadata: None,
        };
        assert!(validator.validate_quorum(&proof_ok, &policy, &public_keys, data).is_ok());

        // Test failure (missing one signature)
        let proof_fail = QuorumProof {
            data_cid: cid.clone(),
            policy_id: "all-policy".to_string(),
            signatures: vec![(key1.did().clone(), sig1)],
            metadata: None,
        };
        let result = validator.validate_quorum(&proof_fail, &policy, &public_keys, data);
         assert!(matches!(result, Err(QuorumError::QuorumNotMet { required: 2, found: 1 })));
    }
} 