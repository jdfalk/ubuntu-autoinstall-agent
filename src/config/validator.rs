// file: src/config/validator.rs
// version: 1.0.0
// guid: e5f6g7h8-i9j0-1234-5678-90abcdef1234

use super::InstallConfig;
use anyhow::{Context, Result};
use std::net::IpAddr;
use std::path::Path;
use validator::Validate;
use tracing::{info, warn, debug};
use regex::Regex;

/// Comprehensive configuration validator
pub struct ConfigValidator {
    /// Validate network accessibility
    pub check_network: bool,

    /// Validate disk accessibility
    pub check_disk: bool,

    /// Validate package availability
    pub check_packages: bool,
}

impl ConfigValidator {
    pub fn new() -> Self {
        Self {
            check_network: true,
            check_disk: true,
            check_packages: false, // Can be slow, disabled by default
        }
    }

    /// Create a validator with all checks enabled
    pub fn strict() -> Self {
        Self {
            check_network: true,
            check_disk: true,
            check_packages: true,
        }
    }

    /// Create a validator with minimal checks
    pub fn lenient() -> Self {
        Self {
            check_network: false,
            check_disk: false,
            check_packages: false,
        }
    }
}

/// Validate the complete configuration
pub fn validate_config(config: &InstallConfig) -> Result<()> {
    let validator = ConfigValidator::new();
    validate_config_with_validator(config, &validator)
}

/// Validate configuration with a specific validator
pub fn validate_config_with_validator(config: &InstallConfig, validator: &ConfigValidator) -> Result<()> {
    info!("Starting configuration validation");

    // First run the basic validation from the Validate trait
    config.validate()
        .context("Basic configuration validation failed")?;

    // Validate hostname
    validate_hostname(&config.hostname)?;

    // Validate disk configuration
    if validator.check_disk {
        validate_disk(&config.disk)?;
    }

    // Validate network configuration
    if validator.check_network {
        validate_network_config(&config.network)?;
    }

    // Validate user configurations
    validate_users(&config.users)?;

    // Validate ZFS configuration
    validate_zfs_config(&config.zfs)?;

    // Validate encryption configuration
    validate_encryption_config(&config.encryption)?;

    // Validate package list
    if validator.check_packages {
        validate_packages(&config.packages).await?;
    }

    // Validate reporting configuration
    if let Some(ref reporting) = config.reporting {
        validate_reporting_config(reporting)?;
    }

    // Validate recovery configuration
    validate_recovery_config(&config.recovery)?;

    info!("Configuration validation completed successfully");
    Ok(())
}

/// Validate hostname according to RFC standards
fn validate_hostname(hostname: &str) -> Result<()> {
    debug!("Validating hostname: {}", hostname);

    if hostname.is_empty() {
        anyhow::bail!("Hostname cannot be empty");
    }

    if hostname.len() > 63 {
        anyhow::bail!("Hostname cannot be longer than 63 characters");
    }

    // RFC 1123 hostname validation
    let hostname_regex = Regex::new(r"^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?$")
        .expect("Invalid hostname regex");

    if !hostname_regex.is_match(hostname) {
        anyhow::bail!("Hostname '{}' does not meet RFC 1123 standards", hostname);
    }

    // Cannot start or end with hyphen
    if hostname.starts_with('-') || hostname.ends_with('-') {
        anyhow::bail!("Hostname cannot start or end with a hyphen");
    }

    Ok(())
}

/// Validate disk device path
fn validate_disk(disk: &str) -> Result<()> {
    debug!("Validating disk: {}", disk);

    if !disk.starts_with("/dev/") {
        anyhow::bail!("Disk path must start with /dev/");
    }

    let disk_path = Path::new(disk);
    if !disk_path.exists() {
        warn!("Disk device {} does not exist", disk);
        // Don't fail validation as device might not be available during config validation
    }

    Ok(())
}

/// Validate network configuration
fn validate_network_config(network: &super::NetworkConfig) -> Result<()> {
    debug!("Validating network configuration");

    // Validate ethernet configuration
    validate_ip_with_cidr(&network.ethernet.address)
        .context("Invalid ethernet IP address")?;

    validate_ip_address(&network.ethernet.gateway)
        .context("Invalid ethernet gateway")?;

    // Validate WiFi configuration if present
    if let Some(ref wifi) = network.wifi {
        validate_ip_with_cidr(&wifi.address)
            .context("Invalid WiFi IP address")?;

        if wifi.ssid.is_empty() {
            anyhow::bail!("WiFi SSID cannot be empty");
        }

        if wifi.password.len() < 8 {
            anyhow::bail!("WiFi password must be at least 8 characters");
        }
    }

    // Validate DNS servers
    for dns in &network.dns_servers {
        validate_ip_address(dns)
            .with_context(|| format!("Invalid DNS server address: {}", dns))?;
    }

    Ok(())
}

