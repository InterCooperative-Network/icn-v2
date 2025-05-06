use crate::dag::DagNode;
use crate::identity::Did;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DAGSyncBundle {
    pub nodes: Vec<DagNode>,
    // TODO: Add other fields if necessary based on compilation errors
    pub federation_id: String, 
    pub source_peer: Option<String>, // Assuming peer ID is a string
    pub timestamp: Option<DateTime<Utc>>,
} 