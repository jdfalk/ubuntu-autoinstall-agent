// file: src/commands/mod.rs
// version: 2.0.0
// guid: b3c95817-32f1-4e1c-8b34-78f6e85029dc

//! Command module for the Copilot Agent Utility
//!
//! This module provides command execution functionality for various tools and operations.

pub mod awk;
pub mod buf;
pub mod editor;
pub mod file;
pub mod git;
pub mod linter;
pub mod prettier;
pub mod python;
pub mod sed;
pub mod system;
pub mod uutils;

use crate::executor::Executor;
use anyhow::Result;
use clap::ArgMatches;
use std::future::Future;

/// Trait for command execution
pub trait CommandExecutor {
    /// Execute the command with the given arguments and executor
    fn execute(
        matches: &ArgMatches,
        executor: &Executor,
    ) -> impl Future<Output = Result<()>> + Send;
}
