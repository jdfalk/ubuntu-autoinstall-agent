// file: src/config/mod.rs
// version: 1.0.0
// guid: c3d4e5f6-g7h8-9012-cdef-g34567890123

use serde::{Deserialize, Serialize};
use validator::Validate;
use std::collections::HashMap;

pub mod loader;
pub mod generator;
pub mod validator;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InstallConfig {
    /// Server hostname
    #[validate(length(min = 1, max = 63))]
    pub hostname: String,

    /// Primary disk device (e.g., /dev/nvme0n1)
    #[validate(regex = "^/dev/[a-zA-Z0-9]+$")]
    pub disk: String,

    /// System timezone
    pub timezone: String,

    /// Network configuration
    #[validate]
    pub network: NetworkConfig,

    /// Disk encryption settings
    #[validate]
    pub encryption: EncryptionConfig,

    /// User accounts to create
    #[validate]
    pub users: Vec<UserConfig>,

    /// Packages to install
    pub packages: Vec<String>,

    /// Services to enable
    pub services: Vec<String>,

    /// ZFS configuration
    #[validate]
    pub zfs: ZfsConfig,

    /// Reporting configuration
    pub reporting: Option<ReportingConfig>,

    /// Recovery options
    pub recovery: RecoveryConfig,

    /// Custom scripts to run at various stages
    pub custom_scripts: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NetworkConfig {
    /// Ethernet interface configuration
    #[validate]
    pub ethernet: EthernetConfig,

    /// WiFi configuration (optional)
    pub wifi: Option<WifiConfig>,

    /// DNS servers
    pub dns_servers: Vec<String>,

    /// Search domains
    pub search_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct EthernetConfig {
    /// Interface name (e.g., enp1s0f0)
    #[validate(length(min = 1))]
    pub interface: String,

    /// IP address with CIDR (e.g., 172.16.3.92/23)
    #[validate(regex = r"^(\d{1,3}\.){3}\d{1,3}/\d{1,2}$")]
    pub address: String,

    /// Gateway IP address
    #[validate(regex = r"^(\d{1,3}\.){3}\d{1,3}$")]
    pub gateway: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct WifiConfig {
    /// WiFi interface name
    #[validate(length(min = 1))]
    pub interface: String,

    /// IP address with CIDR
    #[validate(regex = r"^(\d{1,3}\.){3}\d{1,3}/\d{1,2}$")]
    pub address: String,

    /// SSID
    #[validate(length(min = 1, max = 32))]
    pub ssid: String,

    /// WiFi password
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct EncryptionConfig {
    /// Enable LUKS encryption
    pub enabled: bool,

    /// LUKS passphrase/key
    #[validate(length(min = 8))]
    pub luks_key: Option<String>,

    /// Tang servers for network-bound disk encryption
    pub tang_servers: Vec<String>,

    /// Clevis binding configuration
    pub clevis_config: Option<ClevisConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ClevisConfig {
    /// Threshold for secret sharing
    #[validate(range(min = 1, max = 10))]
    pub threshold: u8,

    /// Tang server configurations
    pub tang_servers: Vec<TangServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TangServerConfig {
    /// Tang server URL
    #[validate(url)]
    pub url: String,

    /// Optional server thumbprint for validation
    pub thumbprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UserConfig {
    /// Username
    #[validate(regex = "^[a-z][a-z0-9_-]*$")]
    pub username: String,

    /// Password hash (crypt format)
    pub password_hash: String,

    /// User groups
    pub groups: Vec<String>,

    /// Shell
    pub shell: String,

    /// SSH public keys
    pub ssh_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ZfsConfig {
    /// Root pool name
    #[validate(length(min = 1))]
    pub root_pool: String,

    /// Boot pool name
    #[validate(length(min = 1))]
    pub boot_pool: String,

    /// ZFS properties
    pub properties: HashMap<String, String>,

    /// Additional datasets to create
    pub datasets: Vec<ZfsDataset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ZfsDataset {
    /// Dataset name
    #[validate(length(min = 1))]
    pub name: String,

    /// Mount point
    pub mountpoint: Option<String>,

    /// Dataset properties
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ReportingConfig {
    /// Webhook URL for status updates
    #[validate(url)]
    pub webhook_url: String,

    /// Reporting interval in seconds
    #[validate(range(min = 1, max = 3600))]
    pub interval: u32,

    /// Include system metrics in reports
    pub include_metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RecoveryConfig {
    /// Enable automatic recovery attempts
    pub auto_recovery: bool,

    /// Maximum retry attempts per step
    #[validate(range(min = 0, max = 10))]
    pub max_retries: u8,

    /// Delay between retry attempts (seconds)
    #[validate(range(min = 1, max = 300))]
    pub retry_delay: u32,

    /// Enable rollback on failure
    pub enable_rollback: bool,

    /// Emergency contact information
    pub emergency_contact: Option<String>,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            hostname: "ubuntu-server".to_string(),
            disk: "/dev/sda".to_string(),
            timezone: "UTC".to_string(),
            network: NetworkConfig::default(),
            encryption: EncryptionConfig::default(),
            users: vec![UserConfig::default()],
            packages: vec![
                "openssh-server".to_string(),
                "curl".to_string(),
                "vim".to_string(),
                "htop".to_string(),
                "zfsutils-linux".to_string(),
                "cryptsetup".to_string(),
            ],
            services: vec![
                "ssh".to_string(),
                "systemd-timesyncd".to_string(),
            ],
            zfs: ZfsConfig::default(),
            reporting: None,
            recovery: RecoveryConfig::default(),
            custom_scripts: HashMap::new(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ethernet: EthernetConfig {
                interface: "eth0".to_string(),
                address: "192.168.1.100/24".to_string(),
                gateway: "192.168.1.1".to_string(),
            },
            wifi: None,
            dns_servers: vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
            search_domains: vec!["local".to_string()],
        }
    }
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            luks_key: None,
            tang_servers: vec![],
            clevis_config: None,
        }
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            username: "ubuntu".to_string(),
            password_hash: "$6$rounds=4096$saltsalt$MfUvCKyI5oGxliXbfzRWbFGNsGR4DNvO3IvR9QtYIuQJ98EOjH3UfgbYGqV8UxZlUZzxOHXq64Av/JOoQxeJM.".to_string(),
            groups: vec!["sudo".to_string(), "adm".to_string()],
            shell: "/bin/bash".to_string(),
            ssh_keys: vec![],
        }
    }
}

impl Default for ZfsConfig {
    fn default() -> Self {
        Self {
            root_pool: "rpool".to_string(),
            boot_pool: "bpool".to_string(),
            properties: HashMap::new(),
            datasets: vec![],
        }
    }
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            auto_recovery: true,
            max_retries: 3,
            retry_delay: 30,
            enable_rollback: true,
            emergency_contact: None,
        }
    }
}
