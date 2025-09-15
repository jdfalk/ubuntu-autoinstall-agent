// file: src/config/image.rs
// version: 1.0.0
// guid: c3d4e5f6-g7h8-9012-3456-789012cdefgh

//! Image specification and metadata structures

use super::Architecture;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Golden image specification for building Ubuntu images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSpec {
    /// Ubuntu version (e.g., "24.04", "22.04")
    pub ubuntu_version: String,
    /// Target architecture
    pub architecture: Architecture,
    /// Base packages to install in the image
    pub base_packages: Vec<String>,
    /// Custom scripts to run during image creation
    pub custom_scripts: Vec<PathBuf>,
    /// VM configuration for image building
    pub vm_config: VmConfig,
}

/// Virtual machine configuration for image building
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// Memory allocation in MB
    pub memory_mb: u32,
    /// Disk size in GB
    pub disk_size_gb: u32,
    /// Number of CPU cores
    pub cpu_cores: u32,
}

/// Metadata for a created golden image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Unique image identifier
    pub id: String,
    /// Ubuntu version
    pub ubuntu_version: String,
    /// Architecture
    pub architecture: Architecture,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Image size in bytes
    pub size_bytes: u64,
    /// Image checksum (SHA256)
    pub checksum: String,
    /// Path to image file
    pub path: PathBuf,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            memory_mb: 2048,
            disk_size_gb: 20,
            cpu_cores: 2,
        }
    }
}

impl ImageSpec {
    /// Validate image specification
    pub fn validate(&self) -> crate::Result<()> {
        // Validate Ubuntu version format (basic check)
        if !self.ubuntu_version.matches('.').count() == 1 {
            return Err(crate::error::AutoInstallError::ValidationError(format!(
                "Invalid Ubuntu version format: {}",
                self.ubuntu_version
            )));
        }

        // Validate VM configuration
        if self.vm_config.memory_mb < 1024 {
            return Err(crate::error::AutoInstallError::ValidationError(
                "VM memory must be at least 1024 MB".to_string(),
            ));
        }

        if self.vm_config.disk_size_gb < 10 {
            return Err(crate::error::AutoInstallError::ValidationError(
                "VM disk size must be at least 10 GB".to_string(),
            ));
        }

        if self.vm_config.cpu_cores == 0 {
            return Err(crate::error::AutoInstallError::ValidationError(
                "VM must have at least 1 CPU core".to_string(),
            ));
        }

        // Validate custom scripts exist
        for script in &self.custom_scripts {
            if !script.exists() {
                return Err(crate::error::AutoInstallError::ValidationError(format!(
                    "Custom script not found: {}",
                    script.display()
                )));
            }
        }

        Ok(())
    }

    /// Create a minimal Ubuntu image specification
    pub fn minimal(ubuntu_version: String, architecture: Architecture) -> Self {
        Self {
            ubuntu_version,
            architecture,
            base_packages: vec![
                "openssh-server".to_string(),
                "curl".to_string(),
                "wget".to_string(),
                "htop".to_string(),
                "vim".to_string(),
            ],
            custom_scripts: vec![],
            vm_config: VmConfig::default(),
        }
    }
}

impl ImageInfo {
    /// Create new image info with generated ID
    pub fn new(
        ubuntu_version: String,
        architecture: Architecture,
        size_bytes: u64,
        checksum: String,
        path: PathBuf,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            ubuntu_version,
            architecture,
            created_at: chrono::Utc::now(),
            size_bytes,
            checksum,
            path,
        }
    }

    /// Get human-readable size
    pub fn size_human(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.size_bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
