// file: src/image/builder.rs
// version: 1.2.0
// guid: a1b2c3d4-e5f6-7890-1234-567890abcdef

//! Golden image builder using QEMU/KVM

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};
use crate::config::{ImageSpec, Architecture};
use crate::network::NetworkDownloader;
use crate::utils::{VmManager};
use crate::Result;

/// Golden image builder using QEMU/KVM
pub struct ImageBuilder {
    vm_manager: VmManager,
    work_dir: PathBuf,
    cache_dir: PathBuf,
}

impl ImageBuilder {
    /// Create a new image builder with default cache directory
    pub fn new() -> Self {
        let default_cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("ubuntu-autoinstall");

        Self {
            vm_manager: VmManager::new(),
            work_dir: default_cache.join("work"),
            cache_dir: default_cache,
        }
    }

    /// Create a new image builder with custom cache directory
    pub fn with_cache_dir<P: AsRef<std::path::Path>>(cache_dir: P) -> Self {
        let cache_path = cache_dir.as_ref().to_path_buf();
        Self {
            vm_manager: VmManager::new(),
            work_dir: cache_path.join("work"),
            cache_dir: cache_path,
        }
    }/ version: 1.0.0
// guid: l2m3n4o5-p6q7-8901-2345-678901lmnopq

//! Golden image builder using QEMU/KVM

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};
use crate::config::{ImageSpec, Architecture};
use crate::network::NetworkDownloader;
use crate::utils::{VmManager};
use crate::Result;

/// Builder for creating golden Ubuntu images
pub struct ImageBuilder {
    vm_manager: VmManager,
    work_dir: PathBuf,
}

impl ImageBuilder {
    /// Create a new image builder
    pub fn new() -> Self {
        Self {
            vm_manager: VmManager::new(),
            work_dir: PathBuf::from("/tmp/ubuntu-autoinstall"),
        }
    }

    /// Create a golden image from specification
    pub async fn create_image(
        &self,
        spec: ImageSpec,
        output_path: Option<String>,
    ) -> Result<PathBuf> {
        info!("Creating Ubuntu {} image for {}",
              spec.ubuntu_version, spec.architecture.as_str());

        // Create working directory
        self.setup_work_dir().await?;

        // Download Ubuntu ISO
        let iso_path = self.download_ubuntu_iso(&spec).await?;

        // Create VM and install Ubuntu
        let vm_disk = self.create_vm_and_install(&spec, &iso_path).await?;

        // Generalize the image (remove machine-specific data)
        self.generalize_image(&vm_disk).await?;

        // Compress and finalize image
        let final_path = self.finalize_image(&vm_disk, output_path, &spec).await?;

        // Cleanup
        self.cleanup_work_dir().await?;

        info!("Image creation completed: {}", final_path.display());
        Ok(final_path)
    }

    /// Set up working directory
    async fn setup_work_dir(&self) -> Result<()> {
        // Create both work and cache directories
        fs::create_dir_all(&self.work_dir).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
        fs::create_dir_all(&self.cache_dir).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        debug!("Work directory created: {}", self.work_dir.display());
        debug!("Cache directory: {}", self.cache_dir.display());
        Ok(())
    }

    /// Download Ubuntu ISO if not cached
    async fn download_ubuntu_iso(&self, spec: &ImageSpec) -> Result<PathBuf> {
        let iso_name = format!("ubuntu-{}-live-server-{}.iso",
                              spec.ubuntu_version, spec.architecture.as_str());

        // Create cache/isos directory for storing ISO files
        let iso_cache_dir = self.cache_dir.join("isos");
        fs::create_dir_all(&iso_cache_dir).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        let iso_path = iso_cache_dir.join(&iso_name);

        if iso_path.exists() {
            info!("Using cached ISO: {}", iso_path.display());
            return Ok(iso_path);
        }

        info!("Downloading Ubuntu ISO to cache: {}", iso_path.display());

        let url = self.get_ubuntu_iso_url(spec)?;
        self.download_file(&url, &iso_path).await?;

        Ok(iso_path)
    }

    /// Get Ubuntu ISO download URL
    fn get_ubuntu_iso_url(&self, spec: &ImageSpec) -> Result<String> {
        // Ubuntu ISO URLs follow this pattern:
        // https://releases.ubuntu.com/{version}/ubuntu-{version}-live-server-{arch}.iso
        let arch_suffix = match spec.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        };

