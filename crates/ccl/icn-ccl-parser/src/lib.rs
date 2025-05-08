#![deny(unsafe_code)]

pub mod ast;
pub use ast::{CclModule, CclStmt};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

#[derive(Parser)]
#[grammar = "../grammar/ccl.pest"]
pub struct CclParser;

pub fn parse_ccl(source: &str) -> Result<CclModule, CclError> {
    let mut pairs = CclParser::parse(Rule::file, source)
        .map_err(|e| CclError::Syntax(e.to_string()))?;

    let file_pair = pairs.next().expect("INTERNAL_ERROR: file rule should always be present");
    if file_pair.as_rule() != Rule::file {
        return Err(CclError::Syntax(format!("INTERNAL_ERROR: Expected Rule::file, got {:?}", file_pair.as_rule())));
    }

    let mut stmts = Vec::new();
    for pair_in_file in file_pair.into_inner() {
        match pair_in_file.as_rule() {
            Rule::stmt => {
                let actual_stmt_pair = pair_in_file.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: Empty stmt rule encountered".to_string()))?;
                
                match actual_stmt_pair.as_rule() {
                    Rule::perform_metered_action => {
                        stmts.push(parse_metered_action(actual_stmt_pair)?);
                    }
                    Rule::mint_token => {
                        stmts.push(parse_mint_token(actual_stmt_pair)?);
                    }
                    Rule::transfer_resource => {
                        stmts.push(parse_transfer_resource(actual_stmt_pair)?);
                    }
                    Rule::anchor_data => {
                        stmts.push(parse_anchor_data(actual_stmt_pair)?);
                    }
                    _ => {
                        return Err(CclError::Syntax(format!(
                            "Unexpected rule {:?} inside stmt. Expected a specific statement type.",
                            actual_stmt_pair.as_rule()
                        )));
                    }
                }
            }
            Rule::EOI => {
                break;
            }
            _ => {
                return Err(CclError::Syntax(format!(
                    "Unexpected rule {:?} directly inside file. Expected stmt or EOI.",
                    pair_in_file.as_rule()
                )));
            }
        }
    }

    Ok(CclModule { stmts })
}

fn parse_metered_action(pair: Pair<Rule>) -> Result<CclStmt, CclError> {
    let mut resource = None;
    let mut amount   = None;

    for inner_kv_pair in pair.into_inner() {
        match inner_kv_pair.as_rule() {
            Rule::kv_resource_type => {
                let string_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_resource_type missing string_lit".to_string()))?;
                resource = Some(unquote(string_lit_pair.as_str()));
            }
            Rule::kv_amount => {
                let int_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_amount missing int_lit".to_string()))?;
                let lit_str = int_lit_pair.as_str();
                amount = Some(lit_str.parse::<u64>().map_err(|_| {
                    CclError::Semantic(format!("Invalid amount '{}': not a valid u64 integer.", lit_str))
                })?);
            }
            _ => {
                 return Err(CclError::Syntax(format!(
                    "Unexpected rule {:?} inside perform_metered_action. Expected kv_resource_type or kv_amount.",
                    inner_kv_pair.as_rule()
                )));
            }
        }
    }

    Ok(CclStmt::PerformMeteredAction {
        resource: resource.ok_or_else(|| CclError::Semantic("Missing 'resource_type' in perform_metered_action.".into()))?,
        amount:   amount  .ok_or_else(|| CclError::Semantic("Missing 'amount' in perform_metered_action.".into()))?,
    })
}

