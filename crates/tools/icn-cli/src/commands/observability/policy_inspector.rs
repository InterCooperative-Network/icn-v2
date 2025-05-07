use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use std::collections::HashMap;
use icn_types::dag::{DagPayload, NodeScope, SignedDagNode};
use icn_types::Cid;
use serde_json::{json, Value};
use std::path::Path;
use crate::context::MutableDagStore;

/// Policy information structure
#[derive(Debug)]
pub struct PolicyInfo {
    /// Policy content as JSON
    pub content: Value,
    /// CID of the policy node
    pub cid: Cid,
    /// Last update timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Update trail - sequence of policy updates with voter information
    pub update_trail: Vec<PolicyUpdateInfo>,
}

/// Policy update information
#[derive(Debug)]
pub struct PolicyUpdateInfo {
    /// CID of the policy update node
    pub cid: Cid,
    /// Timestamp of the update
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Proposer DID
    pub proposer: String,
    /// Votes cast for this update
    pub votes: Vec<VoteInfo>,
}

/// Vote information
#[derive(Debug)]
pub struct VoteInfo {
    /// CID of the vote node
    pub cid: Cid,
    /// Voter DID
    pub voter: String,
    /// Vote decision (approve/reject)
    pub decision: String,
    /// Optional reason
    pub reason: Option<String>,
}

/// Policy inspector utility
pub struct PolicyInspector {
    dag_store: MutableDagStore,
}

impl PolicyInspector {
    /// Create a new policy inspector
    pub fn new(dag_store: MutableDagStore) -> Self {
        Self { dag_store }
    }
    
    /// Get the current active policy for a scope
    pub async fn get_active_policy(
        &self,
        scope_type: NodeScope,
        scope_id: Option<&str>,
    ) -> Result<Option<PolicyInfo>, CliError> {
        let all_nodes = self.dag_store.get_ordered_nodes().await
            .map_err(CliError::Dag)?;
        
        // Find policy nodes for the scope
        let mut policy_nodes = all_nodes.into_iter()
            .filter(|node| {
                // Filter by scope type
                node.node.metadata.scope == scope_type 
                // Filter by scope ID if provided
                && match (scope_id, &node.node.metadata.scope_id) {
                    (Some(id), Some(node_id)) => id == node_id,
                    (Some(_), None) => false,
                    (None, _) => true,
                }
                // Filter by payload type
                && match &node.node.payload {
                    DagPayload::Json(json) => {
                        json.get("type").and_then(|v| v.as_str()) == Some("PolicyUpdate") ||
                        json.get("type").and_then(|v| v.as_str()) == Some("Policy")
                    },
                    _ => false,
                }
            })
            .collect::<Vec<_>>();
        
        // Sort by timestamp to get the most recent policy
        policy_nodes.sort_by(|a, b| b.node.metadata.timestamp.cmp(&a.node.metadata.timestamp));
        
        if policy_nodes.is_empty() {
            return Ok(None);
        }
        
        // Get the most recent policy
        let latest_policy = policy_nodes.first().unwrap();
        let policy_cid = if let Some(cid) = &latest_policy.cid {
            cid.clone()
        } else {
            latest_policy.calculate_cid().map_err(CliError::Dag)?
        };
        
        // Extract policy content
        let policy_content = match &latest_policy.node.payload {
            DagPayload::Json(json) => json.clone(),
            _ => return Err(CliError::SerializationError("Policy node has invalid payload type".to_string())),
        };
        
        // Build update trail
        let update_trail = self.build_policy_update_trail(&policy_nodes).await?;
        
        Ok(Some(PolicyInfo {
            content: policy_content,
            cid: policy_cid,
            timestamp: latest_policy.node.metadata.timestamp,
            update_trail,
        }))
    }
    
    /// Build policy update trail
    async fn build_policy_update_trail(
        &self,
        policy_nodes: &[SignedDagNode],
    ) -> Result<Vec<PolicyUpdateInfo>, CliError> {
        let mut update_trail = Vec::new();
        
        for node in policy_nodes {
            if let DagPayload::Json(json) = &node.node.payload {
                if json.get("type").and_then(|v| v.as_str()) == Some("PolicyUpdate") {
                    let cid = if let Some(cid) = &node.cid {
                        cid.clone()
                    } else {
                        node.calculate_cid().map_err(CliError::Dag)?
                    };
                    
                    // Collect votes for this policy update
                    let votes = self.collect_votes_for_policy_update(&cid).await?;
                    
                    update_trail.push(PolicyUpdateInfo {
                        cid,
                        timestamp: node.node.metadata.timestamp,
                        proposer: node.node.author.to_string(),
                        votes,
                    });
                }
            }
        }
        
        Ok(update_trail)
    }
    
