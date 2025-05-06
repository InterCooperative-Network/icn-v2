use crate::cid::Cid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A reference to an anchored object within the ICN DAG system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnchorRef {
    /// The Content ID of the anchored data.
    pub cid: Cid,
    /// Optional type hint for the anchored data (e.g., "TrustBundle", "ExecutionReceipt").
    pub object_type: Option<String>,
    /// The timestamp associated with this anchor.
    pub timestamp: DateTime<Utc>,
} 