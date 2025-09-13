// file: src/network/ssh_installer/disk_ops.rs
// version: 1.2.0
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

    /// Perform a robust recovery cleanup and wipe in case of prior failures
    ///
    /// This will:
    /// - Unmount chroot bind mounts and anything under /mnt/targetos
    /// - Unmount /mnt/luks if mounted
    /// - Unmount ZFS filesystems and export/destroy pools (best-effort)
    /// - Close any open LUKS mapper devices
    /// - Wipe the disk GPT and FS signatures
    pub async fn recover_after_failure_and_wipe(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Recovery: cleaning up mounts, closing LUKS, exporting ZFS, and wiping disk");

        // 1) Unmount common chroot bind mounts and EFI if present
        let _ = self.log_and_execute("Recovery: umount /mnt/targetos/sys", "umount -lf /mnt/targetos/sys 2>/dev/null || true").await;
        let _ = self.log_and_execute("Recovery: umount /mnt/targetos/proc", "umount -lf /mnt/targetos/proc 2>/dev/null || true").await;
        let _ = self.log_and_execute("Recovery: umount /mnt/targetos/dev", "umount -lf /mnt/targetos/dev 2>/dev/null || true").await;
        let _ = self.log_and_execute("Recovery: umount /mnt/targetos/boot/efi", "umount -lf /mnt/targetos/boot/efi 2>/dev/null || true").await;

        // 2) Unmount anything still mounted under /mnt/targetos (deepest-first)
        let _ = self.log_and_execute(
            "Recovery: unmount all under /mnt/targetos",
            "mount | awk '$3 ~ /^\\/mnt\\/targetos/ {print $3}' | sort -r | xargs -r -n1 umount -lf 2>/dev/null || true"
        ).await;

        // 3) Unmount ZFS filesystems and export pools (best-effort)
        let _ = self.log_and_execute("Recovery: zfs unmount -a", "zfs unmount -a 2>/dev/null || true").await;
        let _ = self.log_and_execute("Recovery: zpool export -a", "zpool export -a 2>/dev/null || true").await;

        // As an extra measure, try to destroy common pools if they linger
        let _ = self.log_and_execute("Recovery: destroy bpool", "zpool destroy bpool 2>/dev/null || true").await;
        let _ = self.log_and_execute("Recovery: destroy rpool", "zpool destroy rpool 2>/dev/null || true").await;

        // 4) Unmount /mnt/luks if mounted
        let _ = self.log_and_execute(
            "Recovery: unmount /mnt/luks if mounted",
            "mountpoint -q /mnt/luks && umount -lf /mnt/luks || true"
        ).await;

        // 5) Close LUKS mapper devices
        // Try the known name first, then any crypt devices discovered under /dev/mapper
        let _ = self.log_and_execute("Recovery: close luks", "cryptsetup close luks 2>/dev/null || true").await;
        let _ = self.log_and_execute(
            "Recovery: close any crypt mappers",
            "for m in $(ls /dev/mapper 2>/dev/null | grep -E '^(luks|crypt)' || true); do cryptsetup close \"$m\" 2>/dev/null || true; done"
        ).await;

        // 6) Finally wipe the disk and GPT
        self.wipe_disk(config).await?;

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

        // Also unmount /mnt/luks if it is mounted (best-effort)
        let _ = self.log_and_execute("Unmount /mnt/luks if mounted", "mountpoint -q /mnt/luks && umount -lf /mnt/luks || true").await;

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

        // LUKS partition (6657MiB to 100%)
        // We use the LUKS-mapped device as the sole vdev for rpool; no separate RPOOL partition.
        self.log_and_execute("Creating LUKS partition",
            &format!("parted -s {} mkpart LUKS 6657MiB 100%", config.disk_device)).await?;

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
        // Do not create a filesystem on the LUKS-mapped device; it will back the ZFS rpool.

        Ok(())
    }

    /// Helper method to log and execute commands
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);
        self.ssh.execute(command).await
    }
}
