// file: src/image/builder/iso.rs
// version: 1.0.0
// guid: a1a2a3a4-b5b6-7890-1234-567890abcdef

//! ISO download and caching functionality

use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;
use crate::config::{ImageSpec, Architecture};
use crate::network::NetworkDownloader;
use crate::Result;

/// ISO download and caching manager
pub struct IsoManager {
    cache_dir: PathBuf,
}

impl IsoManager {
    /// Create a new ISO manager with cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Download Ubuntu ISO if not cached
    pub async fn get_ubuntu_iso(&self, spec: &ImageSpec) -> Result<PathBuf> {
        let iso_name = format!("ubuntu-{}-netboot-{}.iso",
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

        info!("Downloading Ubuntu netboot ISO to cache: {}", iso_path.display());

        let url = self.get_ubuntu_iso_url(spec)?;
        self.download_file(&url, &iso_path).await?;

        Ok(iso_path)
    }

    /// Get Ubuntu ISO download URL
    fn get_ubuntu_iso_url(&self, spec: &ImageSpec) -> Result<String> {
        // Ubuntu netboot images for text-mode installation
        // Format: http://archive.ubuntu.com/ubuntu/dists/{version}/main/installer-{arch}/current/legacy-images/netboot/mini.iso
        let arch_suffix = match spec.architecture {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        };

        Ok(format!("http://archive.ubuntu.com/ubuntu/dists/{}/main/installer-{}/current/legacy-images/netboot/mini.iso",
                  spec.ubuntu_version, arch_suffix))
    }

    /// Download file with progress
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let downloader = NetworkDownloader::new();
        downloader.download_with_progress(url, dest).await
    }
}
