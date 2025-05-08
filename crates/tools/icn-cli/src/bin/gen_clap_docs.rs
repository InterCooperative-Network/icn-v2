use clap::CommandFactory;
use clap_complete::{generate_to, shells::Markdown};
use icn_cli::Cli; // Assuming Cli is exposed from the library root or icn_cli::cli::Cli
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir_str = env::var("OUT_DIR").unwrap_or_else(|_| "docs/generated".into());
    let out_dir = PathBuf::from(out_dir_str);

    fs::create_dir_all(&out_dir)?;

    let mut cmd = Cli::command();
    let bin_name = "icn"; // Or use cmd.get_name().to_string() if preferred
    let path = generate_to(Markdown, &mut cmd, bin_name, &out_dir)?;

    println!("✅ CLI reference written to: {}", path.display());
    Ok(())
} 