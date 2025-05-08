//! icn-cli placeholder

#![allow(missing_docs)] // TODO: Remove this once docs are added

pub mod cli;
pub mod commands; // Assuming this is the main commands module directory
pub mod context;
pub mod error;
pub mod config;
// pub mod metrics; // If needed

// Optional: Re-export key types if desired for convenience
pub use cli::Cli;
pub use cli::Commands;