/// Validate IP address with CIDR notation
fn validate_ip_with_cidr(ip_cidr: &str) -> Result<()> {
    let parts: Vec<&str> = ip_cidr.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("IP address must include CIDR notation (e.g., 192.168.1.100/24)");
    }

    validate_ip_address(parts[0])?;

    let prefix: u8 = parts[1].parse()
        .with_context(|| format!("Invalid CIDR prefix: {}", parts[1]))?;

    if prefix > 32 {
        anyhow::bail!("CIDR prefix cannot be greater than 32");
    }

    Ok(())
}

/// Validate IP address format
fn validate_ip_address(ip: &str) -> Result<()> {
    ip.parse::<IpAddr>()
        .with_context(|| format!("Invalid IP address format: {}", ip))?;
    Ok(())
}

/// Validate user configurations
fn validate_users(users: &[super::UserConfig]) -> Result<()> {
    debug!("Validating user configurations");

    if users.is_empty() {
        anyhow::bail!("At least one user must be configured");
    }

    for user in users {
        validate_username(&user.username)?;
        validate_password_hash(&user.password_hash)?;
        validate_shell(&user.shell)?;

        for ssh_key in &user.ssh_keys {
            validate_ssh_key(ssh_key)?;
        }
    }

    Ok(())
}

/// Validate username format
fn validate_username(username: &str) -> Result<()> {
    if username.is_empty() {
        anyhow::bail!("Username cannot be empty");
    }

    if username.len() > 32 {
        anyhow::bail!("Username cannot be longer than 32 characters");
    }

    let username_regex = Regex::new(r"^[a-z][a-z0-9_-]*$")
        .expect("Invalid username regex");

    if !username_regex.is_match(username) {
        anyhow::bail!("Username '{}' contains invalid characters", username);
    }

    Ok(())
}

/// Validate password hash format
fn validate_password_hash(hash: &str) -> Result<()> {
    if hash.is_empty() {
        anyhow::bail!("Password hash cannot be empty");
    }

    // Check for common hash formats (SHA-512, SHA-256, etc.)
    if !hash.starts_with('$') {
        anyhow::bail!("Password hash must be in crypt format (starting with $)");
    }

    // Basic structure validation for crypt hashes
    let parts: Vec<&str> = hash.split('$').collect();
    if parts.len() < 4 {
        anyhow::bail!("Password hash format appears invalid");
    }

    Ok(())
}

/// Validate shell path
fn validate_shell(shell: &str) -> Result<()> {
    if shell.is_empty() {
        anyhow::bail!("Shell cannot be empty");
    }

    if !shell.starts_with('/') {
        anyhow::bail!("Shell must be an absolute path");
    }

    // Common shells validation
    let valid_shells = [
        "/bin/bash", "/bin/sh", "/bin/zsh", "/bin/fish",
        "/usr/bin/bash", "/usr/bin/zsh", "/usr/bin/fish",
        "/bin/dash", "/usr/bin/dash"
    ];

    if !valid_shells.contains(&shell) {
        warn!("Shell '{}' is not in the list of common shells", shell);
    }

    Ok(())
}

/// Validate SSH public key format
fn validate_ssh_key(ssh_key: &str) -> Result<()> {
    if ssh_key.is_empty() {
        anyhow::bail!("SSH key cannot be empty");
    }

    // Basic SSH key format validation
    let parts: Vec<&str> = ssh_key.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("SSH key format appears invalid");
    }

    let key_type = parts[0];
    let valid_types = ["ssh-rsa", "ssh-dss", "ssh-ed25519", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521"];

    if !valid_types.contains(&key_type) {
        anyhow::bail!("Unknown SSH key type: {}", key_type);
    }

    Ok(())
}

/// Validate ZFS configuration
fn validate_zfs_config(zfs: &super::ZfsConfig) -> Result<()> {
    debug!("Validating ZFS configuration");

    if zfs.root_pool.is_empty() {
        anyhow::bail!("ZFS root pool name cannot be empty");
    }

    if zfs.boot_pool.is_empty() {
        anyhow::bail!("ZFS boot pool name cannot be empty");
    }

    if zfs.root_pool == zfs.boot_pool {
        anyhow::bail!("ZFS root and boot pools must have different names");
    }

    // Validate pool names (ZFS naming rules)
    validate_zfs_name(&zfs.root_pool, "root pool")?;
    validate_zfs_name(&zfs.boot_pool, "boot pool")?;

    // Validate datasets
    for dataset in &zfs.datasets {
        validate_zfs_name(&dataset.name, "dataset")?;
    }

    Ok(())
}

