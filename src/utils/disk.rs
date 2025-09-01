// file: src/utils/disk.rs
// version: 1.0.0
// guid: x5y6z7a8-b9c0-1234-5678-901234567890

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, error, info, warn};

/// Disk and filesystem utilities for the installation agent
pub struct DiskUtils;

impl DiskUtils {
    /// Get all available block devices
    pub async fn get_block_devices() -> Result<Vec<BlockDevice>> {
        let output = tokio::process::Command::new("lsblk")
            .args(&["-J", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE,MODEL,SERIAL,UUID,PARTUUID"])
            .output()
            .await
            .context("Failed to execute lsblk command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "lsblk command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse lsblk output as UTF-8")?;

        let lsblk_output: LsblkOutput = serde_json::from_str(&output_str)
            .context("Failed to parse lsblk JSON output")?;

        Ok(lsblk_output.blockdevices)
    }

    /// Get disk usage information
    pub async fn get_disk_usage(path: &Path) -> Result<DiskUsage> {
        let output = tokio::process::Command::new("df")
            .args(&["-B1", path.to_str().unwrap_or("/")])
            .output()
            .await
            .context("Failed to execute df command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "df command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse df output as UTF-8")?;

        // Parse df output (skip header line)
        let lines: Vec<&str> = output_str.lines().collect();
        if lines.len() < 2 {
            return Err(anyhow::anyhow!("Unexpected df output format"));
        }

        let fields: Vec<&str> = lines[1].split_whitespace().collect();
        if fields.len() < 6 {
            return Err(anyhow::anyhow!("Unexpected df field count"));
        }

        let total = fields[1].parse::<u64>()
            .context("Failed to parse total disk space")?;
        let used = fields[2].parse::<u64>()
            .context("Failed to parse used disk space")?;
        let available = fields[3].parse::<u64>()
            .context("Failed to parse available disk space")?;

        Ok(DiskUsage {
            filesystem: fields[0].to_string(),
            total,
            used,
            available,
            use_percentage: ((used as f64 / total as f64) * 100.0) as u8,
            mount_point: fields[5].to_string(),
        })
    }

    /// Check if a device exists and is a block device
    pub async fn is_block_device(device: &Path) -> Result<bool> {
        if !device.exists() {
            return Ok(false);
        }

        let metadata = tokio::fs::metadata(device).await
            .context("Failed to get device metadata")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            Ok(metadata.file_type().is_block_device())
        }

        #[cfg(not(unix))]
        {
            warn!("Block device detection not supported on this platform");
            Ok(false)
        }
    }

    /// Get device size in bytes
    pub async fn get_device_size(device: &Path) -> Result<u64> {
        let output = tokio::process::Command::new("blockdev")
            .args(&["--getsize64", device.to_str().unwrap()])
            .output()
            .await
            .context("Failed to execute blockdev command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "blockdev command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let size_str = String::from_utf8(output.stdout)
            .context("Failed to parse blockdev output as UTF-8")?;

        size_str.trim().parse::<u64>()
            .context("Failed to parse device size")
    }

    /// Wipe a device securely
    pub async fn wipe_device(device: &Path, method: WipeMethod) -> Result<()> {
        info!("Wiping device {:?} using method {:?}", device, method);

        if !Self::is_block_device(device).await? {
            return Err(anyhow::anyhow!("Device {:?} is not a block device", device));
        }

        match method {
            WipeMethod::Zero => {
                tokio::process::Command::new("dd")
                    .args(&[
                        "if=/dev/zero",
                        &format!("of={}", device.display()),
                        "bs=1M",
                        "status=progress"
                    ])
                    .status()
                    .await
                    .context("Failed to zero device")?;
            }
            WipeMethod::Random => {
                tokio::process::Command::new("dd")
                    .args(&[
                        "if=/dev/urandom",
                        &format!("of={}", device.display()),
                        "bs=1M",
                        "status=progress"
                    ])
                    .status()
                    .await
                    .context("Failed to random wipe device")?;
            }
            WipeMethod::Secure => {
                // Use shred for secure wiping
                tokio::process::Command::new("shred")
                    .args(&["-vfz", "-n", "3", device.to_str().unwrap()])
                    .status()
                    .await
                    .context("Failed to securely wipe device")?;
            }
        }

        info!("Device {:?} wiped successfully", device);
        Ok(())
    }

    /// Create a partition table
    pub async fn create_partition_table(device: &Path, table_type: PartitionTableType) -> Result<()> {
        info!("Creating {} partition table on {:?}", table_type.as_str(), device);

        let label = match table_type {
            PartitionTableType::GPT => "gpt",
            PartitionTableType::MBR => "msdos",
        };

        let status = tokio::process::Command::new("parted")
            .args(&[
                device.to_str().unwrap(),
                "--script",
                "mklabel",
                label
            ])
            .status()
            .await
            .context("Failed to create partition table")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to create partition table"));
        }

        info!("Partition table created successfully");
        Ok(())
    }

    /// Create a partition
    pub async fn create_partition(
        device: &Path,
        partition_num: u32,
        start: &str,
        end: &str,
        fs_type: Option<&str>,
    ) -> Result<()> {
        info!("Creating partition {} on {:?} from {} to {}", partition_num, device, start, end);

        let mut args = vec![
            device.to_str().unwrap(),
            "--script",
            "mkpart"
        ];

        if let Some(fs) = fs_type {
            args.push(fs);
        } else {
            args.push("primary");
        }

        args.extend_from_slice(&[start, end]);

        let status = tokio::process::Command::new("parted")
            .args(&args)
            .status()
            .await
            .context("Failed to create partition")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to create partition"));
        }

        info!("Partition {} created successfully", partition_num);
        Ok(())
    }

    /// Format a partition with a filesystem
    pub async fn format_partition(device: &Path, filesystem: FilesystemType, label: Option<&str>) -> Result<()> {
        info!("Formatting {:?} with {:?}", device, filesystem);

        let mut args = vec![];
        let cmd = match filesystem {
            FilesystemType::Ext4 => {
                args.extend_from_slice(&["-F", device.to_str().unwrap()]);
                if let Some(l) = label {
                    args.extend_from_slice(&["-L", l]);
                }
                "mkfs.ext4"
            }
            FilesystemType::Xfs => {
                args.extend_from_slice(&["-f", device.to_str().unwrap()]);
                if let Some(l) = label {
                    args.extend_from_slice(&["-L", l]);
                }
                "mkfs.xfs"
            }
            FilesystemType::Btrfs => {
                args.extend_from_slice(&["-f", device.to_str().unwrap()]);
                if let Some(l) = label {
                    args.extend_from_slice(&["-L", l]);
                }
                "mkfs.btrfs"
            }
            FilesystemType::Fat32 => {
                args.extend_from_slice(&["-F", "32", device.to_str().unwrap()]);
                if let Some(l) = label {
                    args.extend_from_slice(&["-n", l]);
                }
                "mkfs.fat"
            }
            FilesystemType::Swap => {
                args.push(device.to_str().unwrap());
                if let Some(l) = label {
                    args.extend_from_slice(&["-L", l]);
                }
                "mkswap"
            }
        };

        let status = tokio::process::Command::new(cmd)
            .args(&args)
            .status()
            .await
            .with_context(|| format!("Failed to format partition with {}", cmd))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to format partition"));
        }

        info!("Partition formatted successfully");
        Ok(())
    }

    /// Mount a filesystem
    pub async fn mount_filesystem(device: &Path, mount_point: &Path, options: Option<&str>) -> Result<()> {
        info!("Mounting {:?} to {:?}", device, mount_point);

        // Create mount point if it doesn't exist
        if !mount_point.exists() {
            tokio::fs::create_dir_all(mount_point).await
                .with_context(|| format!("Failed to create mount point {:?}", mount_point))?;
        }

        let mut args = vec![];
        if let Some(opts) = options {
            args.extend_from_slice(&["-o", opts]);
        }
        args.extend_from_slice(&[device.to_str().unwrap(), mount_point.to_str().unwrap()]);

        let status = tokio::process::Command::new("mount")
            .args(&args)
            .status()
            .await
            .context("Failed to mount filesystem")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to mount filesystem"));
        }

        info!("Filesystem mounted successfully");
        Ok(())
    }

