// file: src/image/customizer.rs
// version: 1.0.0
// guid: d4e5f6a7-b8c9-0123-4567-890123defghi

//! Image customization utilities for adapting golden images to specific machines
//!
//! This module provides utilities for:
//! - Machine-specific configuration injection
//! - Hardware-specific driver installation
//! - Security configuration (SSH keys, certificates, etc.)
//! - Network configuration customization

use super::{TargetMachine, NetworkConfig};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomizationTemplate {
    pub name: String,
    pub description: String,
    pub files: Vec<FileTemplate>,
    pub commands: Vec<CommandTemplate>,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTemplate {
    pub path: String,
    pub content: String,
    pub permissions: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub command: String,
    pub description: String,
    pub run_in_chroot: bool,
}

pub struct ImageCustomizer {
    templates: HashMap<String, CustomizationTemplate>,
    mount_point: PathBuf,
}

impl ImageCustomizer {
    pub fn new(mount_point: PathBuf) -> Self {
        Self {
            templates: HashMap::new(),
            mount_point,
        }
    }

    /// Load customization templates from a directory
    pub async fn load_templates(&mut self, template_dir: &PathBuf) -> Result<()> {
        if !template_dir.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(template_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                let content = fs::read_to_string(&path).await?;
                let template: CustomizationTemplate = serde_yaml::from_str(&content)
                    .with_context(|| format!("Failed to parse template: {:?}", path))?;

                self.templates.insert(template.name.clone(), template);
            }
        }

        Ok(())
    }

    /// Apply customizations to the mounted filesystem
    pub async fn customize(&self, target: &TargetMachine, template_name: Option<&str>) -> Result<()> {
        // Apply basic customizations
        self.apply_basic_customizations(target).await?;

        // Apply template-based customizations if specified
        if let Some(template_name) = template_name {
            if let Some(template) = self.templates.get(template_name) {
                self.apply_template(template, target).await?;
            }
        }

        Ok(())
    }

    async fn apply_basic_customizations(&self, target: &TargetMachine) -> Result<()> {
        // Set hostname
        self.set_hostname(&target.hostname).await?;

        // Configure network
        self.configure_network(&target.network_config).await?;

        // Setup SSH keys
        self.setup_ssh_keys(&target.ssh_keys).await?;

        // Set timezone
        self.set_timezone(&target.timezone).await?;

        // Configure LUKS/encryption settings
        self.configure_luks(&target.luks_config).await?;

        Ok(())
    }

    async fn apply_template(&self, template: &CustomizationTemplate, target: &TargetMachine) -> Result<()> {
        println!("Applying customization template: {}", template.name);

        // Apply file templates
        for file_template in &template.files {
            self.apply_file_template(file_template, target).await?;
        }

        // Execute commands
        for command_template in &template.commands {
            self.execute_command_template(command_template).await?;
        }

        // Install packages
        if !template.packages.is_empty() {
            self.install_packages(&template.packages).await?;
        }

        Ok(())
    }

    async fn set_hostname(&self, hostname: &str) -> Result<()> {
        let hostname_path = self.mount_point.join("etc/hostname");
        fs::write(&hostname_path, format!("{}\n", hostname)).await
            .context("Failed to set hostname")?;

        let hosts_path = self.mount_point.join("etc/hosts");
        let hosts_content = self.generate_hosts_file(hostname);
        fs::write(&hosts_path, hosts_content).await
            .context("Failed to update hosts file")?;

        Ok(())
    }

    fn generate_hosts_file(&self, hostname: &str) -> String {
        format!(
            r#"127.0.0.1       localhost
127.0.1.1       {hostname}

# The following lines are desirable for IPv6 capable hosts
::1             localhost ip6-localhost ip6-loopback
ff02::1         ip6-allnodes
ff02::2         ip6-allrouters
"#,
            hostname = hostname
        )
    }

    async fn configure_network(&self, network_config: &NetworkConfig) -> Result<()> {
        let netplan_dir = self.mount_point.join("etc/netplan");
        fs::create_dir_all(&netplan_dir).await?;

        let netplan_config = self.generate_netplan_config(network_config);
        let netplan_path = netplan_dir.join("01-netcfg.yaml");
        fs::write(&netplan_path, netplan_config).await
            .context("Failed to write netplan configuration")?;

        Ok(())
    }

    fn generate_netplan_config(&self, config: &NetworkConfig) -> String {
        format!(
            r#"network:
  version: 2
  ethernets:
    {interface}:
      addresses:
        - {address}
      routes:
        - to: default
          via: {gateway}
      nameservers:
        addresses: [{dns_servers}]
"#,
            interface = config.interface,
            address = config.address,
            gateway = config.gateway,
            dns_servers = config.dns_servers
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    async fn setup_ssh_keys(&self, ssh_keys: &[String]) -> Result<()> {
        if ssh_keys.is_empty() {
            return Ok(());
        }

        let ssh_dir = self.mount_point.join("root/.ssh");
        fs::create_dir_all(&ssh_dir).await?;

        let authorized_keys_path = ssh_dir.join("authorized_keys");
        let keys_content = ssh_keys.join("\n") + "\n";
        fs::write(&authorized_keys_path, keys_content).await
            .context("Failed to write SSH keys")?;

        // Set proper permissions
        self.set_file_permissions(&ssh_dir, "700").await?;
        self.set_file_permissions(&authorized_keys_path, "600").await?;

        Ok(())
    }

    async fn set_timezone(&self, timezone: &str) -> Result<()> {
        let localtime_path = self.mount_point.join("etc/localtime");
        let zoneinfo_path = format!("/usr/share/zoneinfo/{}", timezone);

        // Remove existing symlink
        if localtime_path.exists() {
            fs::remove_file(&localtime_path).await?;
        }

        // Create new symlink
        tokio::fs::symlink(&zoneinfo_path, &localtime_path).await
            .context("Failed to set timezone symlink")?;

        let timezone_path = self.mount_point.join("etc/timezone");
        fs::write(&timezone_path, format!("{}\n", timezone)).await
            .context("Failed to write timezone file")?;

        Ok(())
    }

    async fn configure_luks(&self, luks_config: &super::LuksConfig) -> Result<()> {
        // Create crypttab entry for automatic LUKS unlocking
        let crypttab_path = self.mount_point.join("etc/crypttab");
        let crypttab_content = format!(
            "# <target name> <source device> <key file> <options>\nroot_crypt UUID=PLACEHOLDER none luks\n"
        );
        fs::write(&crypttab_path, crypttab_content).await
            .context("Failed to write crypttab")?;

        // Update initramfs configuration for LUKS
        let initramfs_modules_path = self.mount_point.join("etc/initramfs-tools/modules");
        let modules_content = "# List of modules to include in initramfs for LUKS\naes\nsha256\ndm-crypt\n";
        fs::write(&initramfs_modules_path, modules_content).await
            .context("Failed to write initramfs modules")?;

        Ok(())
    }

    async fn apply_file_template(&self, template: &FileTemplate, target: &TargetMachine) -> Result<()> {
        let file_path = self.mount_point.join(template.path.trim_start_matches('/'));

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Process template variables
        let content = self.process_template_variables(&template.content, target);

        fs::write(&file_path, content).await
            .with_context(|| format!("Failed to write file: {:?}", file_path))?;

        // Set permissions if specified
        if let Some(permissions) = &template.permissions {
            self.set_file_permissions(&file_path, permissions).await?;
        }

        // Set owner if specified
        if let Some(owner) = &template.owner {
            self.set_file_owner(&file_path, owner).await?;
        }

        Ok(())
    }

    fn process_template_variables(&self, content: &str, target: &TargetMachine) -> String {
        content
            .replace("{{hostname}}", &target.hostname)
            .replace("{{architecture}}", target.architecture.as_str())
            .replace("{{disk_device}}", &target.disk_device)
            .replace("{{timezone}}", &target.timezone)
            .replace("{{interface}}", &target.network_config.interface)
            .replace("{{address}}", &target.network_config.address)
            .replace("{{gateway}}", &target.network_config.gateway)
    }

    async fn execute_command_template(&self, template: &CommandTemplate) -> Result<()> {
        println!("Executing: {}", template.description);

        if template.run_in_chroot {
            self.execute_in_chroot(&template.command).await?;
        } else {
            self.execute_command(&template.command).await?;
        }

        Ok(())
    }

    async fn execute_in_chroot(&self, command: &str) -> Result<()> {
        let chroot_command = format!(
            "chroot {} /bin/bash -c '{}'",
            self.mount_point.display(),
            command.replace('\'', "'\"'\"'")
        );

        self.execute_command(&chroot_command).await
    }

    async fn execute_command(&self, command: &str) -> Result<()> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .context("Failed to execute command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Command failed: {}\nStdout: {}\nStderr: {}",
                command,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    async fn install_packages(&self, packages: &[String]) -> Result<()> {
        let package_list = packages.join(" ");
        let install_command = format!(
            "DEBIAN_FRONTEND=noninteractive apt-get update && apt-get install -y {}",
            package_list
        );

        self.execute_in_chroot(&install_command).await
            .context("Failed to install packages")?;

        Ok(())
    }

    async fn set_file_permissions(&self, path: &PathBuf, permissions: &str) -> Result<()> {
        let output = tokio::process::Command::new("chmod")
            .args(&[permissions, path.to_str().unwrap()])
            .output()
            .await
            .context("Failed to set file permissions")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set permissions {} on {:?}",
                permissions,
                path
            ));
        }

        Ok(())
    }

    async fn set_file_owner(&self, path: &PathBuf, owner: &str) -> Result<()> {
        let output = tokio::process::Command::new("chown")
            .args(&[owner, path.to_str().unwrap()])
            .output()
            .await
            .context("Failed to set file owner")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set owner {} on {:?}",
                owner,
                path
            ));
        }

        Ok(())
    }
}
