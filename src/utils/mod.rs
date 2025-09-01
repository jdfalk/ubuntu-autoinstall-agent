// file: src/utils/mod.rs
// version: 1.0.0
// guid: p8q9r0s1-t2u3-4567-8901-def234567890

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub mod crypto;
pub mod disk;
pub mod network;
pub mod system;
pub mod validation;

/// Common utilities for the installation agent
pub struct Utils;

impl Utils {
    /// Generate a new UUID
    pub fn generate_uuid() -> Uuid {
        Uuid::new_v4()
    }

    /// Get current timestamp as ISO 8601 string
    pub fn current_timestamp() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    /// Convert bytes to human-readable format
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
        const THRESHOLD: u64 = 1024;

        if bytes == 0 {
            return "0 B".to_string();
        }

        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
            size /= THRESHOLD as f64;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }

    /// Parse duration from string with units (e.g., "30s", "5m", "1h")
    pub fn parse_duration(input: &str) -> Result<Duration> {
        let input = input.trim();

        if input.is_empty() {
            return Err(anyhow::anyhow!("Empty duration string"));
        }

        // Handle pure numbers as seconds
        if let Ok(secs) = input.parse::<u64>() {
            return Ok(Duration::from_secs(secs));
        }

        // Extract number and unit
        let (number_part, unit_part) = if input.chars().last().unwrap().is_alphabetic() {
            let split_pos = input.chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .count();
            (&input[..split_pos], &input[split_pos..])
        } else {
            (input, "s") // Default to seconds
        };

        let number: f64 = number_part.parse()
            .context("Invalid number in duration")?;

        let multiplier = match unit_part.to_lowercase().as_str() {
            "s" | "sec" | "second" | "seconds" => 1,
            "m" | "min" | "minute" | "minutes" => 60,
            "h" | "hr" | "hour" | "hours" => 3600,
            "d" | "day" | "days" => 86400,
            _ => return Err(anyhow::anyhow!("Unknown duration unit: {}", unit_part)),
        };

        Ok(Duration::from_secs((number * multiplier as f64) as u64))
    }

    /// Format duration as human-readable string
    pub fn format_duration(duration: Duration) -> String {
        let total_secs = duration.as_secs();

        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Sanitize a string for use in filenames
    pub fn sanitize_filename(input: &str) -> String {
        input.chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
                ' ' => '_',
                _ => '-',
            })
            .collect()
    }

    /// Create a backup file with timestamp
    pub async fn create_backup(original_path: &Path) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = original_path.with_extension(
            format!("backup_{}.{}",
                   timestamp,
                   original_path.extension()
                       .and_then(|ext| ext.to_str())
                       .unwrap_or("bak"))
        );

        tokio::fs::copy(original_path, &backup_path).await
            .with_context(|| format!("Failed to create backup of {:?}", original_path))?;

        info!("Created backup: {:?} -> {:?}", original_path, backup_path);
        Ok(backup_path)
    }

    /// Restore file from backup
    pub async fn restore_backup(backup_path: &Path, original_path: &Path) -> Result<()> {
        tokio::fs::copy(backup_path, original_path).await
            .with_context(|| format!("Failed to restore backup {:?} to {:?}", backup_path, original_path))?;

        info!("Restored backup: {:?} -> {:?}", backup_path, original_path);
        Ok(())
    }

    /// Execute command with timeout and capture output
    pub async fn execute_command_with_timeout(
        command: &str,
        args: &[&str],
        timeout: Duration,
        working_dir: Option<&Path>,
    ) -> Result<CommandOutput> {
        let start_time = Instant::now();

        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        debug!("Executing command: {} {}", command, args.join(" "));

        let child = cmd.spawn()
            .with_context(|| format!("Failed to spawn command: {}", command))?;

        let output = tokio::time::timeout(timeout, child.wait_with_output()).await
            .context("Command execution timed out")?
            .context("Failed to wait for command completion")?;

        let execution_time = start_time.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        debug!("Command completed in {:?}: exit_code={}, stdout_lines={}, stderr_lines={}",
               execution_time, output.status.code().unwrap_or(-1),
               stdout.lines().count(), stderr.lines().count());

        Ok(CommandOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            execution_time,
        })
    }

    /// Check if a command exists in PATH
    pub async fn command_exists(command: &str) -> bool {
        tokio::process::Command::new("which")
            .arg(command)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get available disk space for a path
    pub async fn get_available_space(path: &Path) -> Result<u64> {
        let output = tokio::process::Command::new("df")
            .args(&["-B1", path.to_str().unwrap_or("/")])
            .output()
            .await
            .context("Failed to get disk space info")?;

        let stdout = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in df output")?;

        stdout.lines()
            .nth(1) // Skip header
            .and_then(|line| line.split_whitespace().nth(3)) // Available bytes column
            .and_then(|bytes_str| bytes_str.parse::<u64>().ok())
            .context("Failed to parse available space")
    }

    /// Wait for a condition to be true with timeout
    pub async fn wait_for_condition<F, Fut>(
        condition: F,
        timeout: Duration,
        check_interval: Duration,
        description: &str,
    ) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<bool>>,
    {
        let start_time = Instant::now();

        debug!("Waiting for condition: {}", description);

        loop {
            match condition().await {
                Ok(true) => {
                    debug!("Condition met after {:?}: {}", start_time.elapsed(), description);
                    return Ok(());
                }
                Ok(false) => {
                    // Condition not met, continue waiting
                }
                Err(e) => {
                    warn!("Error checking condition '{}': {}", description, e);
                }
            }

            if start_time.elapsed() >= timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for condition: {} (waited {:?})",
                    description, timeout
                ));
            }

            tokio::time::sleep(check_interval).await;
        }
    }

    /// Retry an operation with exponential backoff
    pub async fn retry_with_backoff<F, Fut, T>(
        mut operation: F,
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        description: &str,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut delay = initial_delay;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            debug!("Attempting {}: attempt {}/{}", description, attempt, max_attempts);

            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!("Operation succeeded on attempt {}/{}: {}", attempt, max_attempts, description);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < max_attempts {
                        warn!("Attempt {}/{} failed for {}, retrying in {:?}",
                              attempt, max_attempts, description, delay);
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, max_delay);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed: {}", description)))
    }

    /// Calculate SHA-256 hash of a file
    pub async fn calculate_file_hash(path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};

        let content = tokio::fs::read(path).await
            .with_context(|| format!("Failed to read file for hashing: {:?}", path))?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();

        Ok(format!("{:x}", hash))
    }

    /// Verify file integrity using SHA-256 hash
    pub async fn verify_file_hash(path: &Path, expected_hash: &str) -> Result<bool> {
        let actual_hash = Self::calculate_file_hash(path).await?;
        Ok(actual_hash.eq_ignore_ascii_case(expected_hash))
    }

    /// Create a temporary directory
    pub async fn create_temp_dir(prefix: &str) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let dir_name = format!("{}_{}", prefix, timestamp);
        let temp_path = temp_dir.join(dir_name);

        tokio::fs::create_dir_all(&temp_path).await
            .with_context(|| format!("Failed to create temporary directory: {:?}", temp_path))?;

        debug!("Created temporary directory: {:?}", temp_path);
        Ok(temp_path)
    }

    /// Clean up temporary directory
    pub async fn cleanup_temp_dir(path: &Path) -> Result<()> {
        if path.exists() {
            tokio::fs::remove_dir_all(path).await
                .with_context(|| format!("Failed to remove temporary directory: {:?}", path))?;
            debug!("Cleaned up temporary directory: {:?}", path);
        }
        Ok(())
    }

    /// Read configuration from multiple sources with precedence
    pub async fn load_config_with_precedence<T>(
        config_paths: &[PathBuf],
        env_prefix: Option<&str>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Default,
    {
        let mut config = T::default();

        // Load from files in order (later files override earlier ones)
        for path in config_paths {
            if path.exists() {
                info!("Loading configuration from: {:?}", path);
                let content = tokio::fs::read_to_string(path).await
                    .with_context(|| format!("Failed to read config file: {:?}", path))?;

                // Support both YAML and JSON
                let file_config: T = if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    serde_json::from_str(&content)
                        .with_context(|| format!("Failed to parse JSON config: {:?}", path))?
                } else {
                    serde_yaml::from_str(&content)
                        .with_context(|| format!("Failed to parse YAML config: {:?}", path))?
                };

                // Merge configuration (this would require a custom merge trait in practice)
                config = file_config;
            }
        }

        // Override with environment variables if prefix specified
        if let Some(prefix) = env_prefix {
            debug!("Checking for environment variable overrides with prefix: {}", prefix);
            // Environment variable override logic would go here
            // This requires implementing per-field environment variable mapping
        }

        Ok(config)
    }

    /// Validate network connectivity
    pub async fn check_network_connectivity(hosts: &[&str], timeout: Duration) -> Result<Vec<NetworkConnectivityResult>> {
        let mut results = Vec::new();

        for host in hosts {
            let start_time = Instant::now();
            let result = tokio::time::timeout(
                timeout,
                tokio::net::TcpStream::connect(host)
            ).await;

            let connectivity_result = match result {
                Ok(Ok(_)) => NetworkConnectivityResult {
                    host: host.to_string(),
                    reachable: true,
                    latency: Some(start_time.elapsed()),
                    error: None,
                },
                Ok(Err(e)) => NetworkConnectivityResult {
                    host: host.to_string(),
                    reachable: false,
                    latency: Some(start_time.elapsed()),
                    error: Some(e.to_string()),
                },
                Err(_) => NetworkConnectivityResult {
                    host: host.to_string(),
                    reachable: false,
                    latency: Some(start_time.elapsed()),
                    error: Some("Connection timeout".to_string()),
                },
            };

            results.push(connectivity_result);
        }

        Ok(results)
    }

    /// Get system load information
    pub async fn get_system_load() -> Result<SystemLoad> {
        let loadavg_content = tokio::fs::read_to_string("/proc/loadavg").await
            .context("Failed to read load average")?;

        let fields: Vec<&str> = loadavg_content.split_whitespace().collect();
        if fields.len() < 3 {
            return Err(anyhow::anyhow!("Invalid load average format"));
        }

        Ok(SystemLoad {
            load_1min: fields[0].parse().context("Invalid 1-minute load average")?,
            load_5min: fields[1].parse().context("Invalid 5-minute load average")?,
            load_15min: fields[2].parse().context("Invalid 15-minute load average")?,
        })
    }

    /// Get memory usage information
    pub async fn get_memory_usage() -> Result<MemoryUsage> {
        let meminfo_content = tokio::fs::read_to_string("/proc/meminfo").await
            .context("Failed to read memory info")?;

        let mut total_kb = 0u64;
        let mut available_kb = 0u64;

        for line in meminfo_content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                available_kb = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }

        let used_kb = total_kb.saturating_sub(available_kb);
        let usage_percent = if total_kb > 0 {
            (used_kb as f64 / total_kb as f64) * 100.0
        } else {
            0.0
        };

        Ok(MemoryUsage {
            total_mb: total_kb / 1024,
            used_mb: used_kb / 1024,
            available_mb: available_kb / 1024,
            usage_percent,
        })
    }
}

