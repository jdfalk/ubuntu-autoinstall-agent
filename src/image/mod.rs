// file: src/image/mod.rs
// version: 1.0.0
// guid: a1b2c3d4-e5f6-7890-1234-567890abcdef

//! Image management module for Ubuntu autoinstall agent
//!
//! This module handles:
//! - Golden image creation and management
//! - Image downloading and verification
//! - Machine-specific customization
//! - Image deployment to LUKS volumes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

pub mod builder;
pub mod deployer;
pub mod customizer;
pub mod manager;

// Re-export main components for easier access
pub use builder::ImageBuilder;
pub use customizer::{ImageCustomizer, CustomizationTemplate};
pub use deployer::ImageDeployer;
pub use manager::{ImageManager, ImageManagerConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Architecture {
    Amd64,
    Arm64,
}

impl Architecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        }
    }

    pub fn qemu_arch(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "x86_64",
            Architecture::Arm64 => "aarch64",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub name: String,
    pub version: String,
    pub architecture: Architecture,
    pub ubuntu_version: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetMachine {
    pub hostname: String,
    pub architecture: Architecture,
    pub disk_device: String,
    pub network_config: NetworkConfig,
    pub luks_config: LuksConfig,
    pub ssh_keys: Vec<String>,
    pub timezone: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interface: String,
    pub address: String,
    pub gateway: String,
    pub dns_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuksConfig {
    pub passphrase: String,
    pub cipher: Option<String>,
    pub key_size: Option<u32>,
    pub hash: Option<String>,
}

pub trait ImageManager {
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn get_image(&self, name: &str, arch: Architecture) -> Result<Option<ImageInfo>>;
    async fn download_image(&self, image: &ImageInfo) -> Result<PathBuf>;
    async fn verify_image(&self, image: &ImageInfo, path: &PathBuf) -> Result<bool>;
}

pub trait DiskManager {
    async fn setup_luks_disk(&self, device: &str, config: &LuksConfig) -> Result<String>;
    async fn mount_luks_volume(&self, device: &str, passphrase: &str) -> Result<String>;
    async fn unmount_luks_volume(&self, mount_point: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct ImageError {
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image error: {}", self.message)
    }
}

impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &dyn std::error::Error)
    }
}
