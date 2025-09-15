// file: src/cli/mod.rs
// version: 1.0.0
// guid: e5f6g7h8-i9j0-1234-5678-901234efghij

//! Command line interface for Ubuntu AutoInstall Agent

pub mod args;
pub mod commands;

pub use args::Cli;
pub use commands::*;
