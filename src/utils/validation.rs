// file: src/utils/validation.rs
// version: 1.0.0
// guid: 9a8b7c6d-5e4f-3a2b-1c0d-9e8f7a6b5c4d

//! Comprehensive validation utilities for the auto-installer system.
//!
//! This module provides validation functions for configuration values, system requirements,
//! dependencies, and installation prerequisites. It ensures that all components of the
//! installation process meet the required criteria before execution begins.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::str::FromStr;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;
use validator::{Validate, ValidationError};
use url::Url;

/// Validation utilities for configuration, system requirements, and prerequisites
#[derive(Debug, Clone)]
pub struct ValidationUtils;

/// System requirements structure
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SystemRequirements {
    /// Minimum RAM in GB
    #[validate(range(min = 1, max = 1024))]
    pub min_memory_gb: u64,

    /// Minimum disk space in GB
    #[validate(range(min = 1, max = 10240))]
    pub min_disk_gb: u64,

    /// Required CPU cores
    #[validate(range(min = 1, max = 256))]
    pub min_cpu_cores: u32,

    /// Required architecture (x86_64, aarch64, etc.)
    #[validate(length(min = 1, max = 20))]
    pub architecture: String,

    /// Required Ubuntu version (minimum)
    #[validate(regex = r"^\d+\.\d+$")]
    pub min_ubuntu_version: String,
}

/// Network configuration validation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NetworkConfig {
    /// Interface name
    #[validate(length(min = 1, max = 15))]
    pub interface: String,

    /// IP address
    #[validate(ip)]
    pub ip_address: String,

    /// Network mask (CIDR notation)
    #[validate(range(min = 1, max = 32))]
    pub netmask: u8,

    /// Gateway IP address
    #[validate(ip)]
    pub gateway: String,

    /// DNS servers
    #[validate(length(min = 1, max = 10))]
    pub dns_servers: Vec<String>,
}

/// Disk configuration validation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DiskConfig {
    /// Device path
    #[validate(regex = r"^/dev/[a-zA-Z0-9]+$")]
    pub device: String,

    /// Partition scheme
    #[validate(custom = "validate_partition_scheme")]
    pub partition_scheme: String,

    /// Filesystem type
    #[validate(custom = "validate_filesystem")]
    pub filesystem: String,

    /// Mount points and their configurations
    pub mount_points: HashMap<String, MountPointConfig>,
}

/// Mount point configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MountPointConfig {
    /// Mount point path
    #[validate(regex = r"^/.*")]
    pub path: String,

    /// Size in GB (0 for remaining space)
    #[validate(range(min = 0, max = 10240))]
    pub size_gb: u64,

    /// Filesystem type
    #[validate(custom = "validate_filesystem")]
    pub filesystem: String,

    /// Mount options
    pub options: Vec<String>,
}

/// User configuration validation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UserConfig {
    /// Username
    #[validate(regex = r"^[a-z_][a-z0-9_-]*$", length(min = 1, max = 32))]
    pub username: String,

    /// Full name
    #[validate(length(min = 1, max = 256))]
    pub full_name: String,

    /// Password hash
    #[validate(length(min = 1, max = 512))]
    pub password_hash: String,

    /// SSH authorized keys
    pub ssh_keys: Vec<String>,

    /// User groups
    pub groups: Vec<String>,

    /// Home directory path
    #[validate(regex = r"^/.*")]
    pub home_dir: String,

    /// Shell path
    #[validate(regex = r"^/.*")]
    pub shell: String,
}

/// Package configuration validation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PackageConfig {
    /// Package name
    #[validate(regex = r"^[a-z0-9+.-]+$", length(min = 1, max = 256))]
    pub name: String,

    /// Version constraint (optional)
    pub version: Option<String>,

    /// Repository source (optional)
    pub source: Option<String>,

    /// Installation priority
    #[validate(range(min = 1, max = 10))]
    pub priority: u8,
}

/// Service configuration validation
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServiceConfig {
    /// Service name
    #[validate(regex = r"^[a-zA-Z0-9_.-]+$", length(min = 1, max = 256))]
    pub name: String,

    /// Service state (enabled, disabled, started, stopped)
    #[validate(custom = "validate_service_state")]
    pub state: String,

    /// Configuration file path (optional)
    pub config_path: Option<String>,

    /// Environment variables
    pub environment: HashMap<String, String>,
}