    /// Collect votes for a policy update
    async fn collect_votes_for_policy_update(
        &self,
        policy_update_cid: &Cid,
    ) -> Result<Vec<VoteInfo>, CliError> {
        let all_nodes = self.dag_store.get_ordered_nodes().await
            .map_err(CliError::Dag)?;
        
        let votes = all_nodes.into_iter()
            .filter(|node| {
                if let DagPayload::Json(json) = &node.node.payload {
                    if json.get("type").and_then(|v| v.as_str()) == Some("Vote") {
                        if let Some(target_cid) = json.get("target_cid").and_then(|v| v.as_str()) {
                            return target_cid == policy_update_cid.to_string();
                        }
                    }
                }
                false
            })
            .map(|node| {
                let cid = if let Some(cid) = &node.cid {
                    cid.clone()
                } else {
                    node.calculate_cid().map_err(CliError::Dag)?
                };
                
                if let DagPayload::Json(json) = &node.node.payload {
                    let decision = json.get("vote")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    let reason = json.get("reason")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    Ok(VoteInfo {
                        cid,
                        voter: node.node.author.to_string(),
                        decision,
                        reason,
                    })
                } else {
                    Err(CliError::SerializationError("Vote node has invalid payload type".to_string()))
                }
            })
            .collect::<Result<Vec<_>, CliError>>()?;
        
        Ok(votes)
    }
    
    /// Render policy as text
    pub fn render_text(&self, policy_info: &PolicyInfo) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "=".repeat(80)));
        output.push_str(&format!("ACTIVE POLICY\n"));
        output.push_str(&format!("{}\n", "=".repeat(80)));
        output.push_str(&format!("Policy CID: {}\n", policy_info.cid));
        output.push_str(&format!("Last Updated: {}\n\n", policy_info.timestamp));
        
        // Format policy content
        output.push_str("Policy Content:\n");
        output.push_str(&format!("{}\n\n", serde_json::to_string_pretty(&policy_info.content).unwrap_or_else(|_| "Error formatting policy content".to_string())));
        
        // Format update trail
        if !policy_info.update_trail.is_empty() {
            output.push_str("Policy Update Trail:\n");
            output.push_str(&format!("{}\n", "-".repeat(80)));
            
            for (i, update) in policy_info.update_trail.iter().enumerate() {
                output.push_str(&format!("Update #{}: CID {}\n", i + 1, update.cid));
                output.push_str(&format!("Proposed by: {} at {}\n", update.proposer, update.timestamp));
                
                if !update.votes.is_empty() {
                    output.push_str("Votes:\n");
                    for vote in &update.votes {
                        output.push_str(&format!("  - {}: {} {}\n", 
                            vote.voter, 
                            vote.decision,
                            vote.reason.as_ref().map(|r| format!("({})", r)).unwrap_or_default()
                        ));
                    }
                } else {
                    output.push_str("No votes recorded for this update.\n");
                }
                
                output.push_str(&format!("{}\n", "-".repeat(80)));
            }
        } else {
            output.push_str("No policy update trail available.\n");
        }
        
        output
    }
    
    /// Render policy as JSON
    pub fn render_json(&self, policy_info: &PolicyInfo) -> String {
        let update_trail = policy_info.update_trail.iter().map(|update| {
            json!({
                "cid": update.cid.to_string(),
                "timestamp": update.timestamp,
                "proposer": update.proposer,
                "votes": update.votes.iter().map(|vote| {
                    json!({
                        "cid": vote.cid.to_string(),
                        "voter": vote.voter,
                        "decision": vote.decision,
                        "reason": vote.reason
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>();
        
        let response = json!({
            "policy": {
                "cid": policy_info.cid.to_string(),
                "timestamp": policy_info.timestamp,
                "content": policy_info.content
            },
            "update_trail": update_trail
        });
        
        serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Error generating JSON".to_string())
    }
}

/// Inspect policy for a scope
pub async fn inspect_policy(
    ctx: &mut CliContext,
    scope_type: NodeScope,
    scope_id: Option<&str>,
    dag_dir: Option<&Path>,
    output_format: &str,
) -> CliResult<()> {
    let dag_store = ctx.get_dag_store(dag_dir)?;
    
    let policy_inspector = PolicyInspector::new(dag_store);
    let policy_info = policy_inspector.get_active_policy(scope_type, scope_id).await?;
    
    if let Some(policy) = policy_info {
        match output_format.to_lowercase().as_str() {
            "json" => {
                println!("{}", policy_inspector.render_json(&policy));
            },
            _ => {
                println!("{}", policy_inspector.render_text(&policy));
            }
        }
    } else {
        println!("No policy found for the specified scope.");
    }
    
    Ok(())
} 