        Ok(format!("https://releases.ubuntu.com/{}/ubuntu-{}-live-server-{}.iso",
                  spec.ubuntu_version, spec.ubuntu_version, arch_suffix))
    }

    /// Download file with progress
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let downloader = NetworkDownloader::new();
        downloader.download_with_progress(url, dest).await
    }

    /// Create VM and install Ubuntu
    async fn create_vm_and_install(&self, spec: &ImageSpec, iso_path: &Path) -> Result<PathBuf> {
        info!("Creating VM and installing Ubuntu");

        let vm_disk = self.work_dir.join("ubuntu-install.qcow2");

        // Create disk image
        self.create_qemu_disk(&vm_disk, spec.vm_config.disk_size_gb).await?;

        // Create cloud-init config for automated installation
        let cloud_init_path = self.create_cloud_init_config(spec).await?;

        // Start VM and perform installation
        self.vm_manager.install_ubuntu(
            &vm_disk,
            iso_path,
            &cloud_init_path,
            &spec.vm_config,
            spec.architecture,
        ).await?;

        Ok(vm_disk)
    }

    /// Create QEMU disk image
    async fn create_qemu_disk(&self, disk_path: &Path, size_gb: u32) -> Result<()> {
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

    /// Create cloud-init configuration for automated installation
    async fn create_cloud_init_config(&self, spec: &ImageSpec) -> Result<PathBuf> {
        let cloud_init_dir = self.work_dir.join("cloud-init");
        fs::create_dir_all(&cloud_init_dir).await?;

        // Create user-data
        let user_data = self.generate_user_data(spec)?;
        let user_data_path = cloud_init_dir.join("user-data");
        fs::write(&user_data_path, user_data).await?;

        // Create meta-data
        let meta_data = format!("instance-id: ubuntu-autoinstall-{}\n", uuid::Uuid::new_v4());
        let meta_data_path = cloud_init_dir.join("meta-data");
        fs::write(&meta_data_path, meta_data).await?;

        debug!("Created cloud-init config in: {}", cloud_init_dir.display());
        Ok(cloud_init_dir)
    }

    /// Generate cloud-init user-data for automated installation
    fn generate_user_data(&self, spec: &ImageSpec) -> Result<String> {
        let packages = spec.base_packages.join("\n      - ");

        let config = format!(r#"#cloud-config
autoinstall:
  version: 1
  locale: en_US.UTF-8
  keyboard:
    layout: us
    variant: ''
  network:
    network:
      version: 2
      ethernets:
        eth0:
          dhcp4: true
  storage:
    layout:
      name: direct
      match:
        size: largest
    swap:
      size: 0
  packages:
      - {}
  ssh:
    install-server: true
    allow-pw: false
  user-data:
    disable_root: true
    users:
      - name: ubuntu
        sudo: ALL=(ALL) NOPASSWD:ALL
        shell: /bin/bash
        lock_passwd: true
        ssh_authorized_keys:
          - ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQDHQGvTZ8nZ8/temp-key-for-image-creation
    timezone: UTC
  late-commands:
    # Remove temporary SSH key and prepare image for generalization
    - rm -f /target/home/ubuntu/.ssh/authorized_keys
    - echo "Image creation completed at $(date)" > /target/var/log/autoinstall.log
    # Ensure cloud-init will run on first boot
    - touch /target/etc/cloud/cloud-init.disabled && rm /target/etc/cloud/cloud-init.disabled
    # Clean up any installer logs that might contain sensitive data
    - rm -f /target/var/log/installer/autoinstall-user-data
  error-commands:
    - echo "Installation failed at $(date)" > /target/var/log/autoinstall-error.log
"#, packages);

        Ok(config)
    }

    /// Generalize the image (remove machine-specific data)
    async fn generalize_image(&self, vm_disk: &Path) -> Result<()> {
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
    async fn finalize_image(&self, vm_disk: &Path, output_path: Option<String>, spec: &ImageSpec) -> Result<PathBuf> {
        info!("Finalizing image");

        let final_path = if let Some(output) = output_path {
            PathBuf::from(output)
        } else {
            // Create images directory in cache
            let images_dir = self.cache_dir.join("images");
            fs::create_dir_all(&images_dir).await
                .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

            images_dir.join(format!("ubuntu-{}-{}-{}.qcow2",
                                  spec.ubuntu_version,
                                  spec.architecture.as_str(),
                                  chrono::Utc::now().format("%Y%m%d-%H%M%S")))
        };

        // Ensure output directory exists
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
        }

        // Compress the image
        let output = Command::new("qemu-img")
            .args(&[
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
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
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
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await
                .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

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

    /// Cleanup working directory
    async fn cleanup_work_dir(&self) -> Result<()> {
        if self.work_dir.exists() {
            fs::remove_dir_all(&self.work_dir).await
                .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
            debug!("Cleaned up work directory");
        }
        Ok(())
    }
}

impl Default for ImageBuilder {
    fn default() -> Self {
        Self::new()
    }
}
