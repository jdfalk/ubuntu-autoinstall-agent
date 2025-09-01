// file: src/image/builder.rs
// version: 1.0.0
// guid: b2c3d4e5-f6a7-8901-2345-678901bcdefg

//! Golden image builder for creating standardized Ubuntu images
//!
//! This module handles creating VM-based golden images that can be
//! deployed to target machines with machine-specific customizations.

use super::{Architecture, ImageInfo, ImageError};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;
use tokio::process::Command as AsyncCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageBuildConfig {
    pub ubuntu_version: String,
    pub architecture: Architecture,
    pub vm_memory_mb: u32,
    pub vm_disk_size_gb: u32,
    pub packages: Vec<String>,
    pub scripts: Vec<PathBuf>,
    pub output_dir: PathBuf,
}

pub struct ImageBuilder {
    config: ImageBuildConfig,
    work_dir: PathBuf,
}

impl ImageBuilder {
    pub fn new(config: ImageBuildConfig) -> Self {
        let work_dir = std::env::temp_dir().join("ubuntu-image-builder");
        Self { config, work_dir }
    }

    /// Create a golden image using QEMU/KVM
    pub async fn create_golden_image(&self) -> Result<ImageInfo> {
        self.setup_work_directory().await?;

        let vm_disk = self.create_vm_disk().await?;
        let iso_path = self.download_ubuntu_iso().await?;

        // Create VM and install Ubuntu
        self.install_ubuntu_in_vm(&vm_disk, &iso_path).await?;

        // Boot VM and run provisioning
        self.provision_vm(&vm_disk).await?;

        // Generalize the system (remove machine-specific data)
        self.generalize_vm(&vm_disk).await?;

        // Create compressed image
        let image_path = self.create_final_image(&vm_disk).await?;

        // Generate image info
        let image_info = self.generate_image_info(&image_path).await?;

        Ok(image_info)
    }

    async fn setup_work_directory(&self) -> Result<()> {
        fs::create_dir_all(&self.work_dir).await
            .context("Failed to create work directory")?;
        Ok(())
    }

    async fn create_vm_disk(&self) -> Result<PathBuf> {
        let disk_path = self.work_dir.join("ubuntu.qcow2");

        let output = AsyncCommand::new("qemu-img")
            .args(&[
                "create",
                "-f", "qcow2",
                disk_path.to_str().unwrap(),
                &format!("{}G", self.config.vm_disk_size_gb),
            ])
            .output()
            .await
            .context("Failed to run qemu-img")?;

        if !output.status.success() {
            return Err(ImageError {
                message: format!("qemu-img failed: {}", String::from_utf8_lossy(&output.stderr)),
                source: None,
            }.into());
        }

        Ok(disk_path)
    }

    async fn download_ubuntu_iso(&self) -> Result<PathBuf> {
        let iso_name = format!(
            "ubuntu-{}-server-{}.iso",
            self.config.ubuntu_version,
            self.config.architecture.as_str()
        );
        let iso_path = self.work_dir.join(&iso_name);

        if iso_path.exists() {
            return Ok(iso_path);
        }

        // Download Ubuntu ISO
        let iso_url = format!(
            "https://releases.ubuntu.com/{}/ubuntu-{}-server-{}.iso",
            self.config.ubuntu_version,
            self.config.ubuntu_version,
            self.config.architecture.as_str()
        );

        println!("Downloading Ubuntu ISO: {}", iso_url);

        let response = reqwest::get(&iso_url).await
            .context("Failed to download Ubuntu ISO")?;

        let bytes = response.bytes().await
            .context("Failed to read ISO bytes")?;

        fs::write(&iso_path, bytes).await
            .context("Failed to write ISO file")?;

        Ok(iso_path)
    }

    async fn install_ubuntu_in_vm(&self, vm_disk: &PathBuf, iso_path: &PathBuf) -> Result<()> {
        // Create autoinstall configuration
        let autoinstall_config = self.create_autoinstall_config().await?;

        // Create cloud-init ISO for autoinstall
        let cloud_init_iso = self.create_cloud_init_iso(&autoinstall_config).await?;

        // Run QEMU with autoinstall
        let mut qemu_cmd = AsyncCommand::new("qemu-system-x86_64");
        qemu_cmd
            .args(&[
                "-m", &self.config.vm_memory_mb.to_string(),
                "-smp", "2",
                "-enable-kvm",
                "-drive", &format!("file={},format=qcow2", vm_disk.display()),
                "-drive", &format!("file={},media=cdrom", iso_path.display()),
                "-drive", &format!("file={},media=cdrom", cloud_init_iso.display()),
                "-boot", "d",
                "-vnc", ":1",
                "-daemonize",
            ]);

        let output = qemu_cmd.output().await
            .context("Failed to start QEMU")?;

        if !output.status.success() {
            return Err(ImageError {
                message: format!("QEMU failed: {}", String::from_utf8_lossy(&output.stderr)),
                source: None,
            }.into());
        }

        // Wait for installation to complete
        self.wait_for_installation().await?;

        Ok(())
    }

