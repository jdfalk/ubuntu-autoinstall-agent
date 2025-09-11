// file: src/image/builder/disk.rs
// version: 1.0.0
// guid: b1b2b3b4-c5c6-7890-1234-567890bcdefg

//! Disk creation and management functionality

use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::debug;
use crate::Result;

/// Disk management operations
pub struct DiskManager {
    work_dir: PathBuf,
}

impl DiskManager {
    /// Create a new disk manager
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Create QEMU disk image
    pub async fn create_qemu_disk(&self, disk_path: &Path, size_gb: u32) -> Result<()> {
        let output = Command::new("qemu-img")
            .args(&[
                "create",
                "-f", "qcow2",
                disk_path.to_str().unwrap(),
                &format!("{}G", size_gb),
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to create QEMU disk: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::VmError(
                format!("qemu-img failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        debug!("Created QEMU disk: {}", disk_path.display());
        Ok(())
    }

    /// Get VM disk path
    pub fn get_vm_disk_path(&self) -> PathBuf {
        self.work_dir.join("ubuntu-install.qcow2")
    }
}
