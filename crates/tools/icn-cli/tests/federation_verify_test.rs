use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;
use serde_json::json;

#[test]
fn test_federation_verify_command() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let federation_dir = temp_dir.path().join("test-federation");
    fs::create_dir_all(&federation_dir)?;
    
    // First create a federation
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("test-federation")
        .arg("--output-dir")
        .arg(federation_dir.to_str().unwrap());
    
    // Run the init command
    cmd.assert().success();
    
    // Verify the federation was created with all required files
    let bundle_path = federation_dir.join("genesis_bundle.json");
    assert!(bundle_path.exists());
    
    // Now verify the bundle
    let mut verify_cmd = Command::cargo_bin("icn-cli")?;
    verify_cmd.arg("federation")
        .arg("verify")
        .arg("--bundle-path")
        .arg(bundle_path.to_str().unwrap());
    
    // Verify should succeed
    verify_cmd.assert()
        .success()
        .stdout(predicate::str::contains("Verification result: ✅ VALID"));
    
    Ok(())
}

#[test]
fn test_federation_verify_with_invalid_bundle() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let federation_dir = temp_dir.path().join("invalid-federation");
    fs::create_dir_all(&federation_dir)?;
    
    // Create an invalid bundle (malformed JSON)
    let bundle_path = federation_dir.join("invalid_bundle.json");
    let mut file = fs::File::create(&bundle_path)?;
    file.write_all(b"{ this is not valid json }")?;
    
    // Try to verify the invalid bundle
    let mut verify_cmd = Command::cargo_bin("icn-cli")?;
    verify_cmd.arg("federation")
        .arg("verify")
        .arg("--bundle-path")
        .arg(bundle_path.to_str().unwrap());
    
    // Verify should fail
    verify_cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Serialization error"));
    
    Ok(())
}

#[test]
fn test_federation_verify_verbose() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let federation_dir = temp_dir.path().join("test-federation-verbose");
    fs::create_dir_all(&federation_dir)?;
    
    // First create a federation
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("test-federation")
        .arg("--output-dir")
        .arg(federation_dir.to_str().unwrap());
    
    // Run the init command
    cmd.assert().success();
    
    // Verify the federation was created with all required files
    let bundle_path = federation_dir.join("genesis_bundle.json");
    assert!(bundle_path.exists());
    
    // Now verify the bundle with verbose output
    let mut verify_cmd = Command::cargo_bin("icn-cli")?;
    verify_cmd.arg("federation")
        .arg("verify")
        .arg("--bundle-path")
        .arg(bundle_path.to_str().unwrap())
        .arg("--verbose");
    
    // Verify should succeed with detailed output
    verify_cmd.assert()
        .success()
        .stdout(predicate::str::contains("Valid signers"))
        .stdout(predicate::str::contains("Loaded TrustBundle"));
    
    Ok(())
} 