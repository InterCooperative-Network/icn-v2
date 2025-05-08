use std::{fs, path::PathBuf};
use toml::Value;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let parsed: Value = cargo_toml.parse().expect("parse toml");

    let members = parsed["workspace"]["members"]
        .as_array()
        .expect("workspace.members")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();

    let mut rows = vec![
        "| Crate | Path | Type | Description |".to_string(),
        "|-------|------|------|-------------|".to_string(),
    ];

    for member in members {
        let path = root.join(member);
        let toml_path = path.join("Cargo.toml");
        let cargo = match fs::read_to_string(&toml_path) {
            Ok(c) => c,
            Err(_) => continue, // skip if no Cargo.toml
        };

        let parsed: Value = match cargo.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let package = &parsed["package"];
        let name = package["name"].as_str().unwrap_or("???");
        let desc = package["description"].as_str().unwrap_or("—");

        let typ = if parsed.get("lib").is_some() {
            "lib"
        } else if parsed.get("bin").is_some() || path.join("src/main.rs").exists() {
            "bin"
        } else {
            "?"
        };

        rows.push(format!("| `{}` | `{}` | `{}` | {} |", name, member, typ, desc));
    }

    let output = rows.join("\n");
    // Ensure docs/generated directory exists
    let out_dir = root.join("docs/generated");
    fs::create_dir_all(&out_dir).expect("create docs/generated directory");
    let out_path = out_dir.join("crate_map.md");
    fs::write(&out_path, output).expect("write crate_map.md");
    println!("✅ Wrote crate map to {}", out_path.display());
} 