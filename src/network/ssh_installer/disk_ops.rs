// file: src/network/ssh_installer/disk_ops.rs
// version: 1.0.0
// guid: sshdisk1-2345-6789-abcd-ef0123456789

//! Disk operations for SSH installation

use tracing::info;
use crate::network::SshClient;
use crate::Result;
use super::config::InstallationConfig;

pub struct DiskManager<'a> {
    ssh: &'a mut SshClient,
}

impl<'a> DiskManager<'a> {
    pub fn new(ssh: &'a mut SshClient) -> Self {
        Self { ssh }
    }

    /// Perform complete disk preparation and partitioning
    pub async fn prepare_disk(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Starting disk preparation for {}", config.disk_device);

        // Clean up any existing mounts first
        self.cleanup_existing_mounts(config).await?;

        // Destroy existing ZFS pools
        self.destroy_existing_zfs_pools().await?;

        // Wipe and partition disk
        self.wipe_disk(config).await?;
        self.create_partitions(config).await?;
        self.format_partitions(config).await?;
        self.setup_luks_encryption(config).await?;

        info!("Disk preparation completed successfully");
        Ok(())
    }

    /// Clean up existing mounts and filesystem structures
    async fn cleanup_existing_mounts(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Cleaning up existing mounts and filesystems");

        // Unmount any existing mounts on the target disk
        let mounted_parts = self.ssh.execute_with_output(&format!(
            "mount | grep '{}' | awk '{{print $1}}' || true",
            config.disk_device
        )).await?;

        for mount in mounted_parts.lines() {
            if !mount.trim().is_empty() {
                self.log_and_execute(
                    &format!("Unmounting {}", mount.trim()),
                    &format!("umount -f {} || true", mount.trim())
                ).await?;
            }
        }

        // Close any open LUKS devices
        self.log_and_execute("Closing LUKS devices", "cryptsetup close luks || true").await?;

        Ok(())
    }

    /// Destroy existing ZFS pools
    async fn destroy_existing_zfs_pools(&mut self) -> Result<()> {
        info!("Destroying existing ZFS pools");

        let existing_pools = self.ssh.execute_with_output("zpool list -H -o name 2>/dev/null || true").await?;
        if !existing_pools.trim().is_empty() {
            for pool in existing_pools.lines() {
                if !pool.trim().is_empty() {
                    self.log_and_execute(
                        &format!("Destroying ZFS pool: {}", pool.trim()),
                        &format!("zpool destroy {} || true", pool.trim())
                    ).await?;
                }
            }
        }

        Ok(())
    }

    /// Wipe the target disk completely
    async fn wipe_disk(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Wiping target disk");

        self.log_and_execute("Wiping disk signatures", &format!("wipefs -a {}", config.disk_device)).await?;
        self.log_and_execute("Discarding blocks", &format!("blkdiscard -f {} || true", config.disk_device)).await?;
        self.log_and_execute("Zapping GPT structures", &format!("sgdisk --zap-all {}", config.disk_device)).await?;

        Ok(())
    }

    /// Create disk partitions
    async fn create_partitions(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Creating disk partitions");

        // Create GPT partition table
        self.log_and_execute("Creating GPT table", &format!("parted -s {} mklabel gpt", config.disk_device)).await?;

        // ESP partition (1MiB to 513MiB)
        self.log_and_execute("Creating ESP partition",
            &format!("parted -s {} mkpart ESP fat32 1MiB 513MiB", config.disk_device)).await?;
        self.log_and_execute("Setting ESP boot flag",
            &format!("parted -s {} set 1 boot on", config.disk_device)).await?;
        self.log_and_execute("Setting ESP esp flag",
            &format!("parted -s {} set 1 esp on", config.disk_device)).await?;

        // RESET partition (513MiB to 4609MiB)
        self.log_and_execute("Creating RESET partition",
            &format!("parted -s {} mkpart RESET fat32 513MiB 4609MiB", config.disk_device)).await?;

        // BPOOL partition (4609MiB to 6657MiB)
        self.log_and_execute("Creating BPOOL partition",
            &format!("parted -s {} mkpart BPOOL 4609MiB 6657MiB", config.disk_device)).await?;

        // LUKS partition (6657MiB to 7681MiB)
        self.log_and_execute("Creating LUKS partition",
            &format!("parted -s {} mkpart LUKS 6657MiB 7681MiB", config.disk_device)).await?;

        // RPOOL partition (7681MiB to 100%)
        self.log_and_execute("Creating RPOOL partition",
            &format!("parted -s {} mkpart RPOOL 7681MiB 100%", config.disk_device)).await?;

        Ok(())
    }

    /// Format partitions
    async fn format_partitions(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Formatting partitions");

        // Format ESP and RESET partitions
        self.log_and_execute("Formatting ESP", &format!("mkfs.fat -F32 {}p1", config.disk_device)).await?;
        self.log_and_execute("Formatting RESET", &format!("mkfs.fat -F32 {}p2", config.disk_device)).await?;

        Ok(())
    }

    /// Setup LUKS encryption
    async fn setup_luks_encryption(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up LUKS encryption");

        // Setup LUKS encryption
        self.log_and_execute("Setting up LUKS encryption",
            &format!("echo '{}' | cryptsetup luksFormat --batch-mode {}p4", config.luks_key, config.disk_device)).await?;
        self.log_and_execute("Opening LUKS device",
            &format!("echo '{}' | cryptsetup open {}p4 luks", config.luks_key, config.disk_device)).await?;
        self.log_and_execute("Creating XFS on LUKS", "mkfs.xfs -f -b size=4096 /dev/mapper/luks").await?;

        Ok(())
    }

    /// Helper method to log and execute commands
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);
        self.ssh.execute(command).await
    }
}