/// Validate ZFS names according to ZFS naming rules
fn validate_zfs_name(name: &str, type_name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("{} name cannot be empty", type_name);
    }

    if name.len() > 256 {
        anyhow::bail!("{} name cannot be longer than 256 characters", type_name);
    }

    // ZFS names cannot start with certain characters
    if name.starts_with('-') || name.starts_with('.') {
        anyhow::bail!("{} name cannot start with '-' or '.'", type_name);
    }

    // Check for invalid characters
    let invalid_chars = ['%', ' ', '\t', '\n', '\r'];
    for &ch in &invalid_chars {
        if name.contains(ch) {
            anyhow::bail!("{} name contains invalid character: '{}'", type_name, ch);
        }
    }

    Ok(())
}

/// Validate encryption configuration
fn validate_encryption_config(encryption: &super::EncryptionConfig) -> Result<()> {
    debug!("Validating encryption configuration");

    if encryption.enabled {
        if encryption.luks_key.is_none() && encryption.tang_servers.is_empty() {
            anyhow::bail!("When encryption is enabled, either LUKS key or Tang servers must be configured");
        }

        if let Some(ref luks_key) = encryption.luks_key {
            if luks_key.len() < 8 {
                anyhow::bail!("LUKS key must be at least 8 characters");
            }
        }

        // Validate Tang server URLs
        for tang_url in &encryption.tang_servers {
            validate_url(tang_url)
                .with_context(|| format!("Invalid Tang server URL: {}", tang_url))?;
        }

        // Validate Clevis configuration
        if let Some(ref clevis) = encryption.clevis_config {
            if clevis.threshold == 0 {
                anyhow::bail!("Clevis threshold must be greater than 0");
            }

            if clevis.threshold as usize > clevis.tang_servers.len() {
                anyhow::bail!("Clevis threshold cannot be greater than number of Tang servers");
            }

            for tang_server in &clevis.tang_servers {
                validate_url(&tang_server.url)
                    .with_context(|| format!("Invalid Tang server URL: {}", tang_server.url))?;
            }
        }
    }

    Ok(())
}

/// Validate URL format
fn validate_url(url: &str) -> Result<()> {
    reqwest::Url::parse(url)
        .with_context(|| format!("Invalid URL format: {}", url))?;
    Ok(())
}

/// Validate package list (requires network access)
async fn validate_packages(packages: &[String]) -> Result<()> {
    debug!("Validating package list");

    if packages.is_empty() {
        warn!("No packages specified for installation");
        return Ok(());
    }

    // This is a basic validation - in a real implementation,
    // you might want to check package availability in repositories
    for package in packages {
        if package.is_empty() {
            anyhow::bail!("Package name cannot be empty");
        }

        if package.contains(' ') {
            anyhow::bail!("Package name cannot contain spaces: '{}'", package);
        }
    }

    Ok(())
}

/// Validate reporting configuration
fn validate_reporting_config(reporting: &super::ReportingConfig) -> Result<()> {
    debug!("Validating reporting configuration");

    validate_url(&reporting.webhook_url)
        .context("Invalid webhook URL")?;

    if reporting.interval == 0 {
        anyhow::bail!("Reporting interval must be greater than 0");
    }

    if reporting.interval > 3600 {
        anyhow::bail!("Reporting interval cannot be greater than 1 hour (3600 seconds)");
    }

    Ok(())
}

/// Validate recovery configuration
fn validate_recovery_config(recovery: &super::RecoveryConfig) -> Result<()> {
    debug!("Validating recovery configuration");

    if recovery.max_retries > 10 {
        anyhow::bail!("Maximum retries cannot be greater than 10");
    }

    if recovery.retry_delay == 0 {
        anyhow::bail!("Retry delay must be greater than 0");
    }

    if recovery.retry_delay > 300 {
        anyhow::bail!("Retry delay cannot be greater than 5 minutes (300 seconds)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hostname() {
        assert!(validate_hostname("valid-hostname").is_ok());
        assert!(validate_hostname("test123").is_ok());
        assert!(validate_hostname("").is_err());
        assert!(validate_hostname("-invalid").is_err());
        assert!(validate_hostname("invalid-").is_err());
    }

    #[test]
    fn test_validate_ip_with_cidr() {
        assert!(validate_ip_with_cidr("192.168.1.100/24").is_ok());
        assert!(validate_ip_with_cidr("10.0.0.1/8").is_ok());
        assert!(validate_ip_with_cidr("192.168.1.100").is_err());
        assert!(validate_ip_with_cidr("192.168.1.100/40").is_err());
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("validuser").is_ok());
        assert!(validate_username("user123").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("User").is_err()); // Capital letter
        assert!(validate_username("123user").is_err()); // Starts with number
        assert!(validate_username("").is_err());
    }

    #[test]
    fn test_validate_zfs_name() {
        assert!(validate_zfs_name("validpool", "pool").is_ok());
        assert!(validate_zfs_name("pool123", "pool").is_ok());
        assert!(validate_zfs_name("-invalid", "pool").is_err());
        assert!(validate_zfs_name(".invalid", "pool").is_err());
        assert!(validate_zfs_name("pool with spaces", "pool").is_err());
    }
}