/// Validation results summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Overall validation success
    pub success: bool,

    /// Validation errors by category
    pub errors: HashMap<String, Vec<String>>,

    /// Validation warnings by category
    pub warnings: HashMap<String, Vec<String>>,

    /// System information gathered during validation
    pub system_info: Option<SystemInfo>,
}

/// System information for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Total RAM in GB
    pub total_memory_gb: f64,

    /// Available disk space in GB
    pub available_disk_gb: f64,

    /// Number of CPU cores
    pub cpu_cores: u32,

    /// System architecture
    pub architecture: String,

    /// Ubuntu version
    pub ubuntu_version: String,

    /// Available network interfaces
    pub network_interfaces: Vec<String>,

    /// Available block devices
    pub block_devices: Vec<String>,
}

impl ValidationUtils {
    /// Create a new validation utils instance
    pub fn new() -> Self {
        Self
    }

    /// Validate system requirements against current system
    pub async fn validate_system_requirements(
        &self,
        requirements: &SystemRequirements,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the requirements structure first
        if let Err(validation_errors) = requirements.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("requirements_validation".to_string(), errors);
            result.success = false;
        }

        // Gather system information
        let system_info = match self.gather_system_info().await {
            Ok(info) => {
                result.system_info = Some(info.clone());
                info
            }
            Err(e) => {
                result.errors.insert("system_info".to_string(), vec![format!("Failed to gather system info: {}", e)]);
                result.success = false;
                return Ok(result);
            }
        };

        // Validate memory requirements
        if system_info.total_memory_gb < requirements.min_memory_gb as f64 {
            result.errors.insert("memory".to_string(), vec![
                format!("Insufficient memory: {} GB available, {} GB required",
                       system_info.total_memory_gb, requirements.min_memory_gb)
            ]);
            result.success = false;
        }

        // Validate disk space requirements
        if system_info.available_disk_gb < requirements.min_disk_gb as f64 {
            result.errors.insert("disk_space".to_string(), vec![
                format!("Insufficient disk space: {} GB available, {} GB required",
                       system_info.available_disk_gb, requirements.min_disk_gb)
            ]);
            result.success = false;
        }

        // Validate CPU core requirements
        if system_info.cpu_cores < requirements.min_cpu_cores {
            result.errors.insert("cpu_cores".to_string(), vec![
                format!("Insufficient CPU cores: {} available, {} required",
                       system_info.cpu_cores, requirements.min_cpu_cores)
            ]);
            result.success = false;
        }

        // Validate architecture
        if system_info.architecture != requirements.architecture {
            result.errors.insert("architecture".to_string(), vec![
                format!("Architecture mismatch: {} detected, {} required",
                       system_info.architecture, requirements.architecture)
            ]);
            result.success = false;
        }

        // Validate Ubuntu version
        if let Err(e) = self.validate_ubuntu_version(&system_info.ubuntu_version, &requirements.min_ubuntu_version) {
            result.errors.insert("ubuntu_version".to_string(), vec![e.to_string()]);
            result.success = false;
        }

