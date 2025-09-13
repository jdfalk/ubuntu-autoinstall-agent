// file: src/network/ssh_installer/zfs_ops.rs
// version: 1.1.0
// guid: sshzfs01-2345-6789-abcd-ef0123456789

//! ZFS operations for SSH installation

use std::collections::HashMap;
use tracing::{info, error};
use crate::network::SshClient;
use crate::Result;
use super::config::InstallationConfig;

pub struct ZfsManager<'a> {
    ssh: &'a mut SshClient,
    variables: &'a mut HashMap<String, String>,
}

impl<'a> ZfsManager<'a> {
    pub fn new(ssh: &'a mut SshClient, variables: &'a mut HashMap<String, String>) -> Self {
        Self { ssh, variables }
    }

    /// Create ZFS pools and datasets
    pub async fn create_zfs_pools(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Creating ZFS pools and datasets");

        // Prepare ZFS key storage
        self.prepare_zfs_key_storage(config).await?;

        // Generate UUID for dataset naming
        let uuid = self.generate_installation_uuid().await?;
        self.variables.insert("UUID".to_string(), uuid.clone());

        // Create bpool
        self.create_bpool(config).await?;

        // Create rpool with encryption
        self.create_rpool(config).await?;

        // Create bpool datasets
        self.create_bpool_datasets(&uuid).await?;

        // Create rpool datasets
        self.create_rpool_datasets(&uuid).await?;

        info!("ZFS pools and datasets created successfully");
        Ok(())
    }

    /// Prepare ZFS key storage in LUKS
    async fn prepare_zfs_key_storage(&mut self, _config: &InstallationConfig) -> Result<()> {
        info!("Preparing ZFS key storage");

        self.log_and_execute("Creating /mnt/luks", "mkdir -p /mnt/luks").await?;
        self.log_and_execute("Mounting LUKS", "mount /dev/mapper/luks /mnt/luks").await?;
        self.log_and_execute("Generating ZFS key", "dd if=/dev/random of=/mnt/luks/zfs.key bs=32 count=1").await?;
        self.log_and_execute("Setting ZFS key permissions", "chmod 600 /mnt/luks/zfs.key").await?;
        // Don't unmount or close LUKS yet - we need it for rpool creation
        self.log_and_execute("Creating target directory", "mkdir -p /mnt/targetos").await?;

        Ok(())
    }

    /// Generate unique UUID for this installation
    async fn generate_installation_uuid(&mut self) -> Result<String> {
        let uuid_output = self.ssh.execute_with_output("dd if=/dev/urandom bs=1 count=100 2>/dev/null | tr -dc 'a-z0-9' | cut -c-6").await?;
        let uuid = uuid_output.trim().to_string();

        // Write UUID to target
        self.ssh.execute(&format!("echo 'UUID={}' > /mnt/targetos/uuid", uuid)).await?;
        self.ssh.execute(&format!("echo 'DISK={}' >> /mnt/targetos/uuid", self.variables.get("DISK").unwrap_or(&"unknown".to_string()))).await?;

        info!("Generated installation UUID: {}", uuid);
        Ok(uuid)
    }

    /// Create bpool (boot pool)
    async fn create_bpool(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Creating bpool");

        let bpool_cmd = format!(
            "zpool create -o ashift=12 -o autotrim=on -o cachefile=/etc/zfs/zpool.cache \
             -o compatibility=grub2 -o feature@livelist=enabled -o feature@zpool_checkpoint=enabled \
             -O devices=off -O acltype=posixacl -O xattr=sa -O compression=lz4 \
             -O normalization=formD -O relatime=on -O canmount=off -O mountpoint=none \
             -m none -R /mnt/targetos bpool {}p3", config.disk_device
        );
        self.log_and_execute("Creating bpool", &bpool_cmd).await?;

        Ok(())
    }

    /// Create rpool (root pool) with encryption
    async fn create_rpool(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Creating rpool with encryption");

        // LUKS device should already be open from prepare_zfs_key_storage
        // Just ensure the mount point exists and is mounted
        self.log_and_execute("Ensuring LUKS is mounted", "mount /dev/mapper/luks /mnt/luks 2>/dev/null || true").await?;

        let rpool_cmd = format!(
            "zpool create -o ashift=12 -o autotrim=on \
             -O encryption=on -O keylocation=file:///mnt/luks/zfs.key -O keyformat=raw \
             -O acltype=posixacl -O xattr=sa -O dnodesize=auto -O compression=lz4 \
             -O normalization=formD -O relatime=on -O canmount=off -O mountpoint=none \
             -m none -R /mnt/targetos rpool {}p5", config.disk_device
        );
        self.log_and_execute("Creating rpool", &rpool_cmd).await?;

        Ok(())
    }

