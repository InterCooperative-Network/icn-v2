use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::dag::{DagPayload, SignedDagNode};
use icn_types::Cid;
use serde_json::{json, Value};
use std::path::Path;
use std::collections::HashSet;

/// Quorum information
#[derive(Debug)]
pub struct QuorumInfo {
    /// The node containing the quorum proof
    pub node: SignedDagNode,
    /// CID of the node
    pub cid: Cid,
    /// Required signers as specified in the quorum proof
    pub required_signers: Vec<String>,
    /// Actual signers found in the proof
    pub actual_signers: Vec<SignerInfo>,
    /// Whether the quorum is valid
    pub is_valid: bool,
    /// Error message if quorum is invalid
    pub error_message: Option<String>,
}

/// Signer information
#[derive(Debug)]
pub struct SignerInfo {
    /// DID of the signer
    pub did: String,
    /// Role of the signer (if available)
    pub role: Option<String>,
    /// Scope of the signer (if available)
    pub scope: Option<String>,
}

/// Quorum validator utility
pub struct QuorumValidator {
    dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>,
}

impl QuorumValidator {
    /// Create a new quorum validator
    pub fn new(dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>) -> Self {
        QuorumValidator { dag_store }
    }
    
    /// Validate quorum proof for a node
    pub async fn validate_quorum(
        &self,
        cid: &Cid,
    ) -> Result<QuorumInfo, CliError> {
        let node = self.dag_store.get_node(cid).await
            .map_err(|e| CliError::Dag(e))?;
        
        let quorum_proof = match &node.node.payload {
            DagPayload::Json(json) => {
                if let Some(proof) = json.get("quorum_proof") {
                    proof.clone()
                } else {
                    return Err(CliError::ValidationError("Node does not contain a quorum proof".to_string()));
                }
            },
            _ => return Err(CliError::ValidationError("Node payload is not JSON".to_string())),
        };
        
        // Extract required signers
        let required_signers = if let Some(required) = quorum_proof.get("required_signers") {
            if let Some(signers) = required.as_array() {
                signers.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        
        // Extract actual signers
        let actual_signers = if let Some(actual) = quorum_proof.get("signers") {
            if let Some(signers) = actual.as_array() {
                signers.iter()
                    .filter_map(|v| {
                        if let Some(obj) = v.as_object() {
                            let did = obj.get("did")?.as_str()?.to_string();
                            let role = obj.get("role").and_then(|r| r.as_str()).map(|s| s.to_string());
                            let scope = obj.get("scope").and_then(|s| s.as_str()).map(|s| s.to_string());
                            
                            Some(SignerInfo { did, role, scope })
                        } else if let Some(did) = v.as_str() {
                            Some(SignerInfo { 
                                did: did.to_string(), 
                                role: None, 
                                scope: None 
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        
        // Check if the quorum is valid
        let required_set: HashSet<String> = required_signers.iter().cloned().collect();
        let actual_set: HashSet<String> = actual_signers.iter().map(|s| s.did.clone()).collect();
        
        let is_valid = required_set.is_subset(&actual_set);
        let error_message = if !is_valid {
            let missing: Vec<String> = required_set.difference(&actual_set).cloned().collect();
            Some(format!("Missing required signers: {}", missing.join(", ")))
        } else {
            None
        };
        
        Ok(QuorumInfo {
            node,
            cid: cid.clone(),
            required_signers,
            actual_signers,
            is_valid,
            error_message,
        })
    }
    
    /// Render quorum validation as text
    pub fn render_text(&self, quorum_info: &QuorumInfo, show_signers: bool) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "=".repeat(80)));
        output.push_str(&format!("QUORUM VALIDATION\n"));
        output.push_str(&format!("{}\n", "=".repeat(80)));
        output.push_str(&format!("Node CID: {}\n", quorum_info.cid));
        output.push_str(&format!("Timestamp: {}\n", quorum_info.node.node.metadata.timestamp));
        output.push_str(&format!("Author: {}\n\n", quorum_info.node.node.author));
        
        // Quorum status
        output.push_str(&format!("Quorum Status: {}\n", 
            if quorum_info.is_valid { 
                "VALID ✅" 
            } else { 
                "INVALID ❌" 
            }
        ));
        
        if let Some(error) = &quorum_info.error_message {
            output.push_str(&format!("Error: {}\n", error));
        }
        
        // Required signers
        output.push_str(&format!("\nRequired Signers: {}\n", quorum_info.required_signers.len()));
        for (i, signer) in quorum_info.required_signers.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, signer));
        }
        
        // Actual signers
        output.push_str(&format!("\nActual Signers: {}\n", quorum_info.actual_signers.len()));
        if show_signers {
            for (i, signer) in quorum_info.actual_signers.iter().enumerate() {
                output.push_str(&format!("  {}. {}", i + 1, signer.did));
                
                let mut details = Vec::new();
                if let Some(role) = &signer.role {
                    details.push(format!("role: {}", role));
                }
                if let Some(scope) = &signer.scope {
                    details.push(format!("scope: {}", scope));
                }
                
                if !details.is_empty() {
                    output.push_str(&format!(" ({})", details.join(", ")));
                }
                
                output.push('\n');
            }
        }
        
        output
    }
    
    /// Render quorum validation as JSON
    pub fn render_json(&self, quorum_info: &QuorumInfo, show_signers: bool) -> String {
        let signers = if show_signers {
            quorum_info.actual_signers.iter().map(|signer| {
                json!({
                    "did": signer.did,
                    "role": signer.role,
                    "scope": signer.scope
                })
            }).collect()
        } else {
            Vec::new()
        };
        
        let response = json!({
            "node": {
                "cid": quorum_info.cid.to_string(),
                "timestamp": quorum_info.node.node.metadata.timestamp,
                "author": quorum_info.node.node.author.to_string()
            },
            "quorum": {
                "is_valid": quorum_info.is_valid,
                "error": quorum_info.error_message,
                "required_signers_count": quorum_info.required_signers.len(),
                "actual_signers_count": quorum_info.actual_signers.len(),
                "required_signers": quorum_info.required_signers,
                "actual_signers": if show_signers { signers } else { json!([]) }
            }
        });
        
        serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Error generating JSON".to_string())
    }
}

/// Validate quorum proof for a node
pub async fn validate_quorum(
    ctx: &mut CliContext,
    cid_str: &str,
    show_signers: bool,
    dag_dir: Option<&Path>,
    output_format: &str,
) -> CliResult<()> {
    let dag_store = ctx.get_dag_store(dag_dir)?;
    
    // Parse CID
    let external_cid_parsed: cid::CidGeneric<64> = cid_str.parse()
        .map_err(|e: cid::Error| {
            CliError::InvalidCidFormat(format!("Invalid CID string '{}': {}", cid_str, e))
        })?;
    
    let cid = icn_types::Cid::from_bytes(&external_cid_parsed.to_bytes())
        .map_err(|e| {
            CliError::InvalidCidFormat(format!("Failed to convert CID to internal format: {}", e))
        })?;
    
    let quorum_validator = QuorumValidator::new(dag_store);
    let quorum_info = quorum_validator.validate_quorum(&cid).await?;
    
    match output_format.to_lowercase().as_str() {
        "json" => {
            println!("{}", quorum_validator.render_json(&quorum_info, show_signers));
        },
        _ => {
            println!("{}", quorum_validator.render_text(&quorum_info, show_signers));
        }
    }
    
    Ok(())
} 