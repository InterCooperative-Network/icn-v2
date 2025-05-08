use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;
use serde_json::json;

#[test]
fn test_federation_init_command() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let output_dir = temp_dir.path().join("test-fed");

    // Run the command with a test federation name
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("test-federation")
        .arg("--output-dir")
        .arg(output_dir.to_str().unwrap());

    // Verify command runs successfully
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Federation `test-federation` initialized successfully"));

    // Verify expected files exist
    assert!(output_dir.join("federation.toml").exists());
    assert!(output_dir.join("genesis_bundle.json").exists());
    assert!(output_dir.join("genesis_event.json").exists());
    assert!(output_dir.join("federation_keys.json").exists());

    // Verify content of federation.toml
    let toml_content = fs::read_to_string(output_dir.join("federation.toml"))?;
    assert!(toml_content.contains("name = \"test-federation\""));
    assert!(toml_content.contains("did = \"did:key:"));

    // Verify content of genesis bundle
    let bundle_content = fs::read_to_string(output_dir.join("genesis_bundle.json"))?;
    assert!(bundle_content.contains("\"federation_id\": \"test-federation\""));
    assert!(bundle_content.contains("\"genesis\": \"true\""));

    // Verify content of genesis event
    let event_content = fs::read_to_string(output_dir.join("genesis_event.json"))?;
    assert!(event_content.contains("\"event_type\": \"Genesis\""));
    assert!(event_content.contains("\"kind\": \"Genesis\""));
    assert!(event_content.contains("\"federation_id\": \"test-federation\""));

    // Verify exported keys
    let keys_content = fs::read_to_string(output_dir.join("federation_keys.json"))?;
    assert!(keys_content.contains("\"did\":"));
    assert!(keys_content.contains("\"private_key\":"));
    assert!(keys_content.contains("\"public_key\":"));

    Ok(())
}

#[test]
fn test_federation_init_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let output_dir = temp_dir.path().join("test-fed-dry");

    // Run the command with dry-run flag
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("test-federation")
        .arg("--output-dir")
        .arg(output_dir.to_str().unwrap())
        .arg("--dry-run");

    // Verify command runs successfully with dry run message
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("🧪 DRY RUN: Federation initialized (no files written)"));

    // Verify no files were created
    assert!(!Path::new(&output_dir).exists() || fs::read_dir(&output_dir)?.next().is_none());

    Ok(())
}

#[test]
fn test_federation_init_with_multiple_participants() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let keys_dir = temp_dir.path().join("keys");
    let output_dir = temp_dir.path().join("multi-fed");
    
    fs::create_dir_all(&keys_dir)?;
    
    // Create 3 participant key files
    let key_files = [
        create_test_key_file(&keys_dir, "alice", "jwk")?,
        create_test_key_file(&keys_dir, "bob", "jwk")?,
        create_test_key_file(&keys_dir, "charlie", "jwk")?,
    ];
    
    // Run the command with multiple participants
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("multi-federation")
        .arg("--output-dir")
        .arg(output_dir.to_str().unwrap())
        .arg("--quorum")
        .arg("threshold:67") // 2 of 3 required
        .arg("--participant")
        .arg(&key_files[0])
        .arg("--participant")
        .arg(&key_files[1])
        .arg("--participant")
        .arg(&key_files[2]);
    
    // Verify command runs successfully
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Federation `multi-federation` initialized successfully"))
        .stdout(predicate::str::contains("Configured quorum: 2 of 3 participants required"));
    
    // Verify federation.toml contains multiple guardians
    let toml_content = fs::read_to_string(output_dir.join("federation.toml"))?;
    assert!(toml_content.contains("guardians"));
    assert!(toml_content.contains("quorum_config"));
    assert!(toml_content.contains("threshold:67"));
    
    // Verify bundle has multiple participants
    let bundle_content = fs::read_to_string(output_dir.join("genesis_bundle.json"))?;
    assert!(bundle_content.contains("\"quorum_type\": {"));
    assert!(bundle_content.contains("\"type\": \"Threshold\""));
    assert!(bundle_content.contains("\"value\": 67"));
    assert!(bundle_content.contains("\"participants\": ["));
    
    // Verify keys file contains all participants
    let keys_content = fs::read_to_string(output_dir.join("federation_keys.json"))?;
    assert_eq!(keys_content.matches("\"did\":").count(), 4); // 3 participants + 1 federation DID
    
    Ok(())
}

// Helper function to create test key files
fn create_test_key_file(dir: &Path, name: &str, format: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = dir.join(format!("{}_key.json", name));
    
    // Create a basic key structure (this would normally be generated or loaded properly)
    let key_json = match format {
        "jwk" => json!({
            "did": format!("did:key:test{}", name),
            "format": "jwk",
            "private_key": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": format!("test-pubkey-for-{}", name),
                "d": format!("test-privkey-for-{}", name)
            },
            "public_key": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": format!("test-pubkey-for-{}", name)
            },
            "metadata": {}
        }),
        "base58" => json!({
            "did": format!("did:key:test{}", name),
            "format": "base58",
            "private_key": format!("test-privkey-for-{}", name),
            "public_key": format!("test-pubkey-for-{}", name),
            "metadata": {}
        }),
        _ => return Err("Unsupported format".into()),
    };
    
    let mut file = fs::File::create(&file_path)?;
    file.write_all(serde_json::to_string_pretty(&key_json)?.as_bytes())?;
    
    Ok(file_path.to_string_lossy().to_string())
} 