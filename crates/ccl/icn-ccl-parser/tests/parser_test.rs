use icn_ccl_parser::parse_ccl;

#[test]
fn parse_minimal_metered_action() {
    let source = r#"
        perform_metered_action {
            resource_type = "compute_fuel"
            amount = 42
        }
    "#;

    let module = parse_ccl(source).expect("parse failed");

    assert_eq!(module.stmts.len(), 1);
    if let Some(stmt) = module.stmts.get(0) {
        match stmt {
            icn_ccl_parser::CclStmt::PerformMeteredAction { resource, amount } => {
                assert_eq!(resource, "compute_fuel");
                assert_eq!(*amount, 42);
            }
            other => panic!("Unexpected statement: {:?}", other),
        }
    }
}

#[test]
fn parse_mint_token() {
    let src = r#"
        mint_token {
            token  = "coop_credit"
            amount = 1000
        }
    "#;

    let module = parse_ccl(src).unwrap();
    match &module.stmts[0] {
        icn_ccl_parser::CclStmt::MintToken { token, amount } => {
            assert_eq!(token, "coop_credit");
            assert_eq!(*amount, 1_000);
        }
        _ => panic!("wrong stmt"),
    }
}

#[test]
fn parse_transfer_resource() {
    let src = r#"
        transfer_resource {
            token  = "mesh_gpu_hours"
            to     = "did:coop:alice"
            amount = 10
        }
    "#;

    let module = parse_ccl(src).unwrap();
    match &module.stmts[0] {
        icn_ccl_parser::CclStmt::TransferResource { token, to, amount } => {
            assert_eq!(token, "mesh_gpu_hours");
            assert_eq!(to, "did:coop:alice");
            assert_eq!(*amount, 10);
        }
        _ => panic!("wrong stmt"),
    }
}

#[test]
fn parse_anchor_data() {
    let src = r#"
        anchor_data {
            cid   = "bafybeigdyrzt..."
            bytes = 2048
        }
    "#;

    let module = parse_ccl(src).unwrap();
    match &module.stmts[0] {
        icn_ccl_parser::CclStmt::AnchorData { cid, bytes } => {
            assert_eq!(cid, "bafybeigdyrzt...");
            assert_eq!(*bytes, 2048);
        }
        _ => panic!("wrong stmt"),
    }
} 