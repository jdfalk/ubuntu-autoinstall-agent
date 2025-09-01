// file: src/steps/mod.rs
// version: 1.0.0
// guid: h8i9j0k1-l2m3-4567-8901-bcdef234567

use crate::config::InstallConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Context passed to each installation step
#[derive(Debug, Clone)]
pub struct StepContext<'a> {
    /// Current installation session ID
    pub session_id: Uuid,

    /// Installation configuration
    pub config: &'a InstallConfig,

    /// Current step number (1-based)
    pub step_number: usize,

    /// Total number of steps
    pub total_steps: usize,
}

/// Result of executing an installation step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Status of the step execution
    pub status: StepStatus,

    /// Human-readable message describing the result
    pub message: String,

    /// Error message if the step failed
    pub error_message: Option<String>,

    /// Time taken to execute the step
    pub execution_time: Duration,

    /// Additional metadata from the step execution
    pub metadata: HashMap<String, String>,
}

/// Status of a step execution
#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    /// Step completed successfully
    Completed,

    /// Step failed
    Failed,

    /// Step was skipped (e.g., not applicable)
    Skipped,

    /// Step is in progress
    InProgress,

    /// Step is waiting for prerequisites
    Waiting,
}

/// Trait for installation steps
#[async_trait::async_trait]
pub trait InstallStep {
    /// Get the name of this step
    fn name(&self) -> &str;

    /// Get a description of what this step does
    fn description(&self) -> &str;

    /// Execute the step
    async fn execute(&self, context: &StepContext<'_>) -> StepResult;

    /// Validate that the step can be executed
    async fn validate(&self, context: &StepContext<'_>) -> Result<()>;

    /// Cleanup any resources created by this step
    async fn cleanup(&self, context: &StepContext<'_>) -> Result<()>;

    /// Check if this step should be skipped
    async fn should_skip(&self, context: &StepContext<'_>) -> bool {
        false
    }

    /// Get estimated execution time for this step
    fn estimated_duration(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Get prerequisites for this step
    fn prerequisites(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get what this step provides for other steps
    fn provides(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Helper for creating successful step results
pub fn success_result(message: String, execution_time: Duration) -> StepResult {
    StepResult {
        status: StepStatus::Completed,
        message,
        error_message: None,
        execution_time,
        metadata: HashMap::new(),
    }
}

/// Helper for creating successful step results with metadata
pub fn success_result_with_metadata(
    message: String,
    execution_time: Duration,
    metadata: HashMap<String, String>,
) -> StepResult {
    StepResult {
        status: StepStatus::Completed,
        message,
        error_message: None,
        execution_time,
        metadata,
    }
}

/// Helper for creating failed step results
pub fn failure_result(message: String, error: String, execution_time: Duration) -> StepResult {
    StepResult {
        status: StepStatus::Failed,
        message,
        error_message: Some(error),
        execution_time,
        metadata: HashMap::new(),
    }
}

/// Helper for creating skipped step results
pub fn skipped_result(reason: String) -> StepResult {
    StepResult {
        status: StepStatus::Skipped,
        message: format!("Step skipped: {}", reason),
        error_message: None,
        execution_time: Duration::from_secs(0),
        metadata: HashMap::new(),
    }
}

/// Base step implementation with common functionality
pub struct BaseStep {
    pub name: String,
    pub description: String,
    pub estimated_duration: Duration,
    pub prerequisites: Vec<String>,
    pub provides: Vec<String>,
}

impl BaseStep {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            estimated_duration: Duration::from_secs(30),
            prerequisites: Vec::new(),
            provides: Vec::new(),
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.estimated_duration = duration;
        self
    }

    pub fn with_prerequisites(mut self, prerequisites: Vec<String>) -> Self {
        self.prerequisites = prerequisites;
        self
    }

    pub fn with_provides(mut self, provides: Vec<String>) -> Self {
        self.provides = provides;
        self
    }
}

/// Utility functions for step execution
pub mod utils {
    use super::*;
    use anyhow::{Context, Result};
    use tokio::process::Command;
    use tracing::{debug, warn};

    /// Execute a shell command and return the result
    pub async fn execute_command(
        command: &str,
        args: &[&str],
        context: &str,
    ) -> Result<std::process::Output> {
        debug!("Executing command: {} {}", command, args.join(" "));

        Command::new(command)
            .args(args)
            .output()
            .await
            .with_context(|| format!("Failed to execute {}: {}", context, command))
    }

    /// Execute a shell command and check if it succeeded
    pub async fn execute_command_checked(
        command: &str,
        args: &[&str],
        context: &str,
    ) -> Result<String> {
        let output = execute_command(command, args, context).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "{} failed with exit code {}: stdout={}, stderr={}",
                context,
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            );
        }

        Ok(String::from_utf8(output.stdout)
           .with_context(|| format!("Invalid UTF-8 output from {}", context))?)
    }

    /// Execute a bash script
    pub async fn execute_script(script: &str, context: &str) -> Result<String> {
        debug!("Executing script for: {}", context);
        execute_command_checked("bash", &["-c", script], context).await
    }

    /// Check if a file exists
    pub async fn file_exists(path: &str) -> bool {
        tokio::fs::metadata(path).await.is_ok()
    }

    /// Check if a directory exists
    pub async fn directory_exists(path: &str) -> bool {
        tokio::fs::metadata(path).await.map(|m| m.is_dir()).unwrap_or(false)
    }

    /// Create a directory if it doesn't exist
    pub async fn ensure_directory(path: &str) -> Result<()> {
        if !directory_exists(path).await {
            tokio::fs::create_dir_all(path)
                .await
                .with_context(|| format!("Failed to create directory: {}", path))?;
        }
        Ok(())
    }

    /// Write content to a file
    pub async fn write_file(path: &str, content: &str) -> Result<()> {
        tokio::fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path))
    }

    /// Read content from a file
    pub async fn read_file(path: &str) -> Result<String> {
        tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path))
    }

