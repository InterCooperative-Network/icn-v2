use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CclModule {
    pub stmts: Vec<CclStmt>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CclStmt {
    PerformMeteredAction { resource: String, amount: u64 },
    MintToken            { token:    String, amount: u128 },
    TransferResource     { token:    String, to:     String, amount: u128 },
    AnchorData           { cid:      String, bytes:  u64 },
    // …extend…
}

pub type CclExpr = String; // placeholder for future expression grammar 