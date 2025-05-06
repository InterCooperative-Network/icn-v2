use crate::identity::Did;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] // Added PartialEq for consistency
pub struct QuorumConfig {
    pub authorized_signers: Vec<Did>,
    pub threshold: usize,
} 