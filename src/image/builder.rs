// file: src/image/builder.rs
// version: 1.0.0
// guid: l2m3n4o5-p6q7-8901-2345-678901lmnopq

//! Golden image builder using QEMU/KVM

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::fs;
use tracing::{info, debug};
use crate::{
    config::{ImageSpec, Architecture},
    utils::vm::VmManager,
    Result,
};

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
        let final_path = self.finalize_image(&vm_disk, output_path).await?;

        // Cleanup
        self.cleanup_work_dir().await?;

        info!("Image creation completed: {}", final_path.display());
        Ok(final_path)
    }

    /// Setup working directory
    async fn setup_work_dir(&self) -> Result<()> {
        if self.work_dir.exists() {
            fs::remove_dir_all(&self.work_dir).await
                .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
        }

        fs::create_dir_all(&self.work_dir).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        debug!("Work directory created: {}", self.work_dir.display());
        Ok(())
    }

    /// Download Ubuntu ISO if not cached
    async fn download_ubuntu_iso(&self, spec: &ImageSpec) -> Result<PathBuf> {
        let iso_name = format!("ubuntu-{}-server-{}.iso", 
                              spec.ubuntu_version, spec.architecture.as_str());
        let iso_path = self.work_dir.join(&iso_name);

        if iso_path.exists() {
            info!("Using cached ISO: {}", iso_path.display());
            return Ok(iso_path);
        }

        info!("Downloading Ubuntu ISO: {}", iso_name);

        let url = self.get_ubuntu_iso_url(spec)?;
        self.download_file(&url, &iso_path).await?;

        Ok(iso_path)
    }

    /// Get Ubuntu ISO download URL
    fn get_ubuntu_iso_url(&self, spec: &ImageSpec) -> Result<String> {
        let base_url = "https://releases.ubuntu.com";
        let arch_suffix = match spec.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        };

        Ok(format!("{}/{}/ubuntu-{}-server-{}.iso",
                  base_url, spec.ubuntu_version, spec.ubuntu_version, arch_suffix))
    }

    /// Download file with progress
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        use futures::StreamExt;
        use indicatif::{ProgressBar, ProgressStyle};

        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;
        
        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut file = tokio::fs::File::create(dest).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download completed");
        info!("Downloaded: {}", dest.display());
        Ok(())
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
  network:
    network:
      version: 2
      ethernets:
        eth0:
          dhcp4: true
  storage:
    layout:
      name: direct
  packages:
      - {}
  ssh:
    install-server: true
  user-data:
    disable_root: true
    users:
      - name: ubuntu
        sudo: ALL=(ALL) NOPASSWD:ALL
        shell: /bin/bash
        lock_passwd: true
        ssh_authorized_keys:
          - ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQDHQ... # Temporary key for image creation
  late-commands:
    - 'echo "Image creation completed" > /target/var/log/autoinstall.log'
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
    async fn finalize_image(&self, vm_disk: &Path, output_path: Option<String>) -> Result<PathBuf> {
        info!("Finalizing image");

        let final_path = if let Some(output) = output_path {
            PathBuf::from(output)
        } else {
            PathBuf::from(format!("ubuntu-{}.qcow2", chrono::Utc::now().format("%Y%m%d-%H%M%S")))
        };

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

        info!("Image compressed to: {}", final_path.display());
        Ok(final_path)
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