    /// Check if a service is active
    pub async fn is_service_active(service_name: &str) -> Result<bool> {
        let output = execute_command("systemctl", &["is-active", service_name], "service check").await?;
        Ok(String::from_utf8_lossy(&output.stdout).trim() == "active")
    }

    /// Start a systemd service
    pub async fn start_service(service_name: &str) -> Result<()> {
        execute_command_checked("systemctl", &["start", service_name], &format!("start service {}", service_name)).await?;
        Ok(())
    }

    /// Enable a systemd service
    pub async fn enable_service(service_name: &str) -> Result<()> {
        execute_command_checked("systemctl", &["enable", service_name], &format!("enable service {}", service_name)).await?;
        Ok(())
    }

    /// Stop a systemd service
    pub async fn stop_service(service_name: &str) -> Result<()> {
        execute_command_checked("systemctl", &["stop", service_name], &format!("stop service {}", service_name)).await?;
        Ok(())
    }

    /// Check if a package is installed
    pub async fn is_package_installed(package_name: &str) -> Result<bool> {
        let output = execute_command("dpkg", &["-l", package_name], "package check").await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(&format!("ii  {}", package_name)))
    }

    /// Install packages using apt
    pub async fn install_packages(packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        debug!("Installing packages: {:?}", packages);

        // Update package list first
        execute_command_checked("apt", &["update"], "apt update").await?;

        // Install packages
        let mut args = vec!["install", "-y"];
        let package_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        args.extend(package_refs);

        execute_command_checked("apt", &args, "package installation").await?;

        Ok(())
    }

    /// Get disk usage information
    pub async fn get_disk_usage(path: &str) -> Result<DiskUsage> {
        let output = execute_command_checked("df", &["-h", path], "disk usage check").await?;

        // Parse df output
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() < 2 {
            anyhow::bail!("Invalid df output format");
        }

        let fields: Vec<&str> = lines[1].split_whitespace().collect();
        if fields.len() < 6 {
            anyhow::bail!("Invalid df output format");
        }

        Ok(DiskUsage {
            filesystem: fields[0].to_string(),
            size: fields[1].to_string(),
            used: fields[2].to_string(),
            available: fields[3].to_string(),
            use_percentage: fields[4].to_string(),
            mounted_on: fields[5].to_string(),
        })
    }

    /// Disk usage information
    #[derive(Debug, Clone)]
    pub struct DiskUsage {
        pub filesystem: String,
        pub size: String,
        pub used: String,
        pub available: String,
        pub use_percentage: String,
        pub mounted_on: String,
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
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if condition().await {
                debug!("Condition met: {}", description);
                return Ok(());
            }

            tokio::time::sleep(check_interval).await;
        }

        warn!("Timeout waiting for condition: {}", description);
        anyhow::bail!("Timeout waiting for: {}", description)
    }

    /// Retry an operation with exponential backoff
    pub async fn retry_with_backoff<F, Fut, T, E>(
        operation: F,
        max_retries: u32,
        initial_delay: Duration,
        description: &str,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut delay = initial_delay;

        for attempt in 1..=max_retries {
            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        debug!("Operation succeeded on attempt {}: {}", attempt, description);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if attempt == max_retries {
                        warn!("Operation failed after {} attempts: {}: {}", max_retries, description, e);
                        anyhow::bail!("Operation failed after {} attempts: {}: {}", max_retries, description, e);
                    }

                    warn!("Operation failed on attempt {}/{}: {}: {} - retrying in {:?}",
                          attempt, max_retries, description, e, delay);

                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, Duration::from_secs(60)); // Cap at 60 seconds
                }
            }
        }

        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_step_result_creation() {
        let result = success_result("Test passed".to_string(), Duration::from_secs(1));
        assert_eq!(result.status, StepStatus::Completed);
        assert_eq!(result.message, "Test passed");
        assert!(result.error_message.is_none());
    }

    #[tokio::test]
    async fn test_failure_result_creation() {
        let result = failure_result(
            "Test failed".to_string(),
            "Error details".to_string(),
            Duration::from_secs(1),
        );
        assert_eq!(result.status, StepStatus::Failed);
        assert_eq!(result.message, "Test failed");
        assert_eq!(result.error_message, Some("Error details".to_string()));
    }

    #[tokio::test]
    async fn test_base_step_creation() {
        let step = BaseStep::new("Test Step", "A test step")
            .with_duration(Duration::from_secs(60))
            .with_prerequisites(vec!["prereq1".to_string()])
            .with_provides(vec!["output1".to_string()]);

        assert_eq!(step.name, "Test Step");
        assert_eq!(step.description, "A test step");
        assert_eq!(step.estimated_duration, Duration::from_secs(60));
        assert_eq!(step.prerequisites, vec!["prereq1"]);
        assert_eq!(step.provides, vec!["output1"]);
    }
}
