// file: src/config/loader.rs
// version: 1.0.0
// guid: d4e5f6g7-h8i9-0123-4567-890abcdef123

use super::{InstallConfig, validator};
use anyhow::{Context, Result};
use reqwest;
use serde_yaml;
use std::path::Path;
use tokio::fs;
use tracing::{info, warn, debug};

/// Configuration loader that supports both local files and remote URLs
pub struct ConfigLoader {
    /// HTTP client for downloading remote configurations
    client: reqwest::Client,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("ubuntu-autoinstall-agent/1.0.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Load configuration from a source (file path or URL)
    pub async fn load_config(&self, source: &str) -> Result<InstallConfig> {
        info!("Loading configuration from: {}", source);

        let content = if source.starts_with("http://") || source.starts_with("https://") {
            self.load_remote_config(source).await?
        } else {
            self.load_local_config(source).await?
        };

        let config: InstallConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML configuration from {}", source))?;

        // Validate the configuration
        validator::validate_config(&config)
            .with_context(|| format!("Configuration validation failed for {}", source))?;

        info!("Successfully loaded and validated configuration from: {}", source);
        Ok(config)
    }

    /// Load configuration from a local file
    async fn load_local_config(&self, file_path: &str) -> Result<String> {
        debug!("Loading local configuration file: {}", file_path);

        let path = Path::new(file_path);
        if !path.exists() {
            anyhow::bail!("Configuration file does not exist: {}", file_path);
        }

        let content = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read configuration file: {}", file_path))?;

        Ok(content)
    }

    /// Load configuration from a remote URL
    async fn load_remote_config(&self, url: &str) -> Result<String> {
        debug!("Downloading remote configuration from: {}", url);

        let response = self.client.get(url).send().await
            .with_context(|| format!("Failed to download configuration from: {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error {} when downloading configuration from: {}",
                         response.status(), url);
        }

        let content = response.text().await
            .with_context(|| format!("Failed to read response body from: {}", url))?;

        Ok(content)
    }

    /// Load multiple configurations and merge them
    pub async fn load_multi_config(&self, sources: &[String]) -> Result<InstallConfig> {
        if sources.is_empty() {
            anyhow::bail!("No configuration sources provided");
        }

        info!("Loading configurations from {} sources", sources.len());

        // Load the base configuration from the first source
        let mut config = self.load_config(&sources[0]).await?;

        // Merge additional configurations
        for source in sources.iter().skip(1) {
            let additional_config = self.load_config(source).await?;
            config = self.merge_configs(config, additional_config)?;
        }

        // Final validation after merging
        validator::validate_config(&config)
            .context("Final configuration validation failed after merging")?;

        info!("Successfully merged configurations from {} sources", sources.len());
        Ok(config)
    }

    /// Merge two configurations, with the second one taking precedence
    fn merge_configs(&self, mut base: InstallConfig, overlay: InstallConfig) -> Result<InstallConfig> {
        debug!("Merging configurations");

        // Simple field-by-field merge - overlay takes precedence for non-empty values
        if !overlay.hostname.is_empty() && overlay.hostname != "ubuntu-server" {
            base.hostname = overlay.hostname;
        }

        if !overlay.disk.is_empty() && overlay.disk != "/dev/sda" {
            base.disk = overlay.disk;
        }

        if !overlay.timezone.is_empty() && overlay.timezone != "UTC" {
            base.timezone = overlay.timezone;
        }

        // Merge network configuration
        if overlay.network.ethernet.interface != "eth0" {
            base.network.ethernet = overlay.network.ethernet;
        }

        if overlay.network.wifi.is_some() {
            base.network.wifi = overlay.network.wifi;
        }

        if !overlay.network.dns_servers.is_empty() {
            base.network.dns_servers = overlay.network.dns_servers;
        }

        // Merge encryption settings
        if overlay.encryption.enabled {
            base.encryption = overlay.encryption;
        }

        // Merge users (replace if overlay has users)
        if !overlay.users.is_empty() {
            base.users = overlay.users;
        }

        // Merge packages (append unique packages)
        for package in overlay.packages {
            if !base.packages.contains(&package) {
                base.packages.push(package);
            }
        }

        // Merge services (append unique services)
        for service in overlay.services {
            if !base.services.contains(&service) {
                base.services.push(service);
            }
        }

        // Merge ZFS configuration
        if overlay.zfs.root_pool != "rpool" {
            base.zfs.root_pool = overlay.zfs.root_pool;
        }
        if overlay.zfs.boot_pool != "bpool" {
            base.zfs.boot_pool = overlay.zfs.boot_pool;
        }

        // Merge ZFS properties
        for (key, value) in overlay.zfs.properties {
            base.zfs.properties.insert(key, value);
        }

        // Merge datasets
        base.zfs.datasets.extend(overlay.zfs.datasets);

        // Replace reporting config if provided
        if overlay.reporting.is_some() {
            base.reporting = overlay.reporting;
        }

        // Merge recovery configuration
        if overlay.recovery.auto_recovery != true || overlay.recovery.max_retries != 3 {
            base.recovery = overlay.recovery;
        }

        // Merge custom scripts
        for (stage, scripts) in overlay.custom_scripts {
            base.custom_scripts.entry(stage)
                .or_insert_with(Vec::new)
                .extend(scripts);
        }

        Ok(base)
    }

    /// Save configuration to a file
    pub async fn save_config(&self, config: &InstallConfig, file_path: &str) -> Result<()> {
        info!("Saving configuration to: {}", file_path);

        let yaml_content = serde_yaml::to_string(config)
            .context("Failed to serialize configuration to YAML")?;

        fs::write(file_path, yaml_content).await
            .with_context(|| format!("Failed to write configuration to: {}", file_path))?;

        info!("Successfully saved configuration to: {}", file_path);
        Ok(())
    }

    /// Download and cache a remote configuration locally
    pub async fn cache_remote_config(&self, url: &str, cache_path: &str) -> Result<()> {
        info!("Caching remote configuration {} to {}", url, cache_path);

        let content = self.load_remote_config(url).await?;

        // Ensure the cache directory exists
        if let Some(parent) = Path::new(cache_path).parent() {
            fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create cache directory: {:?}", parent))?;
        }

        fs::write(cache_path, content).await
            .with_context(|| format!("Failed to write cached configuration to: {}", cache_path))?;

        info!("Successfully cached configuration to: {}", cache_path);
        Ok(())
    }

    /// Check if a cached configuration is still valid (not too old)
    pub async fn is_cache_valid(&self, cache_path: &str, max_age_seconds: u64) -> bool {
        let path = Path::new(cache_path);
        if !path.exists() {
            return false;
        }

        match fs::metadata(path).await {
            Ok(metadata) => {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        return elapsed.as_secs() < max_age_seconds;
                    }
                }
            }
            Err(_) => return false,
        }

