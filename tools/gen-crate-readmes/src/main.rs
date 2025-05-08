use anyhow::{Context as AnyhowContext, Result};
use cargo_metadata::MetadataCommand;
use std::fs;
use tera::{Context as TeraContext, Tera};

fn main() -> Result<()> {
    // 1. Initialize Tera and load the template
    let mut tera = Tera::default();
    let template_content = fs::read_to_string("docs/templates/README_TEMPLATE.md")
        .context("Failed to read README template file")?;
    tera.add_raw_template("crate_readme", &template_content)
        .context("Failed to add README template to Tera")?;

    // 2. Get workspace metadata
    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to execute cargo_metadata")?;
    
    let workspace_root = metadata.workspace_root.as_std_path();

    println!("Processing crates...");

    // 3. Iterate over workspace packages
    for package in metadata.workspace_packages() {
        let manifest_path = package.manifest_path.as_std_path();

        // Filter for crates within the `crates/` directory
        if !manifest_path.starts_with(workspace_root.join("crates")) {
            // Also skip if it's a direct member of tools/ but not this tool itself
            if manifest_path.starts_with(workspace_root.join("tools")) && package.name != "gen-crate-readmes" {
                 println!("Skipping non-library/app crate (in tools/ but not gen-crate-readmes): {}", package.name);
                continue;
            }
            if !manifest_path.starts_with(workspace_root.join("tools").join("gen-crate-readmes")) { // don't skip self
                 println!("Skipping non-library/app crate: {}", package.name);
                 continue;
            }
        }
        
        // Skip the gen-crate-readmes crate itself if it's being processed and is not in `crates/` 
        // (though the above filter should handle it if it's in `tools/`)
        if package.name == "gen-crate-readmes" && !manifest_path.starts_with(workspace_root.join("crates")){
            println!("Skipping self (gen-crate-readmes) as it's a tool.");
            continue;
        }

        println!("Generating README for: {}", package.name);

        // 4. Prepare data for the template
        let mut context = TeraContext::new();
        context.insert("crate_name", &package.name);
        context.insert(
            "description",
            &package
                .description
                .clone()
                .unwrap_or_else(|| "No description provided.".to_string()),
        );

        let features_list: Vec<String> = package.features.keys().map(|f| format!("- `{}`", f)).collect();
        if features_list.is_empty() {
            context.insert("features", "No specific features listed.");
        } else {
            context.insert("features", &features_list.join("\n"));
        }

        // 5. Render the template
        let rendered_readme = tera
            .render("crate_readme", &context)
            .with_context(|| format!("Failed to render README for crate: {}", package.name))?;

        // 6. Write the README.md file
        let crate_dir = manifest_path
            .parent()
            .with_context(|| format!("Failed to get parent directory for manifest: {}", manifest_path.display()))?;
        let readme_path = crate_dir.join("README.md");

        fs::write(&readme_path, rendered_readme)
            .with_context(|| format!("Failed to write README.md to: {}", readme_path.display()))?;
        
        println!("Successfully generated README for {}", package.name);
    }

    println!("Finished generating crate READMEs.");
    Ok(())
}
