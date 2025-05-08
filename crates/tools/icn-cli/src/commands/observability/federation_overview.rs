use crate::context::CliContext;
use crate::error::{CliError, CliResult};
use icn_types::dag::{DagPayload, NodeScope, SignedDagNode};
use icn_types::Cid;
use serde_json::{json, Value};
use std::path::Path;
use std::collections::{HashMap, HashSet};
use crate::context::MutableDagStore;

/// Member information
#[derive(Debug)]
pub struct MemberInfo {
    /// ID of the member (cooperative or community)
    pub id: String,
    /// Type of the member (cooperative or community)
    pub member_type: String,
    /// Name or description of the member (if available)
    pub name: Option<String>,
    /// Latest DAG head CID for this member
    pub latest_head: Option<Cid>,
    /// Timestamp of the latest head
    pub latest_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Federation overview information
#[derive(Debug)]
pub struct FederationOverview {
    /// Federation ID
    pub federation_id: String,
    /// Federation description (if available)
    pub description: Option<String>,
    /// Member cooperatives
    pub cooperatives: Vec<MemberInfo>,
    /// Member communities
    pub communities: Vec<MemberInfo>,
    /// Federation DAG head
    pub federation_head: Option<Cid>,
}

/// Federation overview utility
pub struct FederationInspector {
    dag_store: MutableDagStore,
}

impl FederationInspector {
    /// Create a new federation inspector
    pub fn new(dag_store: MutableDagStore) -> Self {
        Self { dag_store }
    }
    
    /// Get federation overview
    pub async fn get_federation_overview(
        &self,
        federation_id: &str,
    ) -> Result<FederationOverview, CliError> {
        let all_nodes = self.dag_store.get_ordered_nodes().await
            .map_err(CliError::Dag)?;
        
        let mut overview = FederationOverview {
            federation_id: federation_id.to_string(),
            description: None,
            cooperatives: Vec::new(),
            communities: Vec::new(),
            federation_head: None,
        };
        
        // Collect cooperative and community members
        let mut coop_members = HashMap::new();
        let mut community_members = HashMap::new();
        
        // Find federation nodes to get the head and description
        let mut federation_nodes = Vec::new();
        
        for node in &all_nodes {
            // Get node CID
            let cid = if let Some(cid) = &node.cid {
                cid.clone()
            } else {
                node.calculate_cid().map_err(CliError::Dag)?
            };
            
            // Check federation nodes
            if node.node.metadata.scope == NodeScope::Federation && 
               node.node.metadata.federation_id == federation_id {
                federation_nodes.push((cid.clone(), node.clone()));
                
                // Extract federation description if available
                if let DagPayload::Json(json) = &node.node.payload {
                    if json.get("type").and_then(|v| v.as_str()) == Some("FederationCreate") {
                        if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                            overview.description = Some(desc.to_string());
                        }
                    }
                }
            }
            
            // Check cooperative members
            if node.node.metadata.scope == NodeScope::Cooperative && 
               node.node.metadata.federation_id == federation_id {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    // Update or insert cooperative member
                    let entry = coop_members.entry(scope_id.clone()).or_insert_with(|| MemberInfo {
                        id: scope_id.clone(),
                        member_type: "Cooperative".to_string(),
                        name: None,
                        latest_head: None,
                        latest_timestamp: None,
                    });
                    
                    // Extract name if available
                    if let DagPayload::Json(json) = &node.node.payload {
                        if json.get("type").and_then(|v| v.as_str()) == Some("CooperativeCreate") {
                            if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                                entry.name = Some(name.to_string());
                            }
                        }
                    }
                    
                    // Update latest head if newer
                    if entry.latest_timestamp.is_none() || 
                       entry.latest_timestamp.as_ref().unwrap() < &node.node.metadata.timestamp {
                        entry.latest_head = Some(cid.clone());
                        entry.latest_timestamp = Some(node.node.metadata.timestamp);
                    }
                }
            }
            