/// Output from command execution
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub execution_time: Duration,
}

impl CommandOutput {
    /// Check if the command was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get combined output (stdout + stderr)
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("STDOUT:\n{}\n\nSTDERR:\n{}", self.stdout, self.stderr)
        }
    }
}

/// Network connectivity test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnectivityResult {
    pub host: String,
    pub reachable: bool,
    pub latency: Option<Duration>,
    pub error: Option<String>,
}

/// System load information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLoad {
    pub load_1min: f64,
    pub load_5min: f64,
    pub load_15min: f64,
}

/// Memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub total_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
    pub usage_percent: f64,
}

/// Environment variable configuration
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub prefix: String,
    pub overrides: HashMap<String, String>,
}

impl EnvConfig {
    /// Create new environment configuration
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            overrides: HashMap::new(),
        }
    }

    /// Get environment variable with prefix
    pub fn get_var(&self, key: &str) -> Option<String> {
        let env_key = format!("{}_{}", self.prefix, key.to_uppercase());
        std::env::var(&env_key).ok()
            .or_else(|| self.overrides.get(key).cloned())
    }

    /// Set override value
    pub fn set_override(&mut self, key: &str, value: &str) {
        self.overrides.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(Utils::format_bytes(0), "0 B");
        assert_eq!(Utils::format_bytes(1023), "1023 B");
        assert_eq!(Utils::format_bytes(1024), "1.00 KB");
        assert_eq!(Utils::format_bytes(1536), "1.50 KB");
        assert_eq!(Utils::format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(Utils::parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(Utils::parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(Utils::parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(Utils::parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(Utils::parse_duration("2d").unwrap(), Duration::from_secs(172800));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(Utils::format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(Utils::format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(Utils::format_duration(Duration::from_secs(3661)), "1h 1m 1s");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(Utils::sanitize_filename("hello world"), "hello_world");
        assert_eq!(Utils::sanitize_filename("test@file.txt"), "test-file.txt");
        assert_eq!(Utils::sanitize_filename("valid_name-123.txt"), "valid_name-123.txt");
    }

    #[tokio::test]
    async fn test_command_exists() {
        assert!(Utils::command_exists("ls").await); // Should exist on Unix systems
        assert!(!Utils::command_exists("definitely_not_a_command_12345").await);
    }

    #[test]
    fn test_env_config() {
        let mut env_config = EnvConfig::new("TEST");
        env_config.set_override("key1", "value1");

        assert_eq!(env_config.get_var("key1"), Some("value1".to_string()));
        assert_eq!(env_config.get_var("nonexistent"), None);
    }
}
