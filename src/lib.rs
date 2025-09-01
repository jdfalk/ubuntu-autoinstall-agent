// file: src/lib.rs
// version: 1.0.0
// guid: d82472d1-7f0f-4eb4-b0a3-6e1547103eb4

//! # Ubuntu AutoInstall Agent
//!
//! Automated Ubuntu server deployment with golden images and LUKS encryption.
//! This system provides zero manual intervention deployment using VM-based golden
//! images that can be deployed via SSH or netboot.

pub mod cli;
pub mod config;
pub mod error;
pub mod image;
pub mod logging;
pub mod network;
pub mod security;
pub mod utils;

pub use error::{AutoInstallError, Result};