    async fn create_autoinstall_config(&self) -> Result<String> {
        let config = format!(r#"
#cloud-config
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
  identity:
    hostname: ubuntu-golden
    username: ubuntu
    password: '$6$rounds=4096$saltsalt$hash'
  ssh:
    install-server: true
    allow-pw: true
  packages:
    - openssh-server
    - cloud-init
    - {}
  late-commands:
    - echo 'ubuntu ALL=(ALL) NOPASSWD:ALL' > /target/etc/sudoers.d/ubuntu
"#, self.config.packages.join("\n    - "));

        Ok(config)
    }

    async fn create_cloud_init_iso(&self, config: &str) -> Result<PathBuf> {
        let cloud_init_dir = self.work_dir.join("cloud-init");
        fs::create_dir_all(&cloud_init_dir).await?;

        // Write user-data
        let user_data_path = cloud_init_dir.join("user-data");
        fs::write(&user_data_path, config).await?;

        // Write meta-data
        let meta_data_path = cloud_init_dir.join("meta-data");
        fs::write(&meta_data_path, "instance-id: golden-image\n").await?;

        // Create ISO
        let iso_path = self.work_dir.join("cloud-init.iso");
        let output = AsyncCommand::new("genisoimage")
            .args(&[
                "-output", iso_path.to_str().unwrap(),
                "-volid", "cidata",
                "-joliet",
                "-rock",
                cloud_init_dir.to_str().unwrap(),
            ])
            .output()
            .await
            .context("Failed to create cloud-init ISO")?;

        if !output.status.success() {
            return Err(ImageError {
                message: format!("genisoimage failed: {}", String::from_utf8_lossy(&output.stderr)),
                source: None,
            }.into());
        }

        Ok(iso_path)
    }

    async fn wait_for_installation(&self) -> Result<()> {
        // TODO: Implement proper waiting mechanism
        // For now, just sleep (in real implementation, we'd monitor the VM)
        tokio::time::sleep(tokio::time::Duration::from_secs(1800)).await; // 30 minutes
        Ok(())
    }

    async fn provision_vm(&self, vm_disk: &PathBuf) -> Result<()> {
        // Boot the VM and run provisioning scripts
        // TODO: Implement SSH connection and script execution
        println!("Provisioning VM...");
        Ok(())
    }

    async fn generalize_vm(&self, vm_disk: &PathBuf) -> Result<()> {
        // Remove machine-specific data to prepare for cloning
        // TODO: Implement generalization (similar to Windows sysprep)
        println!("Generalizing VM...");
        Ok(())
    }

    async fn create_final_image(&self, vm_disk: &PathBuf) -> Result<PathBuf> {
        let final_image = self.config.output_dir.join(format!(
            "ubuntu-{}-{}.img.zst",
            self.config.ubuntu_version,
            self.config.architecture.as_str()
        ));

        // Convert qcow2 to raw and compress with zstd
        let output = AsyncCommand::new("qemu-img")
            .args(&[
                "convert",
                "-f", "qcow2",
                "-O", "raw",
                vm_disk.to_str().unwrap(),
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start qemu-img convert")?;

        let mut zstd_cmd = AsyncCommand::new("zstd")
            .args(&["-o", final_image.to_str().unwrap()])
            .stdin(output.stdout.unwrap())
            .spawn()
            .context("Failed to start zstd compression")?;

        let convert_result = output.wait().await?;
        let compress_result = zstd_cmd.wait().await?;

        if !convert_result.success() || !compress_result.success() {
            return Err(ImageError {
                message: "Failed to convert and compress image".to_string(),
                source: None,
            }.into());
        }

        Ok(final_image)
    }

    async fn generate_image_info(&self, image_path: &PathBuf) -> Result<ImageInfo> {
        let metadata = fs::metadata(image_path).await
            .context("Failed to get image metadata")?;

        // Calculate checksum
        let checksum = self.calculate_checksum(image_path).await?;

        Ok(ImageInfo {
            name: format!("ubuntu-{}", self.config.ubuntu_version),
            version: self.config.ubuntu_version.clone(),
            architecture: self.config.architecture.clone(),
            ubuntu_version: self.config.ubuntu_version.clone(),
            size_bytes: metadata.len(),
            checksum,
            created_at: chrono::Utc::now(),
            path: image_path.clone(),
        })
    }

    async fn calculate_checksum(&self, path: &PathBuf) -> Result<String> {
        let output = AsyncCommand::new("sha256sum")
            .arg(path.to_str().unwrap())
            .output()
            .await
            .context("Failed to calculate checksum")?;

        if !output.status.success() {
            return Err(ImageError {
                message: "Failed to calculate checksum".to_string(),
                source: None,
            }.into());
        }

        let checksum = String::from_utf8(output.stdout)
            .context("Invalid checksum output")?
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        Ok(checksum)
    }
}
