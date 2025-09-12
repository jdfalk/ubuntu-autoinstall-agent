// file: src/image/builder/iso.rs
// version: 1.0.0
// guid: a1a2a3a4-b5b6-7890-1234-567890abcdef

//! ISO management and download utilities

use std::path::{Path, PathBuf};
use tokio::fs;
use crate::{
    config::{Architecture, ImageSpec},
    network::download::NetworkDownloader,
    Result,
};
use tracing::{info, debug};

/// ISO download and caching manager
pub struct IsoManager {
    cache_dir: PathBuf,
}

impl IsoManager {
    /// Create a new ISO manager with cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Download Ubuntu netboot files if not cached
    pub async fn get_ubuntu_iso(&self, spec: &ImageSpec) -> Result<PathBuf> {
        // For netboot, we'll create a bootable ISO from the extracted tarball
        let netboot_dir = self.cache_dir.join("netboot").join(format!("ubuntu-{}-{}",
                                                                     spec.ubuntu_version,
                                                                     spec.architecture.as_str()));
        let iso_name = format!("ubuntu-{}-netboot-{}.iso",
                              spec.ubuntu_version, spec.architecture.as_str());
        let iso_path = self.cache_dir.join("isos").join(&iso_name);

        // Create cache directories
        fs::create_dir_all(&self.cache_dir.join("isos")).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
        fs::create_dir_all(&netboot_dir).await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        if iso_path.exists() {
            info!("Using cached netboot ISO: {}", iso_path.display());
            return Ok(iso_path);
        }

        info!("Downloading and creating Ubuntu netboot ISO: {}", iso_path.display());

        // Download netboot tarball
        let tarball_url = self.get_netboot_tarball_url(spec)?;
        let tarball_path = self.cache_dir.join("isos").join(format!("ubuntu-{}-netboot-{}.tar.gz",
                                                                   spec.ubuntu_version,
                                                                   spec.architecture.as_str()));

        // Download tarball if not cached
        if !tarball_path.exists() {
            self.download_file(&tarball_url, &tarball_path).await?;
        }

        // Extract tarball
        self.extract_netboot_tarball(&tarball_path, &netboot_dir).await?;

        // Create bootable ISO from extracted files
        self.create_netboot_iso(&netboot_dir, &iso_path).await?;

        Ok(iso_path)
    }

    /// Get Ubuntu netboot tarball download URL
    fn get_netboot_tarball_url(&self, spec: &ImageSpec) -> Result<String> {
        // Ubuntu netboot tarball URLs follow this pattern:
        // https://releases.ubuntu.com/{codename}/ubuntu-{version}-netboot-{arch}.tar.gz
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
            _ => return Err(crate::error::AutoInstallError::ConfigError(
                format!("Unsupported Ubuntu version: {}", spec.ubuntu_version)
            )),
        };

        Ok(format!("https://releases.ubuntu.com/{}/ubuntu-{}-netboot-{}.tar.gz",
                  codename, spec.ubuntu_version, arch_suffix))
    }

    /// Extract netboot tarball
    async fn extract_netboot_tarball(&self, tarball_path: &Path, extract_dir: &Path) -> Result<()> {
        use tokio::process::Command;

        info!("Extracting netboot tarball to: {}", extract_dir.display());

        let output = Command::new("tar")
            .args(&[
                "-xzf", tarball_path.to_str().unwrap(),
                "-C", extract_dir.to_str().unwrap(),
                "--strip-components=1", // Remove top-level directory from tarball
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::IoError(
                std::io::Error::new(std::io::ErrorKind::Other,
                    format!("Failed to extract tarball: {}", String::from_utf8_lossy(&output.stderr)))
            ));
        }

        debug!("Netboot tarball extracted successfully");
        Ok(())
    }

    /// Create bootable ISO from netboot files
    async fn create_netboot_iso(&self, netboot_dir: &Path, iso_path: &Path) -> Result<()> {
        use tokio::process::Command;

        info!("Creating bootable ISO from netboot files");

        // Try genisoimage first (most common), then mkisofs as fallback
        let tools = [
            ("genisoimage", vec![
                "-r", "-J", "-l",
                "-V", "Ubuntu-Netboot",
                "-o", iso_path.to_str().unwrap(),
                netboot_dir.to_str().unwrap()
            ]),
            ("mkisofs", vec![
                "-r", "-J", "-l",
                "-V", "Ubuntu-Netboot",
                "-o", iso_path.to_str().unwrap(),
                netboot_dir.to_str().unwrap()
            ]),
        ];

        for (tool_name, args) in &tools {
            debug!("Trying ISO creation with: {}", tool_name);

            let result = Command::new(tool_name)
                .args(args)
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    info!("Netboot ISO created successfully using {}: {}", tool_name, iso_path.display());
                    return Ok(());
                }
                Ok(output) => {
                    debug!("Tool {} failed: {}", tool_name, String::from_utf8_lossy(&output.stderr));
                }
                Err(e) => {
                    debug!("Tool {} not available: {}", tool_name, e);
                }
            }
        }

        Err(crate::error::AutoInstallError::IoError(
            std::io::Error::new(std::io::ErrorKind::Other,
                "No ISO creation tools available. Please install one of: genisoimage, mkisofs, or xorriso.\n\
                 On Ubuntu/Debian: sudo apt install genisoimage\n\
                 On macOS: brew install cdrtools\n\
                 On RHEL/CentOS: sudo yum install genisoimage")
        ))
    }    /// Download file with progress
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let downloader = NetworkDownloader::new();
        downloader.download_with_progress(url, dest).await
    }
}