        Ok(result)
    }

    /// Validate network configuration
    pub async fn validate_network_config(
        &self,
        config: &NetworkConfig,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the configuration structure
        if let Err(validation_errors) = config.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("network_validation".to_string(), errors);
            result.success = false;
        }

        // Validate IP address format
        if IpAddr::from_str(&config.ip_address).is_err() {
            result.errors.insert("ip_address".to_string(), vec![
                format!("Invalid IP address format: {}", config.ip_address)
            ]);
            result.success = false;
        }

        // Validate gateway IP address format
        if IpAddr::from_str(&config.gateway).is_err() {
            result.errors.insert("gateway".to_string(), vec![
                format!("Invalid gateway IP address format: {}", config.gateway)
            ]);
            result.success = false;
        }

        // Validate DNS servers
        for (i, dns) in config.dns_servers.iter().enumerate() {
            if IpAddr::from_str(dns).is_err() {
                result.errors.entry("dns_servers".to_string())
                    .or_insert_with(Vec::new)
                    .push(format!("Invalid DNS server IP address at index {}: {}", i, dns));
                result.success = false;
            }
        }

        // Check if interface exists on the system
        let system_info = self.gather_system_info().await?;
        if !system_info.network_interfaces.contains(&config.interface) {
            result.warnings.insert("interface".to_string(), vec![
                format!("Network interface '{}' not found on system", config.interface)
            ]);
        }

        Ok(result)
    }

    /// Validate disk configuration
    pub async fn validate_disk_config(
        &self,
        config: &DiskConfig,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the configuration structure
        if let Err(validation_errors) = config.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("disk_validation".to_string(), errors);
            result.success = false;
        }

        // Check if device exists
        if !Path::new(&config.device).exists() {
            result.errors.insert("device".to_string(), vec![
                format!("Block device '{}' not found", config.device)
            ]);
            result.success = false;
        }

        // Validate mount points
        for (name, mount_config) in &config.mount_points {
            if let Err(validation_errors) = mount_config.validate() {
                let mut errors = Vec::new();
                for error in validation_errors.field_errors() {
                    for (field, field_errors) in error {
                        for error in field_errors {
                            errors.push(format!("Mount point '{}', field '{}': {}",
                                               name, field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                        }
                    }
                }
                result.errors.insert("mount_points".to_string(), errors);
                result.success = false;
            }
        }

        Ok(result)
    }

    /// Validate user configuration
    pub async fn validate_user_config(
        &self,
        config: &UserConfig,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the configuration structure
        if let Err(validation_errors) = config.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("user_validation".to_string(), errors);
            result.success = false;
        }

        // Validate SSH keys format
        for (i, key) in config.ssh_keys.iter().enumerate() {
            if let Err(e) = self.validate_ssh_key(key) {
                result.errors.entry("ssh_keys".to_string())
                    .or_insert_with(Vec::new)
                    .push(format!("Invalid SSH key at index {}: {}", i, e));
                result.success = false;
            }
        }

        // Check if shell exists
        if !Path::new(&config.shell).exists() {
            result.warnings.insert("shell".to_string(), vec![
                format!("Shell '{}' not found on system", config.shell)
            ]);
        }

        Ok(result)
    }

    /// Validate package configuration
    pub async fn validate_package_config(
        &self,
        config: &PackageConfig,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the configuration structure
        if let Err(validation_errors) = config.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("package_validation".to_string(), errors);
            result.success = false;
        }

        // Additional package-specific validation can be added here
        // (e.g., checking if package exists in repositories)

        Ok(result)
    }

    /// Validate service configuration
    pub async fn validate_service_config(
        &self,
        config: &ServiceConfig,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            success: true,
            errors: HashMap::new(),
            warnings: HashMap::new(),
            system_info: None,
        };

        // Validate the configuration structure
        if let Err(validation_errors) = config.validate() {
            let mut errors = Vec::new();
            for error in validation_errors.field_errors() {
                for (field, field_errors) in error {
                    for error in field_errors {
                        errors.push(format!("Field '{}': {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into())));
                    }
                }
            }
            result.errors.insert("service_validation".to_string(), errors);
            result.success = false;
        }

        // Check if configuration file exists (if specified)
        if let Some(config_path) = &config.config_path {
            if !Path::new(config_path).exists() {
                result.warnings.insert("config_file".to_string(), vec![
                    format!("Service configuration file '{}' not found", config_path)
                ]);
            }
        }

        Ok(result)
    }

    /// Validate URL format and accessibility
    pub async fn validate_url(&self, url: &str) -> Result<bool> {
        // Parse URL format
        let parsed_url = Url::parse(url)
            .context("Invalid URL format")?;

        // Check if URL uses supported scheme
        match parsed_url.scheme() {
            "http" | "https" | "ftp" | "ftps" => {},
            scheme => return Err(anyhow::anyhow!("Unsupported URL scheme: {}", scheme)),
        }

        Ok(true)
    }

    /// Validate file path and accessibility
    pub async fn validate_file_path(&self, path: &str, must_exist: bool) -> Result<bool> {
        let path = Path::new(path);

        if must_exist && !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
        }

        // Check if parent directory exists and is writable (for new files)
        if !must_exist {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    return Err(anyhow::anyhow!("Parent directory does not exist: {}", parent.display()));
                }

                // Test write access by attempting to create a temporary file
                let temp_file = parent.join(".validation_test");
                if let Err(e) = fs::write(&temp_file, "test").await {
                    return Err(anyhow::anyhow!("Cannot write to directory {}: {}", parent.display(), e));
                }
                let _ = fs::remove_file(&temp_file).await; // Clean up
            }
        }

        Ok(true)
    }

    /// Validate environment variable name
    pub fn validate_env_var_name(&self, name: &str) -> Result<bool> {
        // Environment variable names should start with a letter or underscore
        // and contain only letters, numbers, and underscores
        if name.is_empty() {
            return Err(anyhow::anyhow!("Environment variable name cannot be empty"));
        }

        let first_char = name.chars().next().unwrap();
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            return Err(anyhow::anyhow!("Environment variable name must start with a letter or underscore"));
        }

        for c in name.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Err(anyhow::anyhow!("Environment variable name can only contain letters, numbers, and underscores"));
            }
        }

        Ok(true)
    }

    /// Gather system information for validation
    async fn gather_system_info(&self) -> Result<SystemInfo> {
        use crate::utils::system::SystemUtils;
        use crate::utils::disk::DiskUtils;
        use crate::utils::network::NetworkUtils;

        let system_utils = SystemUtils::new();
        let disk_utils = DiskUtils::new();
        let network_utils = NetworkUtils::new();

        let system_info = system_utils.get_system_info().await?;
        let disk_usage = disk_utils.get_disk_usage("/").await?;
        let network_interfaces = network_utils.get_network_interfaces().await?;
        let block_devices = disk_utils.list_block_devices().await?;

        Ok(SystemInfo {
            total_memory_gb: system_info.memory.total as f64 / 1024.0 / 1024.0 / 1024.0,
            available_disk_gb: disk_usage.available as f64 / 1024.0 / 1024.0 / 1024.0,
            cpu_cores: system_info.cpu.cores,
            architecture: system_info.cpu.architecture,
            ubuntu_version: system_info.os.version,
            network_interfaces: network_interfaces.into_iter().map(|i| i.name).collect(),
            block_devices: block_devices.into_iter().map(|d| d.name).collect(),
        })
    }

    /// Validate Ubuntu version comparison
    fn validate_ubuntu_version(&self, current: &str, required: &str) -> Result<()> {
        let parse_version = |version: &str| -> Result<(u32, u32)> {
            let parts: Vec<&str> = version.split('.').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid version format: {}", version));
            }

            let major = parts[0].parse::<u32>()
                .context("Invalid major version number")?;
            let minor = parts[1].parse::<u32>()
                .context("Invalid minor version number")?;

            Ok((major, minor))
        };

        let (current_major, current_minor) = parse_version(current)?;
        let (required_major, required_minor) = parse_version(required)?;

        if current_major < required_major ||
           (current_major == required_major && current_minor < required_minor) {
            return Err(anyhow::anyhow!(
                "Ubuntu version {} is below minimum required version {}",
                current, required
            ));
        }

        Ok(())
    }

    /// Validate SSH key format
    fn validate_ssh_key(&self, key: &str) -> Result<()> {
        // Basic SSH key format validation
        let parts: Vec<&str> = key.trim().split_whitespace().collect();

        if parts.len() < 2 {
            return Err(anyhow::anyhow!("SSH key must have at least key type and key data"));
        }

        // Check key type
        match parts[0] {
            "ssh-rsa" | "ssh-dss" | "ssh-ed25519" | "ecdsa-sha2-nistp256" |
            "ecdsa-sha2-nistp384" | "ecdsa-sha2-nistp521" => {},
            _ => return Err(anyhow::anyhow!("Unsupported SSH key type: {}", parts[0])),
        }

        // Check key data is base64-encoded
        if let Err(e) = base64::decode(&parts[1]) {
            return Err(anyhow::anyhow!("Invalid base64 encoding in SSH key data: {}", e));
        }

        Ok(())
    }
}

