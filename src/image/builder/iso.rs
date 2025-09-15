// file: src/image/builder/iso.rs
// version: 1.0.1
// guid: a1a2a3a4-b5b6-7890-1234-567890abcdef

//! ISO management and download utilities

use crate::{
    config::{Architecture, ImageSpec},
    network::download::NetworkDownloader,
    Result,
};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

/// ISO download and caching manager
pub struct IsoManager {
    cache_dir: PathBuf,
}

impl IsoManager {
    /// Create a new ISO manager with cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Download Ubuntu Server ISO if not cached and extract kernel/initrd for direct boot
    pub async fn get_ubuntu_iso(&self, spec: &ImageSpec) -> Result<PathBuf> {
        // For autoinstall, we need the full Ubuntu Server ISO, not netboot
        let iso_dir = self.cache_dir.join("isos").join(format!(
            "ubuntu-{}-{}",
            spec.ubuntu_version,
            spec.architecture.as_str()
        ));

        let extract_dir = self.cache_dir.join("extracted").join(format!(
            "ubuntu-{}-{}",
            spec.ubuntu_version,
            spec.architecture.as_str()
        ));

        fs::create_dir_all(&iso_dir)
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        fs::create_dir_all(&extract_dir)
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        // Check if kernel files already extracted
        let kernel_path = extract_dir.join("casper").join("vmlinuz");
        if kernel_path.exists() {
            info!(
                "Using cached Ubuntu Server ISO files: {}",
                extract_dir.display()
            );
            return Ok(extract_dir);
        }

        info!(
            "Downloading and extracting Ubuntu Server ISO: {}",
            extract_dir.display()
        );

        // Download Ubuntu Server ISO
        let iso_url = self.get_ubuntu_server_iso_url(spec)?;
        let iso_path = iso_dir.join(format!(
            "ubuntu-{}-live-server-{}.iso",
            spec.ubuntu_version,
            spec.architecture.as_str()
        ));

        info!("Downloading Ubuntu Server ISO from: {}", iso_url);
        self.download_file(&iso_url, &iso_path).await?;

        // Extract kernel and initrd from ISO
        self.extract_iso_boot_files(&iso_path, &extract_dir).await?;

        Ok(extract_dir)
    }
    /// Get Ubuntu Server ISO download URL
    fn get_ubuntu_server_iso_url(&self, spec: &ImageSpec) -> Result<String> {
        // Ubuntu Server ISO URLs follow this pattern:
        // https://releases.ubuntu.com/{codename}/ubuntu-{version}-live-server-{arch}.iso
        let arch_suffix = match spec.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        };

        // Convert version to codename for releases URL
        let codename = match spec.ubuntu_version.as_str() {
            "25.04" => "plucky",
            "24.10" => "oracular",
            "24.04" => "noble",
            "23.10" => "mantic",
            "23.04" => "lunar",
            _ => {
                return Err(crate::error::AutoInstallError::ConfigError(format!(
                    "Unsupported Ubuntu version: {}",
                    spec.ubuntu_version
                )))
            }
        };

        Ok(format!(
            "https://releases.ubuntu.com/{}/ubuntu-{}-live-server-{}.iso",
            codename, spec.ubuntu_version, arch_suffix
        ))
    }

    /// Extract kernel and initrd from Ubuntu Server ISO for direct boot
    async fn extract_iso_boot_files(&self, iso_path: &Path, extract_dir: &Path) -> Result<()> {
        use tokio::process::Command;

        info!("Extracting boot files from ISO: {}", iso_path.display());

        // Mount the ISO and extract kernel/initrd files
        let mount_dir = extract_dir.join("mnt");
        fs::create_dir_all(&mount_dir)
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        // Create a temporary mount script since we need sudo
        let mount_script = extract_dir.join("mount_iso.sh");
        let mount_script_content = format!(
            r#"#!/bin/bash
set -e

# Mount ISO
sudo mount -o loop "{}" "{}"

# Copy casper directory (contains kernel and initrd)
cp -r "{}/casper" "{}"

# Copy .disk directory (contains installer metadata)
if [ -d "{}/disk" ]; then
    cp -r "{}/disk" "{}"
fi

# Unmount ISO
sudo umount "{}"

echo "ISO boot files extracted successfully"
"#,
            iso_path.display(),
            mount_dir.display(),
            mount_dir.display(),
            extract_dir.display(),
            mount_dir.display(),
            mount_dir.display(),
            extract_dir.display(),
            mount_dir.display()
        );

        fs::write(&mount_script, mount_script_content)
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        // Make script executable
        Command::new("chmod")
            .args(["+x", mount_script.to_str().unwrap()])
            .output()
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        // Execute mount script
        let output = Command::new("bash")
            .arg(mount_script.to_str().unwrap())
            .output()
            .await
            .map_err(crate::error::AutoInstallError::IoError)?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::ProcessError {
                command: format!("bash {}", mount_script.display()),
                exit_code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Clean up mount script
        let _ = fs::remove_file(mount_script).await;

        info!("Successfully extracted boot files from Ubuntu Server ISO");
        Ok(())
    }

    /// Download file with progress
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let downloader = NetworkDownloader::new();
        downloader.download_with_progress(url, dest).await
    }
}
