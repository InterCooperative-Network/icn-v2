use std::path::PathBuf;
use anyhow::Result;
use dirs; // Make sure dirs is imported

/// Returns ~/.icn by default, respecting $ICN_DATA_DIR override.
pub fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("ICN_DATA_DIR") {
        Ok(PathBuf::from(dir))
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("cannot find home dir"))
            .map(|home| home.join(".icn"))
    }
}

// Corrected logic for the else branch:
pub fn data_dir_corrected() -> Result<PathBuf> {
    if let Ok(dir_str) = std::env::var("ICN_DATA_DIR") {
        Ok(PathBuf::from(dir_str))
    } else {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to find home directory"))?;
        Ok(home.join(".icn"))
    }
}

// To keep the function signature simple and ensure only one data_dir exists:
// I'll use the corrected logic for the main data_dir function.
// The primary change is to ensure Ok() wraps the final PathBuf in the else.

pub fn get_data_dir() -> Result<PathBuf> { // Renaming to avoid conflict if data_dir existed.
    if let Ok(dir_str) = std::env::var("ICN_DATA_DIR") {
        Ok(PathBuf::from(dir_str))
    } else {
        match dirs::home_dir() {
            Some(home) => Ok(home.join(".icn")),
            None => Err(anyhow::anyhow!("Failed to find home directory"))
        }
    }
}

// Final attempt for data_dir to match the user's snippet more closely
// and ensure it's the one used by the handler.

// pub fn data_dir() -> anyhow::Result<PathBuf> { ... } // This is what the handler calls.
// Forcing the correct one by re-declaring based on the snippet to ensure it's this version. 