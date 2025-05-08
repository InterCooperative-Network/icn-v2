use std::fs;
use toml::Value;
use cargo_metadata::MetadataCommand;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = MetadataCommand::new().exec()?;
    let workspace_root = metadata.workspace_root.as_std_path();

    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(&cargo_toml_path)
        .map_err(|e| format!("Failed to read workspace Cargo.toml at {}: {}", cargo_toml_path.display(), e))?;
    
    let parsed_workspace_toml: Value = cargo_toml_content.parse()
        .map_err(|e| format!("Failed to parse workspace Cargo.toml: {}", e))?;

    let members = parsed_workspace_toml["workspace"]["members"]
        .as_array()
        .ok_or("Failed to find workspace.members in workspace Cargo.toml")?
        .iter()
        .map(|v| v.as_str().ok_or("Found non-string member in workspace.members"))
        .collect::<Result<Vec<_>, _>>()?;

    let mut rows = vec![
        "# Crate Map".to_string(),
        "| Crate | Path | Type | Description |".to_string(),
        "|-------|------|------|-------------|".to_string(),
    ];

    for member_path_str in members {
        let crate_manifest_path = workspace_root.join(member_path_str).join("Cargo.toml");
        
        let cargo_content = match fs::read_to_string(&crate_manifest_path) {
            Ok(c) => c,
            Err(_) => {
                // eprintln!("Skipping member {}: Cargo.toml not found at {}", member_path_str, crate_manifest_path.display());
                continue; // Skip if no Cargo.toml for the member (e.g. could be a glob for non-existent paths)
            }
        };

        let parsed_crate_toml: Value = match cargo_content.parse() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Skipping member {}: Failed to parse Cargo.toml at {}: {}", member_path_str, crate_manifest_path.display(), e);
                continue;
            }
        };

        let package = parsed_crate_toml.get("package").ok_or_else(|| format!("Missing [package] table in {}", crate_manifest_path.display()))?;
        let name = package.get("name").and_then(|n| n.as_str()).unwrap_or("???");
        let desc = package.get("description").and_then(|d| d.as_str()).unwrap_or("—");

        let crate_src_path = workspace_root.join(member_path_str).join("src");
        let typ = if parsed_crate_toml.get("lib").is_some() || crate_src_path.join("lib.rs").exists() {
            "lib"
        } else if parsed_crate_toml.get("bin").is_some() || crate_src_path.join("main.rs").exists() {
            "bin"
        } else {
            "?"
        };
        
        let relative_member_path = member_path_str; // Path from workspace members array

        rows.push(format!("| `{}` | `{}` | `{}` | {} |", name, relative_member_path, typ, desc));
    }

    let output = rows.join("\n");
    
    let out_dir = workspace_root.join("docs").join("generated");
    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create docs/generated directory at {}: {}", out_dir.display(), e))?;
    
    let out_path = out_dir.join("crate_map.md");
    fs::write(&out_path, output)
        .map_err(|e| format!("Failed to write crate_map.md to {}: {}", out_path.display(), e))?;
    
    // Output to stdout is handled by the script redirect now
    // println!("✅ Wrote crate map to {}", out_path.display());
    Ok(())
} 