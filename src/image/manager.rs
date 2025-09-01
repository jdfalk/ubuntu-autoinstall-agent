// file: src/image/manager.rs
// version: 1.0.0
// guid: e5f6a7b8-c9d0-1234-5678-901234efabcd

//! Image management orchestration
//!
//! This module coordinates the entire golden image workflow:
//! - Image creation using the builder
//! - Image customization for specific machines
//! - Image deployment to target systems

use super::{
    Architecture, ImageInfo, TargetMachine,
    builder::ImageBuilder,
    customizer::ImageCustomizer,
    deployer::ImageDeployer,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManagerConfig {
    pub work_dir: PathBuf,
    pub image_cache_dir: PathBuf,
    pub template_dir: PathBuf,
    pub qemu_system_path: String,
    pub qemu_img_path: String,
    pub ubuntu_mirror: String,
    pub vm_memory_mb: u32,
    pub vm_disk_size_gb: u32,
}

impl Default for ImageManagerConfig {
    fn default() -> Self {
        Self {
            work_dir: PathBuf::from("/tmp/ubuntu-autoinstall-agent"),
            image_cache_dir: PathBuf::from("/var/cache/ubuntu-autoinstall-agent/images"),
            template_dir: PathBuf::from("/etc/ubuntu-autoinstall-agent/templates"),
            qemu_system_path: "qemu-system-x86_64".to_string(),
            qemu_img_path: "qemu-img".to_string(),
            ubuntu_mirror: "http://archive.ubuntu.com/ubuntu".to_string(),
            vm_memory_mb: 4096,
            vm_disk_size_gb: 20,
        }
    }
}

pub struct ImageManager {
    config: ImageManagerConfig,
    builder: ImageBuilder,
}

impl ImageManager {
    pub fn new(config: ImageManagerConfig) -> Self {
        let builder = ImageBuilder::new(
            config.work_dir.clone(),
            config.qemu_system_path.clone(),
            config.qemu_img_path.clone(),
        );

        Self { config, builder }
    }

    /// Initialize the image manager (create directories, check dependencies)
    pub async fn initialize(&self) -> Result<()> {
        // Create necessary directories
        fs::create_dir_all(&self.config.work_dir).await
            .context("Failed to create work directory")?;

        fs::create_dir_all(&self.config.image_cache_dir).await
            .context("Failed to create image cache directory")?;

        fs::create_dir_all(&self.config.template_dir).await
            .context("Failed to create template directory")?;

        // Check QEMU dependencies
        self.check_qemu_dependencies().await?;

        println!("Image manager initialized successfully");
        Ok(())
    }

    /// Create a golden image for the specified Ubuntu version and architecture
    pub async fn create_golden_image(
        &self,
        ubuntu_version: &str,
        architecture: Architecture,
        image_name: Option<String>,
    ) -> Result<ImageInfo> {
        let image_name = image_name.unwrap_or_else(|| {
            format!("ubuntu-{}-{}", ubuntu_version, architecture.as_str())
        });

        println!("Creating golden image: {}", image_name);

        // Check if image already exists in cache
        if let Some(cached_image) = self.check_cached_image(&image_name).await? {
            println!("Using cached image: {}", cached_image.name);
            return Ok(cached_image);
        }

        // Build the golden image
        let image_info = self.builder.build_golden_image(
            ubuntu_version,
            architecture,
            &image_name,
            &self.config.image_cache_dir,
            self.config.vm_memory_mb,
            self.config.vm_disk_size_gb,
            &self.config.ubuntu_mirror,
        ).await?;

        println!("Golden image created successfully: {}", image_info.name);
        Ok(image_info)
    }

    /// Deploy a golden image to a target machine
    pub async fn deploy_image(
        &self,
        image_info: &ImageInfo,
        target: &TargetMachine,
        template_name: Option<&str>,
    ) -> Result<()> {
        println!("Deploying image {} to target {}", image_info.name, target.hostname);

        // Create deployer
        let deployer = ImageDeployer::new();

        // Create temporary mount point for customization
        let mount_point = self.config.work_dir.join("mounts").join(&target.hostname);
        fs::create_dir_all(&mount_point).await
            .context("Failed to create mount point")?;

        // Deploy the base image
        deployer.deploy_to_target(image_info, target).await?;

        // Customize for the specific machine
        let mut customizer = ImageCustomizer::new(mount_point.clone());
        customizer.load_templates(&self.config.template_dir).await?;

        // Mount the deployed image for customization
        self.mount_target_filesystem(target, &mount_point).await?;

        // Apply customizations
        customizer.customize(target, template_name).await
            .context("Failed to apply customizations")?;

        // Unmount the filesystem
        self.unmount_target_filesystem(&mount_point).await?;

        // Update bootloader and finalize
        deployer.finalize_deployment(target).await?;

        println!("Image deployed successfully to {}", target.hostname);
        Ok(())
    }

    /// List available golden images
    pub async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let mut images = Vec::new();

        if !self.config.image_cache_dir.exists() {
            return Ok(images);
        }

        let mut entries = fs::read_dir(&self.config.image_cache_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "img") {
                if let Ok(image_info) = self.load_image_metadata(&path).await {
                    images.push(image_info);
                }
            }
        }

        images.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(images)
    }

    /// Remove an image from the cache
    pub async fn remove_image(&self, image_name: &str) -> Result<()> {
        let image_path = self.config.image_cache_dir.join(format!("{}.img", image_name));
        let metadata_path = self.config.image_cache_dir.join(format!("{}.json", image_name));

        if image_path.exists() {
            fs::remove_file(&image_path).await
                .context("Failed to remove image file")?;
        }

        if metadata_path.exists() {
            fs::remove_file(&metadata_path).await
                .context("Failed to remove metadata file")?;
        }

        println!("Image {} removed successfully", image_name);
        Ok(())
    }

    /// Clean up old images (keep only the N most recent)
    pub async fn cleanup_images(&self, keep_count: usize) -> Result<()> {
        let mut images = self.list_images().await?;

        if images.len() <= keep_count {
            return Ok(());
        }

        // Sort by creation date (newest first)
        images.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Remove old images
        for image in images.iter().skip(keep_count) {
            self.remove_image(&image.name).await?;
        }

        println!("Cleaned up {} old images", images.len() - keep_count);
        Ok(())
    }

    async fn check_cached_image(&self, image_name: &str) -> Result<Option<ImageInfo>> {
        let metadata_path = self.config.image_cache_dir.join(format!("{}.json", image_name));

        if !metadata_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&metadata_path).await?;
        let image_info: ImageInfo = serde_json::from_str(&content)?;

        // Verify the actual image file exists
        let image_path = self.config.image_cache_dir.join(format!("{}.img", image_name));
        if !image_path.exists() {
            return Ok(None);
        }

        Ok(Some(image_info))
    }

    async fn load_image_metadata(&self, image_path: &PathBuf) -> Result<ImageInfo> {
        let stem = image_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid image path"))?;

        let metadata_path = image_path.with_extension("json");
        let content = fs::read_to_string(&metadata_path).await?;
        let image_info: ImageInfo = serde_json::from_str(&content)?;

        Ok(image_info)
    }

    async fn check_qemu_dependencies(&self) -> Result<()> {
        // Check QEMU system binary
        let output = tokio::process::Command::new(&self.config.qemu_system_path)
            .arg("--version")
            .output()
            .await
            .context("QEMU system binary not found")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("QEMU system binary check failed"));
        }

        // Check qemu-img binary
        let output = tokio::process::Command::new(&self.config.qemu_img_path)
            .arg("--version")
            .output()
            .await
            .context("qemu-img binary not found")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("qemu-img binary check failed"));
        }

        println!("QEMU dependencies verified");
        Ok(())
    }

    async fn mount_target_filesystem(&self, target: &TargetMachine, mount_point: &PathBuf) -> Result<()> {
        // Open the LUKS device
        let luks_device = format!("/dev/mapper/{}_crypt", target.hostname);
        let unlock_command = format!(
            "echo '{}' | cryptsetup luksOpen {} {}_crypt",
            "placeholder_password", // This would come from secure storage
            target.disk_device,
            target.hostname
        );

        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&unlock_command)
            .status()
            .await
            .context("Failed to unlock LUKS device")?;

        // Mount the root filesystem
        tokio::process::Command::new("mount")
            .args(&[&luks_device, mount_point.to_str().unwrap()])
            .status()
            .await
            .context("Failed to mount root filesystem")?;

        // Mount necessary filesystems for chroot
        let bind_mounts = ["/dev", "/proc", "/sys", "/run"];
        for bind_mount in &bind_mounts {
            let target_path = mount_point.join(bind_mount.trim_start_matches('/'));
            fs::create_dir_all(&target_path).await?;

            tokio::process::Command::new("mount")
                .args(&["--bind", bind_mount, target_path.to_str().unwrap()])
                .status()
                .await
                .with_context(|| format!("Failed to bind mount {}", bind_mount))?;
        }

        Ok(())
    }

    async fn unmount_target_filesystem(&self, mount_point: &PathBuf) -> Result<()> {
        // Unmount bind mounts
        let bind_mounts = ["/run", "/sys", "/proc", "/dev"];
        for bind_mount in &bind_mounts {
            let target_path = mount_point.join(bind_mount.trim_start_matches('/'));
            tokio::process::Command::new("umount")
                .arg(target_path.to_str().unwrap())
                .status()
                .await
                .ok(); // Ignore errors for cleanup
        }

        // Unmount root filesystem
        tokio::process::Command::new("umount")
            .arg(mount_point.to_str().unwrap())
            .status()
            .await
            .context("Failed to unmount root filesystem")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_image_manager_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let config = ImageManagerConfig {
            work_dir: temp_dir.path().join("work"),
            image_cache_dir: temp_dir.path().join("cache"),
            template_dir: temp_dir.path().join("templates"),
            ..Default::default()
        };

        let manager = ImageManager::new(config);

        // This will fail in test environment without QEMU, but tests directory creation
        assert!(manager.initialize().await.is_err());
        assert!(manager.config.work_dir.exists());
    }

    #[tokio::test]
    async fn test_list_empty_images() {
        let temp_dir = TempDir::new().unwrap();
        let config = ImageManagerConfig {
            image_cache_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = ImageManager::new(config);
        let images = manager.list_images().await.unwrap();
        assert!(images.is_empty());
    }
}
