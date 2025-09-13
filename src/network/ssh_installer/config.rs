// file: src/network/ssh_installer/config.rs
// version: 1.1.0
// guid: sshcfg01-2345-6789-abcd-ef0123456789

//! Configuration structures for SSH installation

#[derive(Debug, Clone)]
pub struct InstallationConfig {
    pub hostname: String,
    pub disk_device: String,
    pub timezone: String,
    pub luks_key: String,
    pub root_password: String,
    pub network_interface: String,
    pub network_address: String,
    pub network_gateway: String,
    pub network_search: String,
    pub network_nameservers: Vec<String>,
    pub debootstrap_release: Option<String>,
    pub debootstrap_mirror: Option<String>,
}

impl InstallationConfig {
    /// Create configuration for len-serv-003
    pub fn for_len_serv_003() -> Self {
        Self {
            hostname: "len-serv-003".to_string(),
            disk_device: "/dev/nvme0n1".to_string(),
            timezone: "America/New_York".to_string(),
            luks_key: "changeme123!@#".to_string(),
            root_password: "changeme123!@#".to_string(),
            network_interface: "eno1".to_string(),
            network_address: "172.16.3.96/23".to_string(),
            network_gateway: "172.16.2.1".to_string(),
            network_search: "local.jdfalk.com".to_string(),
            network_nameservers: vec!["172.16.2.1".to_string(), "8.8.8.8".to_string()],
            debootstrap_release: Some("oracular".to_string()),
            debootstrap_mirror: Some("http://archive.ubuntu.com/ubuntu/".to_string()),
        }
    }
}

#[derive(Debug, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub kernel_version: String,
    pub os_release: String,
    pub disk_info: String,
    pub network_info: String,
    pub available_tools: Vec<String>,
    pub memory_info: String,
    pub cpu_info: String,
}
