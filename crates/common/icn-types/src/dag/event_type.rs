use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Genesis,
    Proposal,
    Vote,
    Execution,
    Receipt,
    Custom(String),
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::Genesis => write!(f, "genesis"),
            EventType::Proposal => write!(f, "proposal"),
            EventType::Vote => write!(f, "vote"),
            EventType::Execution => write!(f, "execution"),
            EventType::Receipt => write!(f, "receipt"),
            EventType::Custom(s) => write!(f, "custom:{}", s),
        }
    }
} 