        false
    }

    /// Load configuration with caching support
    pub async fn load_config_with_cache(
        &self,
        source: &str,
        cache_path: Option<&str>,
        cache_max_age: Option<u64>
    ) -> Result<InstallConfig> {
        // If source is a URL and caching is enabled
        if (source.starts_with("http://") || source.starts_with("https://"))
            && cache_path.is_some()
            && cache_max_age.is_some() {

            let cache_path = cache_path.unwrap();
            let max_age = cache_max_age.unwrap();

            // Check if we have a valid cached version
            if self.is_cache_valid(cache_path, max_age).await {
                info!("Using cached configuration from: {}", cache_path);
                match self.load_config(cache_path).await {
                    Ok(config) => return Ok(config),
                    Err(e) => {
                        warn!("Failed to load cached config, falling back to remote: {}", e);
                    }
                }
            }

            // Cache is invalid or doesn't exist, download and cache
            if let Err(e) = self.cache_remote_config(source, cache_path).await {
                warn!("Failed to cache remote config: {}", e);
            }
        }

        // Load the configuration normally
        self.load_config(source).await
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_local_config() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_config.yaml");

        let test_config = InstallConfig::default();
        let yaml_content = serde_yaml::to_string(&test_config).unwrap();
        tokio::fs::write(&file_path, yaml_content).await.unwrap();

        let loader = ConfigLoader::new();
        let loaded_config = loader.load_config(file_path.to_str().unwrap()).await.unwrap();

        assert_eq!(loaded_config.hostname, test_config.hostname);
    }

    #[tokio::test]
    async fn test_merge_configs() {
        let loader = ConfigLoader::new();

        let mut base = InstallConfig::default();
        base.hostname = "base-host".to_string();
        base.packages = vec!["package1".to_string()];

        let mut overlay = InstallConfig::default();
        overlay.hostname = "overlay-host".to_string();
        overlay.packages = vec!["package2".to_string()];

        let merged = loader.merge_configs(base, overlay).unwrap();

        assert_eq!(merged.hostname, "overlay-host");
        assert_eq!(merged.packages.len(), 2);
        assert!(merged.packages.contains(&"package1".to_string()));
        assert!(merged.packages.contains(&"package2".to_string()));
    }
}
