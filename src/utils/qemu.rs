// file: src/utils/qemu.rs
// version: 1.0.2
// guid: h9i0j1k2-l3m4-5678-9012-345678hijklm

//! QEMU image utilities

use std::path::Path;
use tokio::process::Command;
use tracing::{info, debug};
use crate::Result;

/// QEMU image utilities
pub struct QemuUtils;

impl QemuUtils {
    /// Get image information using qemu-img info
    pub async fn get_image_info<P: AsRef<Path>>(image_path: P) -> Result<ImageInfo> {
        let output = Command::new("qemu-img")
            .args(["info", "--output=json", image_path.as_ref().to_str().unwrap()])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to get image info: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("qemu-img info failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        let info_str = String::from_utf8_lossy(&output.stdout);
        let info_json: serde_json::Value = serde_json::from_str(&info_str)
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to parse qemu-img output: {}", e)
            ))?;

        Ok(ImageInfo {
            format: info_json["format"].as_str().unwrap_or("unknown").to_string(),
            virtual_size: info_json["virtual-size"].as_u64().unwrap_or(0),
            actual_size: info_json["actual-size"].as_u64().unwrap_or(0),
            cluster_size: info_json["cluster-size"].as_u64(),
            compressed: info_json["compressed"].as_bool().unwrap_or(false),
        })
    }

    /// Convert image to raw format for extraction
    pub async fn convert_to_raw<P: AsRef<Path>>(
        qcow2_path: P,
        raw_path: P,
    ) -> Result<()> {
        info!("Converting QCOW2 image to raw format for extraction");

        let output = Command::new("qemu-img")
            .args([
                "convert",
                "-f", "qcow2",
                "-O", "raw",
                qcow2_path.as_ref().to_str().unwrap(),
                raw_path.as_ref().to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to convert image: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Image conversion failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        debug!("Image conversion completed");
        Ok(())
    }

    /// Mount raw image using loop device
    pub async fn mount_raw_image<P: AsRef<Path>>(
        raw_path: P,
        mount_point: P,
    ) -> Result<String> {
        // Create loop device
        let output = Command::new("losetup")
            .args(["-P", "-f", "--show", raw_path.as_ref().to_str().unwrap()])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to create loop device: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Loop device creation failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        let loop_device = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Mount the filesystem (usually partition 1)
        let partition = format!("{}p1", loop_device);
        let output = Command::new("mount")
            .args([&partition, mount_point.as_ref().to_str().unwrap()])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to mount image: {}", e)
            ))?;

        if !output.status.success() {
            // Clean up loop device on mount failure
            let _ = Command::new("losetup").args(["-d", &loop_device]).output().await;
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Mount failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        info!("Image mounted at {} via {}", mount_point.as_ref().display(), loop_device);
        Ok(loop_device)
    }

    /// Unmount image and clean up loop device
    pub async fn unmount_image<P: AsRef<Path>>(
        mount_point: P,
        loop_device: &str,
    ) -> Result<()> {
        // Unmount
        let output = Command::new("umount")
            .args([mount_point.as_ref().to_str().unwrap()])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to unmount: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Unmount failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        // Remove loop device
        let output = Command::new("losetup")
            .args(["-d", loop_device])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to remove loop device: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Loop device removal failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        debug!("Image unmounted and loop device cleaned up");
        Ok(())
    }

    /// Extract image contents to target directory
    pub async fn extract_image_contents<P: AsRef<Path>>(
        qcow2_path: P,
        target_dir: P,
    ) -> Result<()> {
        let temp_dir = tempfile::tempdir()
            .map_err(crate::error::AutoInstallError::IoError)?;

        let raw_path = temp_dir.path().join("image.raw");
        let mount_point = temp_dir.path().join("mount");

        tokio::fs::create_dir_all(&mount_point).await
            .map_err(crate::error::AutoInstallError::IoError)?;

        // Convert to raw
        Self::convert_to_raw(qcow2_path.as_ref(), &raw_path).await?;

        // Mount image
        let loop_device = Self::mount_raw_image(&raw_path, &mount_point).await?;

        // Copy contents
        let output = Command::new("rsync")
            .args([
                "-av",
                "--numeric-ids",
                &format!("{}/", mount_point.display()),
                target_dir.as_ref().to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to copy image contents: {}", e)
            ))?;

        // Cleanup
        let _ = Self::unmount_image(&mount_point, &loop_device).await;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Content extraction failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        info!("Image contents extracted successfully");
        Ok(())
    }
}

/// QEMU image information
#[derive(Debug)]
pub struct ImageInfo {
    pub format: String,
    pub virtual_size: u64,
    pub actual_size: u64,
    pub cluster_size: Option<u64>,
    pub compressed: bool,
}