fn parse_mint_token(pair: Pair<Rule>) -> Result<CclStmt, CclError> {
    let mut token  = None;
    let mut amount = None;

    for inner_kv_pair in pair.into_inner() {
        match inner_kv_pair.as_rule() {
            Rule::kv_token => {
                let string_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_token missing string_lit".to_string()))?;
                token = Some(unquote(string_lit_pair.as_str()));
            }
            Rule::kv_amount => {
                let int_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_amount missing int_lit".to_string()))?;
                let lit_str = int_lit_pair.as_str();
                amount = Some(lit_str.parse::<u128>().map_err(|_| {
                    CclError::Semantic(format!("Invalid amount '{}': not a valid u128 integer.", lit_str))
                })?);
            }
            _ => {
                return Err(CclError::Syntax(format!(
                   "Unexpected rule {:?} inside mint_token. Expected kv_token or kv_amount.",
                   inner_kv_pair.as_rule()
               )));
           }
        }
    }

    Ok(CclStmt::MintToken {
        token:  token .ok_or_else(|| CclError::Semantic("Missing 'token' in mint_token.".into()))?,
        amount: amount.ok_or_else(|| CclError::Semantic("Missing 'amount' in mint_token.".into()))?,
    })
}

fn parse_transfer_resource(pair: Pair<Rule>) -> Result<CclStmt, CclError> {
    let mut token  = None;
    let mut to     = None;
    let mut amount = None;

    for inner_kv_pair in pair.into_inner() {
        match inner_kv_pair.as_rule() {
            Rule::kv_token => {
                let string_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_token missing string_lit".to_string()))?;
                token = Some(unquote(string_lit_pair.as_str()));
            }
            Rule::kv_to => {
                let string_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_to missing string_lit".to_string()))?;
                to = Some(unquote(string_lit_pair.as_str()));
            }
            Rule::kv_amount => {
                let int_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_amount missing int_lit".to_string()))?;
                let lit_str = int_lit_pair.as_str();
                amount = Some(lit_str.parse::<u128>().map_err(|_| {
                    CclError::Semantic(format!("Invalid amount '{}': not a valid u128 integer.", lit_str))
                })?);
            }
            _ => {
                return Err(CclError::Syntax(format!(
                   "Unexpected rule {:?} inside transfer_resource. Expected kv_token, kv_to, or kv_amount.",
                   inner_kv_pair.as_rule()
               )));
           }
        }
    }

    Ok(CclStmt::TransferResource {
        token:  token .ok_or_else(|| CclError::Semantic("Missing 'token' in transfer_resource.".into()))?,
        to:     to    .ok_or_else(|| CclError::Semantic("Missing 'to' in transfer_resource.".into()))?,
        amount: amount.ok_or_else(|| CclError::Semantic("Missing 'amount' in transfer_resource.".into()))?,
    })
}

fn parse_anchor_data(pair: Pair<Rule>) -> Result<CclStmt, CclError> {
    let mut cid   = None;
    let mut bytes = None;

    for inner_kv_pair in pair.into_inner() {
        match inner_kv_pair.as_rule() {
            Rule::kv_cid => {
                let string_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_cid missing string_lit".to_string()))?;
                cid = Some(unquote(string_lit_pair.as_str()));
            }
            Rule::kv_bytes => {
                let int_lit_pair = inner_kv_pair.into_inner().next()
                    .ok_or_else(|| CclError::Syntax("INTERNAL_ERROR: kv_bytes missing int_lit".to_string()))?;
                let lit_str = int_lit_pair.as_str();
                bytes = Some(lit_str.parse::<u64>().map_err(|_| {
                    CclError::Semantic(format!("Invalid bytes '{}': not a valid u64 integer.", lit_str))
                })?);
            }
            _ => {
                return Err(CclError::Syntax(format!(
                   "Unexpected rule {:?} inside anchor_data. Expected kv_cid or kv_bytes.",
                   inner_kv_pair.as_rule()
               )));
           }
        }
    }

    Ok(CclStmt::AnchorData {
        cid:   cid  .ok_or_else(|| CclError::Semantic("Missing 'cid' in anchor_data.".into()))?,
        bytes: bytes.ok_or_else(|| CclError::Semantic("Missing 'bytes' in anchor_data.".into()))?,
    })
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[derive(Error, Debug)]
pub enum CclError {
    #[error("syntax error: {0}")]
    Syntax(String),
    #[error("semantic error: {0}")]
    Semantic(String),
} 