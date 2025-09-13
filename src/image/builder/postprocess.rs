// file: src/image/builder/postprocess.rs
// version: 1.0.1
// guid: d1d2d3d4-e5e6-7890-1234-567890defghi

//! Image post-processing: generalization and finalization

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;
use tracing::{info, warn};
use crate::config::ImageSpec;
use crate::Result;

/// Image post-processing manager
pub struct PostProcessor {
    work_dir: PathBuf,
    cache_dir: PathBuf,
}

impl PostProcessor {
    /// Create a new post-processor
    pub fn new(work_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self { work_dir, cache_dir }
    }

    /// Generalize the image (remove machine-specific data)
    pub async fn generalize_image(&self, vm_disk: &Path) -> Result<()> {
        info!("Generalizing image");

        // Mount the image
        let mount_point = self.work_dir.join("mount");
        fs::create_dir_all(&mount_point).await?;

        // Use guestfish to modify the image
        let script = format!(r#"
add {}
run
mount /dev/sda2 /

# Remove machine-specific files
rm-rf /etc/machine-id
rm-rf /var/lib/dbus/machine-id
rm-rf /etc/ssh/ssh_host_*
rm-rf /var/log/*
rm-rf /tmp/*
rm-rf /var/tmp/*
rm-rf /root/.bash_history
rm-rf /home/ubuntu/.bash_history

# Clear package cache
rm-rf /var/cache/apt/archives/*.deb

# Create empty machine-id (will be regenerated on first boot)
touch /etc/machine-id

sync
umount-all
"#, vm_disk.display());

        let output = Command::new("guestfish")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to start guestfish: {}", e)
            ))?;

        // Write script to stdin and wait for completion
        let mut child = output;
        if let Some(stdin) = child.stdin.take() {
            tokio::io::AsyncWriteExt::write_all(&mut tokio::io::BufWriter::new(stdin), script.as_bytes()).await?;
        }

        let result = child.wait().await?;
        if !result.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                "Image generalization failed".to_string()
            ));
        }

        info!("Image generalization completed");
        Ok(())
    }

    /// Finalize and compress the image
    pub async fn finalize_image(&self, vm_disk: &Path, output_path: Option<String>, spec: &ImageSpec) -> Result<PathBuf> {
        info!("Finalizing image");

        let final_path = if let Some(output) = output_path {
            PathBuf::from(output)
        } else {
            // Create images directory in cache
            let images_dir = self.cache_dir.join("images");
            fs::create_dir_all(&images_dir).await
                .map_err(crate::error::AutoInstallError::IoError)?;

            images_dir.join(format!("ubuntu-{}-{}-{}.qcow2",
                                  spec.ubuntu_version,
                                  spec.architecture.as_str(),
                                  chrono::Utc::now().format("%Y%m%d-%H%M%S")))
        };

        // Ensure output directory exists
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(crate::error::AutoInstallError::IoError)?;
        }

        // Compress the image
        let output = Command::new("qemu-img")
            .args([
                "convert",
                "-c",  // Compress
                "-O", "qcow2",
                vm_disk.to_str().unwrap(),
                final_path.to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::ImageError(
                format!("Failed to compress image: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ImageError(
                format!("Image compression failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        // Calculate checksum for integrity verification
        let checksum = self.calculate_image_checksum(&final_path).await?;
        info!("Image checksum (SHA256): {}", checksum);

        // Get image size
        let metadata = fs::metadata(&final_path).await
            .map_err(crate::error::AutoInstallError::IoError)?;
        let size_bytes = metadata.len();

        // Register the image if it was created successfully
        let manager = crate::image::manager::ImageManager::new();
        let image_info = crate::config::ImageInfo::new(
            spec.ubuntu_version.clone(),
            spec.architecture,
            size_bytes,
            checksum,
            final_path.clone(),
        );

        if let Err(e) = manager.register_image(image_info).await {
            warn!("Failed to register image in database: {}", e);
        }

        info!("Image compressed to: {} ({})", final_path.display(),
              Self::format_size(size_bytes));
        Ok(final_path)
    }

    /// Calculate SHA256 checksum of image file
    async fn calculate_image_checksum(&self, image_path: &Path) -> Result<String> {
        use sha2::{Sha256, Digest};
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(image_path).await
            .map_err(crate::error::AutoInstallError::IoError)?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await
                .map_err(crate::error::AutoInstallError::IoError)?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Format file size in human-readable format
    fn format_size(size_bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size_bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
