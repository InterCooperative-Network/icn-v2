use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use icn_cli::commands::federation::export;
use icn_cli::commands::federation::import;
use icn_cli::commands::federation::bootstrap;
use icn_cli::context::CliContext;

// Helper for test cleanup
fn cleanup_dir(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

#[tokio::test]
async fn test_federation_import_export_roundtrip() {
    // Create temporary directories for the test
    let federation_dir = tempdir().unwrap();
    let export_file = tempdir().unwrap().path().join("test_federation.car");
    let import_dir = tempdir().unwrap().path().join("imported");
    
    // Ensure clean slate
    cleanup_dir(federation_dir.path());
    cleanup_dir(&export_file);
    cleanup_dir(&import_dir);
    
    // Setup the context
    let mut context = CliContext::default();
    
    // Step 1: Bootstrap a test federation
    let federation_name = "test_federation";
    let bootstrap_result = bootstrap::run_init(
        &mut context,
        federation_name,
        Some(federation_dir.path().to_str().unwrap()),
        false, // Not dry run
        &[], // No participants, will generate a federation key
        "all", // Quorum type
        true, // Export keys
        "jwk", // Key format
    ).await;
    
    assert!(bootstrap_result.is_ok(), "Failed to bootstrap federation: {:?}", bootstrap_result);
    
    // Step 2: Export the federation to a CAR archive
    let export_result = export::run_export(
        &context,
        federation_dir.path().to_str().unwrap(),
        export_file.to_str(),
        true, // Include keys
        &[], // No additional files
    ).await;
    
    assert!(export_result.is_ok(), "Failed to export federation: {:?}", export_result);
    assert!(export_file.exists(), "Export file was not created");
    
    // Step 3: Import the federation from the CAR archive
    let import_result = import::run_import(
        &context,
        export_file.to_str().unwrap(),
        Some(import_dir.to_str().unwrap()),
        false, // Not verify only
        false, // Not override
        false, // Don't skip keys
    ).await;
    
    assert!(import_result.is_ok(), "Failed to import federation: {:?}", import_result);
    
    // Step 4: Verify that the imported federation has the correct files
    let original_files = collect_files(federation_dir.path());
    let imported_files = collect_files(&import_dir);
    
    // Check that essential files exist in the imported directory
    assert!(imported_files.contains(&PathBuf::from("federation.toml")), 
        "Imported federation is missing federation.toml");
    assert!(imported_files.contains(&PathBuf::from("genesis_bundle.json")), 
        "Imported federation is missing genesis_bundle.json");
    assert!(imported_files.contains(&PathBuf::from("genesis_event.json")), 
        "Imported federation is missing genesis_event.json");
    assert!(imported_files.contains(&PathBuf::from("federation_keys.json")), 
        "Imported federation is missing federation_keys.json");
    
    // Test importing with --no-keys option
    let import_no_keys_dir = tempdir().unwrap().path().join("imported_no_keys");
    
    let import_no_keys_result = import::run_import(
        &context,
        export_file.to_str().unwrap(),
        Some(import_no_keys_dir.to_str().unwrap()),
        false, // Not verify only
        false, // Not override
        true,  // Skip keys
    ).await;
    
    assert!(import_no_keys_result.is_ok(), "Failed to import federation with --no-keys: {:?}", import_no_keys_result);
    
    let imported_no_keys_files = collect_files(&import_no_keys_dir);
    assert!(imported_no_keys_files.contains(&PathBuf::from("federation.toml")), 
        "Imported federation is missing federation.toml");
    assert!(!imported_no_keys_files.contains(&PathBuf::from("federation_keys.json")), 
        "Imported federation should not have federation_keys.json with --no-keys");
    
    // Test verify-only option
    let verify_only_result = import::run_import(
        &context,
        export_file.to_str().unwrap(),
        None,
        true, // Verify only
        false, // Not override
        false, // Don't skip keys
    ).await;
    
    assert!(verify_only_result.is_ok(), "Failed to verify federation: {:?}", verify_only_result);
    
    // Cleanup
    cleanup_dir(federation_dir.path());
    cleanup_dir(&export_file);
    cleanup_dir(&import_dir);
    cleanup_dir(&import_no_keys_dir);
}

// Helper to collect files in a directory recursively
fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    if !dir.exists() {
        return files;
    }
    
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        
        if path.is_dir() {
            let mut subdir_files = collect_files(&path);
            files.append(&mut subdir_files);
        } else {
            // Convert to relative path from directory
            let rel_path = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
            files.push(rel_path);
        }
    }
    
    files
} 