            // Check community members
            if node.node.metadata.scope == NodeScope::Community && 
               node.node.metadata.federation_id == federation_id {
                if let Some(scope_id) = &node.node.metadata.scope_id {
                    // Update or insert community member
                    let entry = community_members.entry(scope_id.clone()).or_insert_with(|| MemberInfo {
                        id: scope_id.clone(),
                        member_type: "Community".to_string(),
                        name: None,
                        latest_head: None,
                        latest_timestamp: None,
                    });
                    
                    // Extract name if available
                    if let DagPayload::Json(json) = &node.node.payload {
                        if json.get("type").and_then(|v| v.as_str()) == Some("CommunityCreate") {
                            if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                                entry.name = Some(name.to_string());
                            }
                        }
                    }
                    
                    // Update latest head if newer
                    if entry.latest_timestamp.is_none() || 
                       entry.latest_timestamp.as_ref().unwrap() < &node.node.metadata.timestamp {
                        entry.latest_head = Some(cid.clone());
                        entry.latest_timestamp = Some(node.node.metadata.timestamp);
                    }
                }
            }
        }
        
        // Find federation head (latest node)
        if !federation_nodes.is_empty() {
            // Sort by timestamp (newest first)
            federation_nodes.sort_by(|(_, a), (_, b)| b.node.metadata.timestamp.cmp(&a.node.metadata.timestamp));
            overview.federation_head = Some(federation_nodes[0].0.clone());
        }
        
        // Convert members to vectors
        overview.cooperatives = coop_members.into_values().collect();
        overview.communities = community_members.into_values().collect();
        
        Ok(overview)
    }
    
    /// Render federation overview as text
    pub fn render_text(&self, overview: &FederationOverview) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "=".repeat(80)));
        output.push_str(&format!("FEDERATION OVERVIEW: {}\n", overview.federation_id));
        output.push_str(&format!("{}\n", "=".repeat(80)));
        
        if let Some(desc) = &overview.description {
            output.push_str(&format!("Description: {}\n", desc));
        }
        
        if let Some(head) = &overview.federation_head {
            output.push_str(&format!("Federation DAG Head: {}\n", head));
        }
        
        // Member cooperatives
        output.push_str(&format!("\nCooperative Members: {}\n", overview.cooperatives.len()));
        output.push_str(&format!("{}\n", "-".repeat(80)));
        
        if overview.cooperatives.is_empty() {
            output.push_str("  No cooperative members found.\n");
        } else {
            for (i, coop) in overview.cooperatives.iter().enumerate() {
                output.push_str(&format!("{: >3}. {}", i + 1, coop.id));
                
                if let Some(name) = &coop.name {
                    output.push_str(&format!(" - {}", name));
                }
                
                output.push_str("\n");
                
                if let Some(head) = &coop.latest_head {
                    output.push_str(&format!("     Latest DAG Head: {}\n", head));
                }
                
                if let Some(ts) = &coop.latest_timestamp {
                    output.push_str(&format!("     Last Activity: {}\n", ts));
                }
                
                output.push_str("\n");
            }
        }
        
        // Member communities
        output.push_str(&format!("\nCommunity Members: {}\n", overview.communities.len()));
        output.push_str(&format!("{}\n", "-".repeat(80)));
        
        if overview.communities.is_empty() {
            output.push_str("  No community members found.\n");
        } else {
            for (i, community) in overview.communities.iter().enumerate() {
                output.push_str(&format!("{: >3}. {}", i + 1, community.id));
                
                if let Some(name) = &community.name {
                    output.push_str(&format!(" - {}", name));
                }
                
                output.push_str("\n");
                
                if let Some(head) = &community.latest_head {
                    output.push_str(&format!("     Latest DAG Head: {}\n", head));
                }
                
                if let Some(ts) = &community.latest_timestamp {
                    output.push_str(&format!("     Last Activity: {}\n", ts));
                }
                
                output.push_str("\n");
            }
        }
        
        output
    }
    
    /// Render federation overview as JSON
    pub fn render_json(&self, overview: &FederationOverview) -> String {
        let cooperatives = overview.cooperatives.iter().map(|coop| {
            json!({
                "id": coop.id,
                "type": coop.member_type,
                "name": coop.name,
                "latest_head": coop.latest_head.as_ref().map(|c| c.to_string()),
                "latest_timestamp": coop.latest_timestamp
            })
        }).collect::<Vec<_>>();
        
        let communities = overview.communities.iter().map(|community| {
            json!({
                "id": community.id,
                "type": community.member_type,
                "name": community.name,
                "latest_head": community.latest_head.as_ref().map(|c| c.to_string()),
                "latest_timestamp": community.latest_timestamp
            })
        }).collect::<Vec<_>>();
        
        let response = json!({
            "federation": {
                "id": overview.federation_id,
                "description": overview.description,
                "head": overview.federation_head.as_ref().map(|c| c.to_string())
            },
            "members": {
                "cooperatives": {
                    "count": overview.cooperatives.len(),
                    "items": cooperatives
                },
                "communities": {
                    "count": overview.communities.len(),
                    "items": communities
                }
            }
        });
        
        serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Error generating JSON".to_string())
    }
}

/// Get federation overview
pub async fn get_federation_overview(
    ctx: &mut CliContext,
    federation_id: &str,
    dag_dir: Option<&Path>,
    output_format: &str,
) -> CliResult<()> {
    let dag_store = ctx.get_dag_store(dag_dir)?;
    
    let federation_inspector = FederationInspector::new(dag_store);
    let overview = federation_inspector.get_federation_overview(federation_id).await?;
    
    match output_format.to_lowercase().as_str() {
        "json" => {
            println!("{}", federation_inspector.render_json(&overview));
        },
        _ => {
            println!("{}", federation_inspector.render_text(&overview));
        }
    }
    
    Ok(())
} 