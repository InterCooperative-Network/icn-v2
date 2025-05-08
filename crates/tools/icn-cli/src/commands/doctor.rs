use std::path::Path;
use std::process::Command;

// A helper to print section titles
fn print_section_title(title: &str) {
    // Using a bit more flair for section titles
    println!("\n🩺 === {} === 🩺", title.to_uppercase());
}

// A helper to print check results
fn print_check_result(check_name: &str, success: bool, message: String) {
    let status_emoji = if success { "✅" } else { "❌" };
    let status_text = if success { "OK" } else { "FAILED" };
    // Aligning the status part for better readability
    println!("  [{:<7}] {}: {}", format!("{} {}", status_emoji, status_text), check_name, message);
}

fn check_rust_toolchain() -> Result<(), String> {
    print_section_title("Rust Toolchain Verification");
    match Command::new("rustc").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                print_check_result("Rust Compiler (rustc)", true, version_str);
                Ok(())
            } else {
                let err_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
                print_check_result("Rust Compiler (rustc)", false, format!("Error output: {}", err_str));
                Err(format!("Failed to get rustc version. Details: {}", err_str))
            }
        }
        Err(e) => {
            let msg = format!("'rustc' command not found or could not execute. Is Rust installed and in PATH? Details: {}", e);
            print_check_result("Rust Compiler (rustc)", false, msg.clone());
            Err(msg)
        }
    }
}

fn check_wasm_target() -> Result<(), String> {
    print_section_title("WASM Target Verification");
    match Command::new("rustup").args(["target", "list", "--installed"]).output() {
        Ok(output) => {
            if output.status.success() {
                let installed_targets = String::from_utf8_lossy(&output.stdout);
                if installed_targets.contains("wasm32-unknown-unknown") {
                    print_check_result("wasm32-unknown-unknown target", true, "Correctly installed".to_string());
                    Ok(())
                } else {
                    let msg = "Not found among installed Rust targets. Please install it via: rustup target add wasm32-unknown-unknown".to_string();
                    print_check_result("wasm32-unknown-unknown target", false, msg.clone());
                    Err(msg)
                }
            } else {
                let err_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
                print_check_result("wasm32-unknown-unknown target", false, format!("Error checking targets: {}", err_str));
                Err(format!("'rustup target list' command failed. Details: {}", err_str))
            }
        }
        Err(e) => {
            let msg = format!("'rustup' command not found or failed. Is rustup installed correctly? Details: {}", e);
            print_check_result("wasm32-unknown-unknown target", false, msg.clone());
            Err(msg)
        }
    }
}

fn check_env_file() -> Result<(), String> {
    print_section_title("Environment File (.env) Check");
    let env_path = Path::new(".env");
    if env_path.exists() {
        print_check_result(".env file check", true, format!("Found at: {}", env_path.display()));
        // TODO: Add checks for specific essential variables if needed
        // Example: check_specific_env_var("ICN_NODE_KEY_PATH");
        Ok(())
    } else {
        print_check_result(".env file check", true, "Optional .env file not found in current directory. This might be normal for some setups.".to_string());
        // Returning Ok as absence is not necessarily a failure for basic doctor check
        Ok(())
    }
}

fn check_dag_config() -> Result<(), String> {
    print_section_title("DAG Configuration Check");
    let common_paths = ["dag_config.toml", "config/dag.toml", ".icn/dag_config.toml", "data/dag_config.toml"];
    let mut found_path: Option<String> = None;
    for path_str in &common_paths {
        let path = Path::new(path_str);
        if path.exists() {
            found_path = Some(format!("Found at: {}", path.display()));
            break;
        }
    }
    if let Some(msg) = found_path {
        print_check_result("DAG Config File", true, msg);
    } else {
        print_check_result("DAG Config File", true, "No common DAG config file found. Ensure configuration is loaded via arguments or default paths if this is unexpected.".to_string());
    }
    // TODO: Add more specific checks, e.g., parse the config, check key fields
    Ok(())
}

pub async fn run_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧑‍⚕️  ICN Doctor: Running system diagnostics...");

    let mut all_checks_passed = true;

    // Group checks and update all_checks_passed
    let checks = vec![
        check_rust_toolchain,
        check_wasm_target,
        check_env_file, 
        check_dag_config,
    ];

    for check_fn in checks {
        if let Err(_e) = check_fn() {
            // Individual check functions now print their own detailed error messages with context
            // We mark that at least one check had an issue.
            all_checks_passed = false;
        }
    }

    println!("\n✨ --- DIAGNOSTICS COMPLETE --- ✨");
    if all_checks_passed {
        println!("✅  All checks passed successfully. Your ICN environment looks good to go!");
    } else {
        println!("❌  Some checks reported issues. Please review the output above for details and suggestions.");
        // Potentially exit with a non-zero status code to indicate failure for scripting
        // std::process::exit(1);
    }

    Ok(())
} 