impl Default for ValidationUtils {
    fn default() -> Self {
        Self::new()
    }
}

// Custom validation functions for validator crate

/// Validate partition scheme
fn validate_partition_scheme(scheme: &str) -> Result<(), ValidationError> {
    match scheme {
        "gpt" | "msdos" | "dvh" | "mac" | "bsd" | "loop" | "sun" => Ok(()),
        _ => Err(ValidationError::new("Invalid partition scheme")),
    }
}

/// Validate filesystem type
fn validate_filesystem(fs: &str) -> Result<(), ValidationError> {
    match fs {
        "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "zfs" | "ntfs" | "vfat" |
        "exfat" | "swap" | "tmpfs" | "squashfs" => Ok(()),
        _ => Err(ValidationError::new("Invalid filesystem type")),
    }
}

/// Validate service state
fn validate_service_state(state: &str) -> Result<(), ValidationError> {
    match state {
        "enabled" | "disabled" | "started" | "stopped" | "restarted" | "reloaded" => Ok(()),
        _ => Err(ValidationError::new("Invalid service state")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_validation_utils_creation() {
        let utils = ValidationUtils::new();
        assert!(true); // Constructor succeeds
    }

    #[tokio::test]
    async fn test_system_requirements_validation() {
        let requirements = SystemRequirements {
            min_memory_gb: 4,
            min_disk_gb: 20,
            min_cpu_cores: 2,
            architecture: "x86_64".to_string(),
            min_ubuntu_version: "20.04".to_string(),
        };

        let utils = ValidationUtils::new();
        let result = utils.validate_system_requirements(&requirements).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_network_config_validation() {
        let config = NetworkConfig {
            interface: "eth0".to_string(),
            ip_address: "192.168.1.100".to_string(),
            netmask: 24,
            gateway: "192.168.1.1".to_string(),
            dns_servers: vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
        };

        let utils = ValidationUtils::new();
        let result = utils.validate_network_config(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_ip_address() {
        let config = NetworkConfig {
            interface: "eth0".to_string(),
            ip_address: "invalid.ip".to_string(),
            netmask: 24,
            gateway: "192.168.1.1".to_string(),
            dns_servers: vec!["8.8.8.8".to_string()],
        };

        let utils = ValidationUtils::new();
        let result = utils.validate_network_config(&config).await.unwrap();
        assert!(!result.success);
        assert!(result.errors.contains_key("ip_address"));
    }

    #[tokio::test]
    async fn test_ssh_key_validation() {
        let utils = ValidationUtils::new();

        // Valid SSH key format
        let valid_key = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC7vbqajDjneHyQUEgMSdryqi2DfUsZX1M user@host";
        assert!(utils.validate_ssh_key(valid_key).is_ok());

        // Invalid SSH key format
        let invalid_key = "invalid-key-format";
        assert!(utils.validate_ssh_key(invalid_key).is_err());
    }

    #[tokio::test]
    async fn test_url_validation() {
        let utils = ValidationUtils::new();

        // Valid URLs
        assert!(utils.validate_url("https://example.com").await.is_ok());
        assert!(utils.validate_url("http://example.com/path").await.is_ok());
        assert!(utils.validate_url("ftp://example.com/file").await.is_ok());

        // Invalid URLs
        assert!(utils.validate_url("invalid-url").await.is_err());
        assert!(utils.validate_url("unsupported://example.com").await.is_err());
    }

    #[test]
    fn test_env_var_name_validation() {
        let utils = ValidationUtils::new();

        // Valid environment variable names
        assert!(utils.validate_env_var_name("PATH").is_ok());
        assert!(utils.validate_env_var_name("_PRIVATE_VAR").is_ok());
        assert!(utils.validate_env_var_name("VAR_123").is_ok());

        // Invalid environment variable names
        assert!(utils.validate_env_var_name("").is_err());
        assert!(utils.validate_env_var_name("123VAR").is_err());
        assert!(utils.validate_env_var_name("VAR-NAME").is_err());
        assert!(utils.validate_env_var_name("VAR.NAME").is_err());
    }

    #[test]
    fn test_ubuntu_version_validation() {
        let utils = ValidationUtils::new();

        // Valid version comparisons
        assert!(utils.validate_ubuntu_version("22.04", "20.04").is_ok());
        assert!(utils.validate_ubuntu_version("20.04", "20.04").is_ok());

        // Invalid version comparisons
        assert!(utils.validate_ubuntu_version("18.04", "20.04").is_err());
        assert!(utils.validate_ubuntu_version("20.02", "20.04").is_err());
    }

    #[test]
    fn test_custom_validators() {
        // Test partition scheme validation
        assert!(validate_partition_scheme("gpt").is_ok());
        assert!(validate_partition_scheme("msdos").is_ok());
        assert!(validate_partition_scheme("invalid").is_err());

        // Test filesystem validation
        assert!(validate_filesystem("ext4").is_ok());
        assert!(validate_filesystem("xfs").is_ok());
        assert!(validate_filesystem("btrfs").is_ok());
        assert!(validate_filesystem("invalid").is_err());

        // Test service state validation
        assert!(validate_service_state("enabled").is_ok());
        assert!(validate_service_state("started").is_ok());
        assert!(validate_service_state("invalid").is_err());
    }
}