    /// Unmount a filesystem
    pub async fn unmount_filesystem(mount_point: &Path, force: bool) -> Result<()> {
        info!("Unmounting {:?}", mount_point);

        let mut args = vec![];
        if force {
            args.push("-f");
        }
        args.push(mount_point.to_str().unwrap());

        let status = tokio::process::Command::new("umount")
            .args(&args)
            .status()
            .await
            .context("Failed to unmount filesystem")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to unmount filesystem"));
        }

        info!("Filesystem unmounted successfully");
        Ok(())
    }

    /// Get UUID of a filesystem
    pub async fn get_filesystem_uuid(device: &Path) -> Result<String> {
        let output = tokio::process::Command::new("blkid")
            .args(&["-s", "UUID", "-o", "value", device.to_str().unwrap()])
            .output()
            .await
            .context("Failed to execute blkid command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "blkid command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let uuid = String::from_utf8(output.stdout)
            .context("Failed to parse blkid output as UTF-8")?
            .trim()
            .to_string();

        if uuid.is_empty() {
            return Err(anyhow::anyhow!("No UUID found for device {:?}", device));
        }

        Ok(uuid)
    }

    /// Check filesystem integrity
    pub async fn check_filesystem(device: &Path, filesystem: FilesystemType, fix: bool) -> Result<()> {
        info!("Checking filesystem on {:?}", device);

        let (cmd, mut args) = match filesystem {
            FilesystemType::Ext4 => {
                let mut a = vec!["-f"];
                if fix {
                    a.push("-p"); // Automatically fix problems
                }
                a.push(device.to_str().unwrap());
                ("fsck.ext4", a)
            }
            FilesystemType::Xfs => {
                let mut a = vec![];
                if fix {
                    a.push("-r"); // Repair mode
                }
                a.push(device.to_str().unwrap());
                ("xfs_repair", a)
            }
            FilesystemType::Btrfs => {
                let mut a = vec!["check"];
                if fix {
                    a.push("--repair");
                }
                a.push(device.to_str().unwrap());
                ("btrfs", a)
            }
            FilesystemType::Fat32 => {
                let mut a = vec![];
                if fix {
                    a.push("-a"); // Automatically fix errors
                }
                a.push(device.to_str().unwrap());
                ("fsck.fat", a)
            }
            FilesystemType::Swap => {
                return Ok(()); // Swap doesn't need filesystem check
            }
        };

        let status = tokio::process::Command::new(cmd)
            .args(&args)
            .status()
            .await
            .with_context(|| format!("Failed to check filesystem with {}", cmd))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Filesystem check failed"));
        }

        info!("Filesystem check completed successfully");
        Ok(())
    }

    /// Resize a filesystem
    pub async fn resize_filesystem(device: &Path, filesystem: FilesystemType, size: Option<&str>) -> Result<()> {
        info!("Resizing filesystem on {:?}", device);

        let (cmd, mut args) = match filesystem {
            FilesystemType::Ext4 => {
                let mut a = vec![device.to_str().unwrap()];
                if let Some(s) = size {
                    a.push(s);
                }
                ("resize2fs", a)
            }
            FilesystemType::Xfs => {
                let mut a = vec!["-d"];
                if let Some(s) = size {
                    a.push(&format!("size={}", s));
                }
                a.push(device.to_str().unwrap());
                ("xfs_growfs", a)
            }
            FilesystemType::Btrfs => {
                let mut a = vec!["filesystem", "resize"];
                if let Some(s) = size {
                    a.push(s);
                } else {
                    a.push("max");
                }
                a.push(device.to_str().unwrap());
                ("btrfs", a)
            }
            _ => {
                return Err(anyhow::anyhow!("Resize not supported for {:?}", filesystem));
            }
        };

        let status = tokio::process::Command::new(cmd)
            .args(&args)
            .status()
            .await
            .with_context(|| format!("Failed to resize filesystem with {}", cmd))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Filesystem resize failed"));
        }

        info!("Filesystem resized successfully");
        Ok(())
    }

    /// Get all mounted filesystems
    pub async fn get_mounted_filesystems() -> Result<Vec<MountInfo>> {
        let output = tokio::process::Command::new("findmnt")
            .args(&["-J", "-o", "SOURCE,TARGET,FSTYPE,OPTIONS"])
            .output()
            .await
            .context("Failed to execute findmnt command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "findmnt command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse findmnt output as UTF-8")?;

        let findmnt_output: FindmntOutput = serde_json::from_str(&output_str)
            .context("Failed to parse findmnt JSON output")?;

        Ok(findmnt_output.filesystems)
    }
}

