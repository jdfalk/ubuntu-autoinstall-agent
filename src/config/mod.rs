// file: src/config/mod.rs
// version: 1.0.0
// guid: a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d

//! Configuration module for Ubuntu AutoInstall Agent
//! 
//! Handles loading and validation of target configurations and image specifications.

pub mod image;
pub mod loader;
pub mod target;

pub use image::{ImageInfo, ImageSpec, VmConfig};
pub use target::{LuksConfig, NetworkConfig, TargetConfig, UserConfig};

use serde::{Deserialize, Serialize};

/// Supported system architectures for Ubuntu deployment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    #[serde(rename = "amd64")]
    Amd64,
    #[serde(rename = "arm64")]
    Arm64,
}

impl Architecture {
    /// Get the architecture as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        }
    }

    /// Get the QEMU architecture string
    pub fn qemu_arch(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "x86_64",
            Architecture::Arm64 => "aarch64",
        }
    }
}

impl std::str::FromStr for Architecture {
    type Err = crate::error::AutoInstallError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "amd64" | "x86_64" => Ok(Architecture::Amd64),
            "arm64" | "aarch64" => Ok(Architecture::Arm64),
            _ => Err(crate::error::AutoInstallError::ValidationError(
                format!("Unknown architecture: {}", s),
            )),
        }
    }
}