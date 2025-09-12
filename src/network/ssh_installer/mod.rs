// file: src/network/ssh_installer/mod.rs
// version: 1.0.0
// guid: sshmod01-2345-6789-abcd-ef0123456789

//! SSH-based Ubuntu installation with ZFS and LUKS
//!
//! This module provides a comprehensive SSH-based installation system
//! for Ubuntu with ZFS and LUKS encryption.

pub mod config;
pub mod investigation;
pub mod packages;
pub mod disk_ops;
pub mod zfs_ops;
pub mod system_setup;
pub mod installer;

pub use config::{InstallationConfig, SystemInfo};
pub use installer::SshInstaller;