    /// Create bpool datasets
    async fn create_bpool_datasets(&mut self, uuid: &str) -> Result<()> {
        info!("Creating bpool datasets");

        self.log_and_execute("Creating bpool/BOOT", "zfs create -o canmount=off -o mountpoint=none bpool/BOOT").await?;
        self.log_and_execute("Creating bpool boot dataset",
            &format!("zfs create -o mountpoint=/boot bpool/BOOT/ubuntu_{}", uuid)).await?;

        Ok(())
    }

    /// Create comprehensive rpool dataset structure
    async fn create_rpool_datasets(&mut self, uuid: &str) -> Result<()> {
        info!("Creating rpool dataset structure");

        // Root dataset structure
        self.log_and_execute("Creating rpool/ROOT", "zfs create -o canmount=off -o mountpoint=none rpool/ROOT").await?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_and_execute("Creating root filesystem",
            &format!("zfs create -o mountpoint=/ -o com.ubuntu.zsys:bootfs=yes -o com.ubuntu.zsys:last-used={} rpool/ROOT/ubuntu_{}", current_time, uuid)).await?;

        // System directories
        let datasets = vec![
            ("usr", "rpool/ROOT/ubuntu_{}/usr", "-o com.ubuntu.zsys:bootfs=no -o canmount=off"),
            ("var", "rpool/ROOT/ubuntu_{}/var", "-o com.ubuntu.zsys:bootfs=no -o canmount=off"),
            ("var/lib", "rpool/ROOT/ubuntu_{}/var/lib", ""),
            ("var/log", "rpool/ROOT/ubuntu_{}/var/log", ""),
            ("var/spool", "rpool/ROOT/ubuntu_{}/var/spool", ""),
            ("var/cache", "rpool/ROOT/ubuntu_{}/var/cache", ""),
            ("var/lib/nfs", "rpool/ROOT/ubuntu_{}/var/lib/nfs", ""),
            ("var/tmp", "rpool/ROOT/ubuntu_{}/var/tmp", ""),
            ("var/lib/apt", "rpool/ROOT/ubuntu_{}/var/lib/apt", ""),
            ("var/lib/dpkg", "rpool/ROOT/ubuntu_{}/var/lib/dpkg", ""),
            ("srv", "rpool/ROOT/ubuntu_{}/srv", "-o com.ubuntu.zsys:bootfs=no"),
            ("usr/local", "rpool/ROOT/ubuntu_{}/usr/local", ""),
            ("var/games", "rpool/ROOT/ubuntu_{}/var/games", ""),
            ("var/lib/AccountsService", "rpool/ROOT/ubuntu_{}/var/lib/AccountsService", ""),
        ];

        for (name, dataset, opts) in datasets {
            let dataset_name = dataset.replace("{}", uuid);
            self.log_and_execute(
                &format!("Creating {}", name),
                &format!("zfs create {} {}", opts, dataset_name)
            ).await?;
        }

        // Set special permissions
        self.log_and_execute("Setting /root permissions", "chmod 700 /mnt/targetos/root").await?;
        self.log_and_execute("Setting /var/tmp permissions", "chmod 1777 /mnt/targetos/var/tmp").await?;

        // Create USERDATA structure
        self.log_and_execute("Creating USERDATA", "zfs create -o canmount=off -o mountpoint=/ rpool/USERDATA").await?;
        self.log_and_execute("Creating root user data",
            &format!("zfs create -o com.ubuntu.zsys:bootfs-datasets=rpool/ROOT/ubuntu_{} -o canmount=on -o mountpoint=/root rpool/USERDATA/root_{}", uuid, uuid)).await?;

        Ok(())
    }

    /// Helper method to log and execute commands with better error handling
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);

        match self.ssh.execute_with_error_collection(command, description).await {
            Ok((exit_code, stdout, stderr)) => {
                if exit_code == 0 {
                    if !stdout.is_empty() {
                        info!("Command output: {}", stdout.trim());
                    }
                    Ok(())
                } else {
                    error!("Command '{}' failed with exit code {}", description, exit_code);
                    error!("STDOUT: {}", stdout);
                    error!("STDERR: {}", stderr);

                    // Don't immediately fail - collect debug info
                    if let Ok(debug_info) = self.ssh.collect_debug_info().await {
                        error!("System debug information:\n{}", debug_info);
                    }

                    Err(crate::error::AutoInstallError::SshError(
                        format!("Command '{}' failed with exit code {}: stderr={}", description, exit_code, stderr)
                    ))
                }
            }
            Err(e) => {
                error!("Failed to execute command '{}': {}", description, e);

                // Try to collect debug info even if the command completely failed
                if let Ok(debug_info) = self.ssh.collect_debug_info().await {
                    error!("System debug information:\n{}", debug_info);
                }

                Err(e)
            }
        }
    }
}
