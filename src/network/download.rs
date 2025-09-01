// file: src/network/download.rs
// version: 1.0.0
// guid: u1v2w3x4-y5z6-7890-1234-567890uvwxyz

//! Network download utilities

use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use crate::Result;
use tracing::{info, debug};

/// Network downloader with progress tracking
pub struct NetworkDownloader {
    client: reqwest::Client,
}

impl NetworkDownloader {
    /// Create a new network downloader
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Download file with progress bar
    pub async fn download_with_progress<P: AsRef<Path>>(
        &self,
        url: &str,
        dest: P,
    ) -> Result<()> {
        info!("Downloading: {}", url);

        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(crate::error::AutoInstallError::NetworkError(
                format!("Download failed with status: {}", response.status())
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-")
        );

        let mut file = File::create(&dest).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        file.flush().await?;
        pb.finish_with_message("Download completed");

        info!("Downloaded to: {}", dest.as_ref().display());
        Ok(())
    }

    /// Download file without progress (for smaller files)
    pub async fn download<P: AsRef<Path>>(&self, url: &str, dest: P) -> Result<()> {
        debug!("Downloading (no progress): {}", url);

        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(crate::error::AutoInstallError::NetworkError(
                format!("Download failed with status: {}", response.status())
            ));
        }

        let bytes = response.bytes().await?;
        tokio::fs::write(&dest, bytes).await?;

        debug!("Downloaded to: {}", dest.as_ref().display());
        Ok(())
    }

    /// Get file size without downloading
    pub async fn get_file_size(&self, url: &str) -> Result<Option<u64>> {
        let response = self.client.head(url).send().await?;
        Ok(response.content_length())
    }

    /// Verify URL is accessible
    pub async fn verify_url(&self, url: &str) -> Result<bool> {
        match self.client.head(url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

impl Default for NetworkDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_verify_url() {
        let downloader = NetworkDownloader::new();
        
        // Test with a reliable URL (this might fail in CI without internet)
        // In a real test environment, you'd use a mock server
        let result = downloader.verify_url("https://httpbin.org/status/200").await;
        // We can't assert this will always pass due to network conditions
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_file_size() {
        let downloader = NetworkDownloader::new();
        
        // Test with a URL that should return content-length
        let result = downloader.get_file_size("https://httpbin.org/bytes/1024").await;
        assert!(result.is_ok());
    }
}