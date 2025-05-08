use clap::Parser;
use std::path::PathBuf;
use anyhow::Result;
use crate::cli::Cli;

/// Command for generating CLI documentation.
#[derive(Parser, Debug, Clone)]
pub struct GenCliDocsCmd {
    /// Directory to output the generated Markdown files.
    #[clap(short, long, default_value = "./docs/cli")]
    pub output_dir: String,
}

pub fn generate_cli_docs<T: clap::CommandFactory>(cmd: &GenCliDocsCmd) -> Result<()> {
    use clap_markdown::help_markdown;
    use std::{fs::{File, create_dir_all}, io::Write};

    let output_path = PathBuf::from(&cmd.output_dir);
    if !output_path.exists() {
        create_dir_all(&output_path)?;
    }
    
    let markdown = help_markdown::<Cli>();
    let file_path = output_path.join("icn.md");
    let mut file = File::create(&file_path)?;
    write!(file, "{}", markdown)?;
    println!("Generated CLI docs at: {}", file_path.display());
    Ok(())
} 