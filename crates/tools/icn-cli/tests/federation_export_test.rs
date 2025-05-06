use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn test_federation_export_command() -> Result<(), Box<dyn std::error::Error>> {
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
    assert!(federation_dir.join("federation.toml").exists());
    assert!(federation_dir.join("genesis_bundle.json").exists());
    assert!(federation_dir.join("genesis_event.json").exists());
    
    // Add an extra file to include in the export
    let extra_file_path = federation_dir.join("extra_file.txt");
    let mut extra_file = fs::File::create(extra_file_path)?;
    extra_file.write_all(b"Extra file content for testing export")?;
    
    // Create a directory for the export output
    let export_dir = temp_dir.path().join("export");
    fs::create_dir_all(&export_dir)?;
    let export_file = export_dir.join("federation.car");
    
    // Now export the federation to a CAR archive
    let mut export_cmd = Command::cargo_bin("icn-cli")?;
    export_cmd.arg("federation")
        .arg("export")
        .arg("--federation-dir")
        .arg(federation_dir.to_str().unwrap())
        .arg("--output")
        .arg(export_file.to_str().unwrap())
        .arg("--include")
        .arg(federation_dir.join("extra_file.txt").to_str().unwrap());
    
    // Export should succeed
    export_cmd.assert()
        .success()
        .stdout(predicate::str::contains("Federation exported to CAR archive"))
        .stdout(predicate::str::contains("Federation name: test-federation"));
    
    // Verify the CAR file was created
    assert!(export_file.exists());
    
    // Check the file has non-zero size (verify it has content)
    let file_metadata = fs::metadata(&export_file)?;
    assert!(file_metadata.len() > 0, "Exported CAR file is empty");
    
    Ok(())
}

#[test]
fn test_federation_export_with_keys() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let federation_dir = temp_dir.path().join("test-fed-keys");
    fs::create_dir_all(&federation_dir)?;
    
    // First create a federation
    let mut cmd = Command::cargo_bin("icn-cli")?;
    cmd.arg("federation")
        .arg("init")
        .arg("--name")
        .arg("test-federation-keys")
        .arg("--output-dir")
        .arg(federation_dir.to_str().unwrap());
    
    // Run the init command
    cmd.assert().success();
    
    // Create a directory for the export output
    let export_dir = temp_dir.path().join("export-keys");
    fs::create_dir_all(&export_dir)?;
    let export_file = export_dir.join("federation-with-keys.car");
    
    // Now export the federation to a CAR archive including keys
    let mut export_cmd = Command::cargo_bin("icn-cli")?;
    export_cmd.arg("federation")
        .arg("export")
        .arg("--federation-dir")
        .arg(federation_dir.to_str().unwrap())
        .arg("--output")
        .arg(export_file.to_str().unwrap())
        .arg("--include-keys");
    
    // Export should succeed
    export_cmd.assert()
        .success()
        .stdout(predicate::str::contains("Including federation keys in export"));
    
    // Verify the CAR file was created
    assert!(export_file.exists());
    
    Ok(())
}

#[test]
fn test_federation_export_nonexistent_directory() -> Result<(), Box<dyn std::error::Error>> {
    // Try to export from a non-existent directory
    let mut export_cmd = Command::cargo_bin("icn-cli")?;
    export_cmd.arg("federation")
        .arg("export")
        .arg("--federation-dir")
        .arg("nonexistent-directory");
    
    // Export should fail
    export_cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Federation directory not found"));
    
    Ok(())
} 