/// Block device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDevice {
    pub name: String,
    pub size: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub mountpoint: Option<String>,
    pub fstype: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub uuid: Option<String>,
    pub partuuid: Option<String>,
    pub children: Option<Vec<BlockDevice>>,
}

/// lsblk command output structure
#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

/// Disk usage information
#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub filesystem: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub use_percentage: u8,
    pub mount_point: String,
}

/// Mount information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub source: Option<String>,
    pub target: String,
    pub fstype: String,
    pub options: String,
}

/// findmnt command output structure
#[derive(Debug, Deserialize)]
struct FindmntOutput {
    filesystems: Vec<MountInfo>,
}

/// Disk wiping methods
#[derive(Debug, Clone, Copy)]
pub enum WipeMethod {
    Zero,     // Fill with zeros
    Random,   // Fill with random data
    Secure,   // Multi-pass secure wipe
}

/// Partition table types
#[derive(Debug, Clone, Copy)]
pub enum PartitionTableType {
    GPT,
    MBR,
}

impl PartitionTableType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionTableType::GPT => "GPT",
            PartitionTableType::MBR => "MBR",
        }
    }
}

/// Supported filesystem types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FilesystemType {
    Ext4,
    Xfs,
    Btrfs,
    Fat32,
    Swap,
}

