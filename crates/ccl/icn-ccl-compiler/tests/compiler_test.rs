use icn_ccl_compiler::compile;

#[test]
fn compile_anchor_data_hash_stable() {
    let src = r#"
        anchor_data {
            cid   = "bafy123"
            bytes = 16
        }
    "#;

    let art = compile(src, "did:coop:alice").expect("compile");

    // ✱ 1. ensure Wasm header is present (0x00 0x61 0x73 0x6D)
    assert!(art.wasm.starts_with(&[0x00, 0x61, 0x73, 0x6d]));

    // ✱ 2. golden hash – to be filled in after first run
    assert_eq!(
        art.hash_hex,
        "80167470ee173847a5b0b6825edba4bc6ac973a5f86dc0a6f05d0cdf184858e7"
    );
} 