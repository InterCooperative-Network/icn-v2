use icn_types::{Cid, Did};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::Value;
use ed25519_dalek::Signature; // Import Signature
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD as BASE64_ENGINE;

// Basic structure mirroring W3C VC Data Model concepts
// Needs refinement with proper context, proof types etc.

/// Represents a Verifiable Credential.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VerifiableCredential {
    #[serde(rename = "@context")]
    pub context: Vec<String>, // e.g., ["https://www.w3.org/2018/credentials/v1"]
    pub id: Option<String>, // URI identifying the credential
    #[serde(rename = "type")]
    pub type_: Vec<String>, // e.g., ["VerifiableCredential", "ExecutionProof"]
    pub issuer: Did, // Issuer's DID
    #[serde(rename = "issuanceDate")]
    pub issuance_date: DateTime<Utc>,
    #[serde(rename = "credentialSubject")]
    pub credential_subject: Value, // The actual claims
    // Proof needs a dedicated structure (e.g., Ed25519Signature2020)
    pub proof: Option<Proof>, // Use the defined Proof struct
}

/// Represents the cryptographic proof attached to a VC.
/// Based on Ed25519Signature2020 (simplified).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Proof {
    #[serde(rename = "type")]
    pub type_: String, // e.g., "Ed25519Signature2020"
    #[serde(rename = "created")]
    pub created: DateTime<Utc>,
    #[serde(rename = "verificationMethod")]
    pub verification_method: String, // Should be the issuer's DID URL
    #[serde(rename = "proofPurpose")]
    pub proof_purpose: String, // e.g., "assertionMethod"
    // For Ed25519Signature2020, this is typically base64url encoded
    #[serde(rename = "proofValue")]
    pub proof_value: String, // Base64 encoded signature
}

/// Placeholder for VC Issuance logic.
pub struct VcIssuer {
    // Potentially holds the issuer's DidKey
}

impl VcIssuer {
    // Method to issue a new VC (simplified)
    pub fn issue(
        &self,
        issuer_did_key: &super::did::DidKey, // Need issuer's key to sign
        context: Vec<String>,
        type_: Vec<String>,
        credential_subject: Value,
    ) -> Result<VerifiableCredential, String> {
        let now = Utc::now();
        let issuer_did = issuer_did_key.did().clone();

        // Create the VC structure *without* the proof first
        let mut vc_to_sign = VerifiableCredential {
            context,
            id: None, // Could generate a UUID URN
            type_,
            issuer: issuer_did.clone(),
            issuance_date: now,
            credential_subject,
            proof: None,
        };

        // Serialize the VC (excluding proof) to canonical JSON or similar for signing
        // NOTE: Proper canonicalization (e.g., JCS) is crucial for interoperability!
        // Using pretty JSON here is **INSECURE** for real applications.
        let signing_input_bytes = serde_json::to_vec_pretty(&vc_to_sign)
            .map_err(|e| format!("Failed to serialize VC for signing: {}", e))?;

        // Sign the canonical form
        let signature = issuer_did_key.sign(&signing_input_bytes);

        // Create the proof
        let proof = Proof {
            type_: "Ed25519Signature2020".to_string(),
            created: now,
            verification_method: issuer_did_key.to_did_string(),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: BASE64_ENGINE.encode(signature.to_bytes()), // Use Engine API
        };

        // Add the proof to the VC
        vc_to_sign.proof = Some(proof);

        Ok(vc_to_sign)
    }
}

// Placeholder for Verifier logic
pub struct VcVerifier;

impl VcVerifier {
    pub fn verify(
        vc: &VerifiableCredential,
        // Need a way to resolve the issuer DID to a PublicKey
    ) -> Result<(), String> {
        let proof = vc.proof.as_ref().ok_or("Credential has no proof")?;

        // 1. Resolve proof.verification_method (DID) to PublicKey
        //    let public_key = DidKey::public_key_from_did(&proof.verification_method)?;
        let public_key = super::did::DidKey::public_key_from_did(&proof.verification_method)
            .map_err(|e| format!("Failed to get public key from DID: {}", e))?;

        // 2. Decode the signature from proof.proof_value (base64)
        let signature_bytes = BASE64_ENGINE.decode(&proof.proof_value)
             .map_err(|e| format!("Failed to decode proof signature: {}", e))?;
        let signature = Signature::from_bytes(&signature_bytes)
            .map_err(|e| format!("Invalid signature format: {}", e))?;

        // 3. Create the signing input by removing the proof and canonicalizing
        let mut vc_data_to_verify = vc.clone();
        vc_data_to_verify.proof = None;
        let verification_input_bytes = serde_json::to_vec_pretty(&vc_data_to_verify)
            .map_err(|e| format!("Failed to serialize VC for verification: {}", e))?;

        // 4. Verify the signature
        public_key.verify(&verification_input_bytes, &signature)
            .map_err(|e| format!("Signature verification failed: {}", e))?; // Use ? directly

        Ok(())
    }
}