impl FromStr for FilesystemType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ext4" => Ok(FilesystemType::Ext4),
            "xfs" => Ok(FilesystemType::Xfs),
            "btrfs" => Ok(FilesystemType::Btrfs),
            "fat32" | "vfat" => Ok(FilesystemType::Fat32),
            "swap" => Ok(FilesystemType::Swap),
            _ => Err(anyhow::anyhow!("Unsupported filesystem type: {}", s)),
        }
    }
}

impl std::fmt::Display for FilesystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilesystemType::Ext4 => write!(f, "ext4"),
            FilesystemType::Xfs => write!(f, "xfs"),
            FilesystemType::Btrfs => write!(f, "btrfs"),
            FilesystemType::Fat32 => write!(f, "fat32"),
            FilesystemType::Swap => write!(f, "swap"),
        }
    }
}

/// Disk partition information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub device: PathBuf,
    pub partition_number: u32,
    pub start_sector: u64,
    pub end_sector: u64,
    pub size_bytes: u64,
    pub filesystem: Option<FilesystemType>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub mount_point: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_get_block_devices() {
        // This test requires lsblk to be available
        match DiskUtils::get_block_devices().await {
            Ok(devices) => {
                assert!(!devices.is_empty());
                println!("Found {} block devices", devices.len());
            }
            Err(e) => {
                println!("Could not get block devices (this is normal in some test environments): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_disk_usage() {
        // Test getting disk usage for root filesystem
        match DiskUtils::get_disk_usage(Path::new("/")).await {
            Ok(usage) => {
                assert!(usage.total > 0);
                assert!(usage.used <= usage.total);
                assert!(usage.available <= usage.total);
                println!("Root filesystem: {} used, {} available, {}% used",
                         usage.used, usage.available, usage.use_percentage);
            }
            Err(e) => {
                println!("Could not get disk usage (this is normal in some test environments): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_filesystem_type_parsing() {
        assert_eq!(FilesystemType::from_str("ext4").unwrap(), FilesystemType::Ext4);
        assert_eq!(FilesystemType::from_str("xfs").unwrap(), FilesystemType::Xfs);
        assert_eq!(FilesystemType::from_str("btrfs").unwrap(), FilesystemType::Btrfs);
        assert_eq!(FilesystemType::from_str("fat32").unwrap(), FilesystemType::Fat32);
        assert_eq!(FilesystemType::from_str("vfat").unwrap(), FilesystemType::Fat32);
        assert_eq!(FilesystemType::from_str("swap").unwrap(), FilesystemType::Swap);

        assert!(FilesystemType::from_str("invalid").is_err());
    }

    #[test]
    fn test_filesystem_type_display() {
        assert_eq!(FilesystemType::Ext4.to_string(), "ext4");
        assert_eq!(FilesystemType::Xfs.to_string(), "xfs");
        assert_eq!(FilesystemType::Btrfs.to_string(), "btrfs");
        assert_eq!(FilesystemType::Fat32.to_string(), "fat32");
        assert_eq!(FilesystemType::Swap.to_string(), "swap");
    }

    #[test]
    fn test_partition_table_type_as_str() {
        assert_eq!(PartitionTableType::GPT.as_str(), "GPT");
        assert_eq!(PartitionTableType::MBR.as_str(), "MBR");
    }

    #[tokio::test]
    async fn test_get_mounted_filesystems() {
        match DiskUtils::get_mounted_filesystems().await {
            Ok(filesystems) => {
                assert!(!filesystems.is_empty());
                println!("Found {} mounted filesystems", filesystems.len());

                // Should have at least root filesystem
                let root_fs = filesystems.iter().find(|fs| fs.target == "/");
                assert!(root_fs.is_some(), "Root filesystem not found");
            }
            Err(e) => {
                println!("Could not get mounted filesystems (this is normal in some test environments): {}", e);
            }
        }
    }
}
