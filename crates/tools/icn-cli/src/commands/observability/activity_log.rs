use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::dag::{DagPayload, NodeScope, SignedDagNode};
use icn_types::Cid;
use serde_json::{json, Value};
use std::path::Path;
use chrono::{DateTime, Utc};

/// Activity type enum
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityType {
    ProposalSubmitted,
    VoteCast,
    PolicyChanged,
    FederationJoin,
    Other(String),
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityType::ProposalSubmitted => write!(f, "Proposal Submitted"),
            ActivityType::VoteCast => write!(f, "Vote Cast"),
            ActivityType::PolicyChanged => write!(f, "Policy Changed"),
            ActivityType::FederationJoin => write!(f, "Federation Join"),
            ActivityType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Activity event
#[derive(Debug)]
pub struct ActivityEvent {
    /// Type of activity
    pub activity_type: ActivityType,
    /// CID of the node
    pub cid: Cid,
    /// Timestamp of the activity
    pub timestamp: DateTime<Utc>,
    /// Actor (DID) who performed the activity
    pub actor: String,
    /// Description of the activity
    pub description: String,
    /// Additional details as JSON
    pub details: Option<Value>,
}

/// Activity log utility
pub struct ActivityLog {
    dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>,
}

impl ActivityLog {
    /// Create a new activity log
    pub fn new(dag_store: std::sync::Arc<dyn icn_types::dag::DagStore + Send + Sync>) -> Self {
        ActivityLog { dag_store }
    }
    
    /// Get activity events for a scope
    pub async fn get_scope_activities(
        &self,
        scope_type: NodeScope,
        scope_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ActivityEvent>, CliError> {
        let all_nodes = self.dag_store.get_ordered_nodes().await
            .map_err(CliError::Dag)?;
        
        // Filter nodes by scope
        let scope_nodes = all_nodes.into_iter()
            .filter(|node| {
                // Filter by scope type
                node.node.metadata.scope == scope_type 
                // Filter by scope ID if provided
                && match (scope_id, &node.node.metadata.scope_id) {
                    (Some(id), Some(node_id)) => id == node_id,
                    (Some(_), None) => false,
                    (None, _) => true,
                }
            })
            .collect::<Vec<_>>();
        
        // Convert nodes to activity events
        let mut activities = Vec::new();
        
        for node in scope_nodes {
            let cid = if let Some(cid) = &node.cid {
                cid.clone()
            } else {
                node.calculate_cid().map_err(CliError::Dag)?
            };
            
            if let Some(event) = self.node_to_activity_event(&node, &cid)? {
                activities.push(event);
            }
        }
        
        // Sort by timestamp (newest first)
        activities.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        // Apply limit
        let limited_activities = activities.into_iter().take(limit).collect();
        
        Ok(limited_activities)
    }
    
    /// Convert a DAG node to an activity event
    fn node_to_activity_event(
        &self,
        node: &SignedDagNode,
        cid: &Cid,
    ) -> Result<Option<ActivityEvent>, CliError> {
        match &node.node.payload {
            DagPayload::Json(json) => {
                // Try to determine the activity type from the JSON payload
                if let Some(payload_type) = json.get("type").and_then(|v| v.as_str()) {
                    match payload_type {
                        "Proposal" => {
                            let title = json.get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Untitled proposal");
                                
                            let description = format!("Proposal: {}", title);
                            
                            Ok(Some(ActivityEvent {
                                activity_type: ActivityType::ProposalSubmitted,
                                cid: cid.clone(),
                                timestamp: node.node.metadata.timestamp,
                                actor: node.node.author.to_string(),
                                description,
                                details: Some(json.clone()),
                            }))
                        },
                        "Vote" => {
                            let vote_value = json.get("vote")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                                
                            let target_cid = json.get("target_cid")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                                
                            let description = format!("Vote: {} on {}", vote_value, target_cid);
                            
                            Ok(Some(ActivityEvent {
                                activity_type: ActivityType::VoteCast,
                                cid: cid.clone(),
                                timestamp: node.node.metadata.timestamp,
                                actor: node.node.author.to_string(),
                                description,
                                details: Some(json.clone()),
                            }))
                        },
                        "Policy" | "PolicyUpdate" => {
                            Ok(Some(ActivityEvent {
                                activity_type: ActivityType::PolicyChanged,
                                cid: cid.clone(),
                                timestamp: node.node.metadata.timestamp,
                                actor: node.node.author.to_string(),
                                description: "Policy updated".to_string(),
                                details: Some(json.clone()),
                            }))
                        },
                        "FederationJoin" => {
                            let federation_id = json.get("federation_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                                
                            let description = format!("Joined federation: {}", federation_id);
                            
                            Ok(Some(ActivityEvent {
                                activity_type: ActivityType::FederationJoin,
                                cid: cid.clone(),
                                timestamp: node.node.metadata.timestamp,
                                actor: node.node.author.to_string(),
                                description,
                                details: Some(json.clone()),
                            }))
                        },
                        other => {
                            // For unknown types, create a generic activity event
                            Ok(Some(ActivityEvent {
                                activity_type: ActivityType::Other(other.to_string()),
                                cid: cid.clone(),
                                timestamp: node.node.metadata.timestamp,
                                actor: node.node.author.to_string(),
                                description: format!("Activity: {}", other),
                                details: Some(json.clone()),
                            }))
                        }
                    }
                } else {
                    // No type field found in JSON
                    Ok(None)
                }
            },
            // Skip non-JSON payloads
            _ => Ok(None),
        }
    }
    
    /// Render activity log as text
    pub fn render_text(&self, activities: &[ActivityEvent]) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "=".repeat(80)));
        output.push_str(&format!("GOVERNANCE ACTIVITY LOG\n"));
        output.push_str(&format!("{}\n", "=".repeat(80)));
        
        if activities.is_empty() {
            output.push_str("\nNo governance activities found.\n");
            return output;
        }
        
        for (i, activity) in activities.iter().enumerate() {
            output.push_str(&format!("\n{: >3}. [{}] {}\n", 
                i + 1,
                activity.timestamp.format("%Y-%m-%d %H:%M:%S"),
                activity.activity_type.to_string()
            ));
            output.push_str(&format!("     Actor: {}\n", activity.actor));
            output.push_str(&format!("     CID: {}\n", activity.cid));
            output.push_str(&format!("     {}\n", activity.description));
        }
        
        output
    }
    
    /// Render activity log as JSON
    pub fn render_json(&self, activities: &[ActivityEvent]) -> String {
        let activities_json = activities.iter().map(|activity| {
            json!({
                "activity_type": activity.activity_type.to_string(),
                "cid": activity.cid.to_string(),
                "timestamp": activity.timestamp,
                "actor": activity.actor,
                "description": activity.description,
                "details": activity.details
            })
        }).collect::<Vec<_>>();
        
        let response = json!({
            "total_activities": activities.len(),
            "activities": activities_json
        });
        
        serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Error generating JSON".to_string())
    }
}

/// Get activity log for a scope
pub async fn get_activity_log(
    ctx: &mut CliContext,
    scope_type: NodeScope,
    scope_id: Option<&str>,
    dag_dir: Option<&Path>,
    limit: usize,
    output_format: &str,
) -> CliResult<()> {
    let dag_store = ctx.get_dag_store(dag_dir)?;
    
    let activity_log = ActivityLog::new(dag_store);
    let activities = activity_log.get_scope_activities(scope_type, scope_id, limit).await?;
    
    match output_format.to_lowercase().as_str() {
        "json" => {
            println!("{}", activity_log.render_json(&activities));
        },
        _ => {
            println!("{}", activity_log.render_text(&activities));
        }
    }
    
    Ok(())
} 