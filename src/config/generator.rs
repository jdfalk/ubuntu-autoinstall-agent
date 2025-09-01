// file: src/config/generator.rs
// version: 1.0.0
// guid: f6g7h8i9-j0k1-2345-6789-0abcdef12345

use super::{InstallConfig, NetworkConfig, EthernetConfig, EncryptionConfig, UserConfig, ZfsConfig, RecoveryConfig};
use anyhow::{Context, Result};
use serde_yaml;
use std::collections::HashMap;
use tracing::{info, debug};

/// Configuration generator for creating installation configurations
pub struct ConfigGenerator {
    /// Base configuration template
    base_config: InstallConfig,
}

impl ConfigGenerator {
    /// Create a new configuration generator with default settings
    pub fn new() -> Self {
        Self {
            base_config: InstallConfig::default(),
        }
    }

    /// Create a generator with a custom base configuration
    pub fn with_base_config(base_config: InstallConfig) -> Self {
        Self {
            base_config,
        }
    }

    /// Generate a minimal configuration for basic server setup
    pub fn generate_minimal(&self, hostname: &str, disk: &str) -> Result<InstallConfig> {
        info!("Generating minimal configuration for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // Minimal package set
        config.packages = vec![
            "openssh-server".to_string(),
            "curl".to_string(),
            "vim".to_string(),
        ];

        // Basic services
        config.services = vec![
            "ssh".to_string(),
        ];

        Ok(config)
    }

    /// Generate configuration for ZFS-enabled server
    pub fn generate_zfs_server(
        &self,
        hostname: &str,
        disk: &str,
        root_pool: &str,
        boot_pool: &str,
    ) -> Result<InstallConfig> {
        info!("Generating ZFS server configuration for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // ZFS-specific packages
        config.packages = vec![
            "openssh-server".to_string(),
            "curl".to_string(),
            "vim".to_string(),
            "htop".to_string(),
            "zfsutils-linux".to_string(),
        ];

        // ZFS configuration
        config.zfs.root_pool = root_pool.to_string();
        config.zfs.boot_pool = boot_pool.to_string();

        // ZFS properties
        config.zfs.properties.insert("compression".to_string(), "lz4".to_string());
        config.zfs.properties.insert("atime".to_string(), "off".to_string());
        config.zfs.properties.insert("xattr".to_string(), "sa".to_string());

        Ok(config)
    }

    /// Generate configuration for encrypted server
    pub fn generate_encrypted_server(
        &self,
        hostname: &str,
        disk: &str,
        luks_passphrase: &str,
    ) -> Result<InstallConfig> {
        info!("Generating encrypted server configuration for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // Encryption packages
        config.packages.extend(vec![
            "cryptsetup".to_string(),
            "cryptsetup-initramfs".to_string(),
        ]);

        // Enable encryption
        config.encryption = EncryptionConfig {
            enabled: true,
            luks_key: Some(luks_passphrase.to_string()),
            tang_servers: vec![],
            clevis_config: None,
        };

        Ok(config)
    }

    /// Generate configuration with network-bound disk encryption (Tang)
    pub fn generate_tang_encrypted_server(
        &self,
        hostname: &str,
        disk: &str,
        tang_servers: Vec<String>,
    ) -> Result<InstallConfig> {
        info!("Generating Tang-encrypted server configuration for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // Tang and Clevis packages
        config.packages.extend(vec![
            "cryptsetup".to_string(),
            "clevis".to_string(),
            "clevis-luks".to_string(),
            "clevis-initramfs".to_string(),
            "tang".to_string(),
        ]);

        // Configure Tang encryption
        config.encryption = EncryptionConfig {
            enabled: true,
            luks_key: None,
            tang_servers: tang_servers.clone(),
            clevis_config: Some(super::ClevisConfig {
                threshold: 1,
                tang_servers: tang_servers
                    .into_iter()
                    .map(|url| super::TangServerConfig {
                        url,
                        thumbprint: None,
                    })
                    .collect(),
            }),
        };

        Ok(config)
    }

    /// Generate configuration for development server
    pub fn generate_dev_server(&self, hostname: &str, disk: &str) -> Result<InstallConfig> {
        info!("Generating development server configuration for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // Development packages
        config.packages = vec![
            "openssh-server".to_string(),
            "curl".to_string(),
            "wget".to_string(),
            "vim".to_string(),
            "nano".to_string(),
            "htop".to_string(),
            "git".to_string(),
            "build-essential".to_string(),
            "python3".to_string(),
            "python3-pip".to_string(),
            "nodejs".to_string(),
            "npm".to_string(),
            "docker.io".to_string(),
            "docker-compose".to_string(),
        ];

        // Development services
        config.services.extend(vec![
            "docker".to_string(),
        ]);

        // Add developer user to docker group
        if let Some(user) = config.users.first_mut() {
            user.groups.push("docker".to_string());
        }

        Ok(config)
    }

    /// Generate configuration with custom networking
    pub fn generate_with_network(
        &self,
        hostname: &str,
        disk: &str,
        interface: &str,
        ip_cidr: &str,
        gateway: &str,
        dns_servers: Vec<String>,
    ) -> Result<InstallConfig> {
        info!("Generating configuration with custom network for {}", hostname);

        let mut config = self.base_config.clone();
        config.hostname = hostname.to_string();
        config.disk = disk.to_string();

        // Custom network configuration
        config.network = NetworkConfig {
            ethernet: EthernetConfig {
                interface: interface.to_string(),
                address: ip_cidr.to_string(),
                gateway: gateway.to_string(),
            },
            wifi: None,
            dns_servers,
            search_domains: vec!["local".to_string()],
        };

        Ok(config)
    }

    /// Generate configuration from autoinstall user-data
    pub fn from_autoinstall_userdata(&self, userdata_path: &str) -> Result<InstallConfig> {
        info!("Generating configuration from autoinstall user-data: {}", userdata_path);

        // This would parse existing cloud-init user-data and extract
        // relevant configuration. For now, return a basic config.
        let mut config = self.base_config.clone();

        // TODO: Implement actual parsing of cloud-init user-data
        // This would involve:
        // 1. Reading the YAML file
        // 2. Extracting network configuration
        // 3. Extracting user configuration
        // 4. Extracting package lists
        // 5. Converting to our configuration format

        config.hostname = "converted-from-userdata".to_string();

        Ok(config)
    }

    /// Generate configuration with custom user
    pub fn with_user(
        &mut self,
        username: &str,
        password_hash: &str,
        groups: Vec<String>,
        ssh_keys: Vec<String>,
    ) -> &mut Self {
        let user = UserConfig {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            groups,
            shell: "/bin/bash".to_string(),
            ssh_keys,
        };

        self.base_config.users = vec![user];
        self
    }

    /// Add additional packages to the configuration
    pub fn with_packages(&mut self, packages: Vec<String>) -> &mut Self {
        self.base_config.packages.extend(packages);
        self
    }

    /// Add custom scripts to the configuration
    pub fn with_custom_scripts(&mut self, stage: &str, scripts: Vec<String>) -> &mut Self {
        self.base_config
            .custom_scripts
            .entry(stage.to_string())
            .or_insert_with(Vec::new)
            .extend(scripts);
        self
    }

    /// Enable reporting with webhook
    pub fn with_reporting(&mut self, webhook_url: &str, interval: u32) -> &mut Self {
        self.base_config.reporting = Some(super::ReportingConfig {
            webhook_url: webhook_url.to_string(),
            interval,
            include_metrics: true,
        });
        self
    }

    /// Configure recovery settings
    pub fn with_recovery(&mut self, max_retries: u8, retry_delay: u32, auto_recovery: bool) -> &mut Self {
        self.base_config.recovery = RecoveryConfig {
            auto_recovery,
            max_retries,
            retry_delay,
            enable_rollback: true,
            emergency_contact: None,
        };
        self
    }

    /// Generate the configuration
    pub fn build(&self) -> InstallConfig {
        debug!("Building configuration");
        self.base_config.clone()
    }

    /// Generate and save configuration to file
    pub async fn save_to_file(&self, file_path: &str) -> Result<()> {
        let config = self.build();
        let yaml_content = serde_yaml::to_string(&config)
            .context("Failed to serialize configuration to YAML")?;

        tokio::fs::write(file_path, yaml_content)
            .await
            .with_context(|| format!("Failed to write configuration to: {}", file_path))?;

        info!("Configuration saved to: {}", file_path);
        Ok(())
    }

    /// Generate configuration as YAML string
    pub fn to_yaml(&self) -> Result<String> {
        let config = self.build();
        serde_yaml::to_string(&config)
            .context("Failed to serialize configuration to YAML")
    }

    /// Generate configuration from template with substitutions
    pub fn from_template(
        &self,
        template_path: &str,
        substitutions: &HashMap<String, String>,
    ) -> Result<InstallConfig> {
        info!("Generating configuration from template: {}", template_path);

        // Load template
        let template_content = std::fs::read_to_string(template_path)
            .with_context(|| format!("Failed to read template: {}", template_path))?;

        // Perform substitutions
        let mut processed_content = template_content;
        for (key, value) in substitutions {
            let placeholder = format!("{{{{{}}}}}", key);
            processed_content = processed_content.replace(&placeholder, value);
        }

        // Parse as configuration
        let config: InstallConfig = serde_yaml::from_str(&processed_content)
            .context("Failed to parse template as configuration")?;

        Ok(config)
    }

    /// Create a configuration preset for specific use cases
    pub fn create_preset(preset_name: &str) -> Result<InstallConfig> {
        info!("Creating configuration preset: {}", preset_name);

        let generator = ConfigGenerator::new();

        match preset_name {
            "minimal" => generator.generate_minimal("ubuntu-minimal", "/dev/sda"),
            "zfs-server" => generator.generate_zfs_server("ubuntu-zfs", "/dev/sda", "rpool", "bpool"),
            "encrypted" => generator.generate_encrypted_server("ubuntu-encrypted", "/dev/sda", "defaultLUKSkey123"),
            "development" => generator.generate_dev_server("ubuntu-dev", "/dev/sda"),
            _ => anyhow::bail!("Unknown preset: {}", preset_name),
        }
    }
}

impl Default for ConfigGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for common configuration tasks
pub mod utils {
    use super::*;

    /// Generate a secure password hash using SHA-512
    pub fn generate_password_hash(password: &str, salt: Option<&str>) -> Result<String> {
        use sha2::{Sha512, Digest};
        use rand::Rng;

        let salt = salt.unwrap_or_else(|| {
            let mut rng = rand::thread_rng();
            let salt_bytes: [u8; 16] = rng.gen();
            &hex::encode(salt_bytes)[..16]
        });

        let rounds = 4096;
        let hash_input = format!("{}${}${}", password, salt, rounds);
        let mut hasher = Sha512::new();
        hasher.update(hash_input.as_bytes());
        let result = hasher.finalize();

        let hash = format!("$6$rounds={}${}${}", rounds, salt, hex::encode(result)[..86]);
        Ok(hash)
    }

    /// Generate SSH key pair (returns public key string)
    pub fn generate_ssh_keypair() -> Result<(String, String)> {
        // This is a placeholder - in a real implementation you'd use
        // a proper SSH key generation library
        let public_key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG4rT3vTt99Ox5kndS4HmgTrKBT8SKzhK4rhGkEVGlCI generated@autoinstaller".to_string();
        let private_key = "-----BEGIN OPENSSH PRIVATE KEY-----\n[PLACEHOLDER PRIVATE KEY]\n-----END OPENSSH PRIVATE KEY-----".to_string();

        Ok((public_key, private_key))
    }

    /// Validate disk device exists and is suitable for installation
    pub async fn validate_installation_disk(disk: &str) -> Result<bool> {
        use tokio::process::Command;

        let output = Command::new("lsblk")
            .arg("-n")
            .arg("-o")
            .arg("NAME,SIZE,TYPE")
            .arg(disk)
            .output()
            .await
            .context("Failed to run lsblk command")?;

        if !output.status.success() {
            return Ok(false);
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 output from lsblk")?;

        // Basic validation - check if it's a disk device
        Ok(!output_str.is_empty() && output_str.contains("disk"))
    }

    /// Detect network interfaces on the system
    pub async fn detect_network_interfaces() -> Result<Vec<String>> {
        use network_interface::{NetworkInterface, NetworkInterfaceConfig};

        let interfaces = NetworkInterface::show()
            .context("Failed to get network interfaces")?;

        let interface_names: Vec<String> = interfaces
            .into_iter()
            .filter(|iface| !iface.name.starts_with("lo")) // Exclude loopback
            .map(|iface| iface.name)
            .collect();

        Ok(interface_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_minimal() {
        let generator = ConfigGenerator::new();
        let config = generator.generate_minimal("test-host", "/dev/sdb").unwrap();

        assert_eq!(config.hostname, "test-host");
        assert_eq!(config.disk, "/dev/sdb");
        assert!(config.packages.contains(&"openssh-server".to_string()));
    }

    #[test]
    fn test_generate_zfs_server() {
        let generator = ConfigGenerator::new();
        let config = generator.generate_zfs_server("zfs-host", "/dev/nvme0n1", "tank", "bootpool").unwrap();

        assert_eq!(config.hostname, "zfs-host");
        assert_eq!(config.zfs.root_pool, "tank");
        assert_eq!(config.zfs.boot_pool, "bootpool");
        assert!(config.packages.contains(&"zfsutils-linux".to_string()));
    }

    #[test]
    fn test_builder_pattern() {
        let mut generator = ConfigGenerator::new();
        let config = generator
            .with_packages(vec!["git".to_string(), "vim".to_string()])
            .with_reporting("https://webhook.example.com", 300)
            .build();

        assert!(config.packages.contains(&"git".to_string()));
        assert!(config.reporting.is_some());
        assert_eq!(config.reporting.unwrap().interval, 300);
    }

    #[test]
    fn test_create_preset() {
        let config = ConfigGenerator::create_preset("minimal").unwrap();
        assert_eq!(config.hostname, "ubuntu-minimal");

        let config = ConfigGenerator::create_preset("development").unwrap();
        assert!(config.packages.contains(&"git".to_string()));
        assert!(config.packages.contains(&"docker.io".to_string()));
    }
}
