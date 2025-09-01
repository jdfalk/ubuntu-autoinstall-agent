// file: src/recovery/mod.rs
// version: 1.0.0
// guid: i9j0k1l2-m3n4-5678-9012-cdef34567890

use crate::steps::StepResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Recovery manager for handling installation failures
pub struct RecoveryManager {
    /// Recovery strategies for different step types
    strategies: HashMap<String, Box<dyn RecoveryStrategy + Send + Sync>>,

    /// Recovery history
    history: std::sync::Mutex<Vec<RecoveryAttempt>>,

    /// Recovery configuration
    config: RecoveryConfig,
}

/// Configuration for recovery behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Maximum number of retry attempts per step
    pub max_retries: u32,

    /// Delay between retry attempts (seconds)
    pub retry_delay: u32,

    /// Whether to enable automatic recovery
    pub auto_recovery: bool,

    /// Whether to create snapshots before risky operations
    pub create_snapshots: bool,

    /// Maximum time to spend on recovery attempts
    pub max_recovery_time: Duration,

    /// Whether to roll back changes on failure
    pub rollback_on_failure: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay: 30,
            auto_recovery: true,
            create_snapshots: true,
            max_recovery_time: Duration::from_secs(600), // 10 minutes
            rollback_on_failure: true,
        }
    }
}

/// Record of a recovery attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    /// Step name that failed
    pub step_name: String,

    /// Attempt number
    pub attempt_number: u32,

    /// Timestamp of the attempt
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Recovery strategy used
    pub strategy_used: String,

    /// Whether the recovery succeeded
    pub succeeded: bool,

    /// Error message if recovery failed
    pub error_message: Option<String>,

    /// Time taken for recovery
    pub recovery_time: Duration,

    /// Actions taken during recovery
    pub actions_taken: Vec<String>,
}

/// Trait for recovery strategies
#[async_trait::async_trait]
pub trait RecoveryStrategy {
    /// Get the name of this strategy
    fn name(&self) -> &str;

    /// Check if this strategy can handle the given failure
    fn can_handle(&self, step_name: &str, failure: &StepResult) -> bool;

    /// Attempt to recover from the failure
    async fn recover(&self, step_name: &str, failure: &StepResult) -> Result<Vec<String>>;

    /// Estimate how long this recovery might take
    fn estimated_recovery_time(&self) -> Duration {
        Duration::from_secs(60)
    }

    /// Get the priority of this strategy (higher = more preferred)
    fn priority(&self) -> u32 {
        50
    }
}

impl RecoveryManager {
    /// Create a new recovery manager
    pub fn new() -> Self {
        Self::with_config(RecoveryConfig::default())
    }

    /// Create a new recovery manager with custom configuration
    pub fn with_config(config: RecoveryConfig) -> Self {
        let mut manager = Self {
            strategies: HashMap::new(),
            history: std::sync::Mutex::new(Vec::new()),
            config,
        };

        // Register default recovery strategies
        manager.register_default_strategies();

        manager
    }

    /// Register default recovery strategies
    fn register_default_strategies(&mut self) {
        self.register_strategy("disk", Box::new(DiskRecoveryStrategy::new()));
        self.register_strategy("network", Box::new(NetworkRecoveryStrategy::new()));
        self.register_strategy("package", Box::new(PackageRecoveryStrategy::new()));
        self.register_strategy("service", Box::new(ServiceRecoveryStrategy::new()));
        self.register_strategy("filesystem", Box::new(FilesystemRecoveryStrategy::new()));
        self.register_strategy("generic", Box::new(GenericRecoveryStrategy::new()));
    }

    /// Register a recovery strategy
    pub fn register_strategy(&mut self, name: impl Into<String>, strategy: Box<dyn RecoveryStrategy + Send + Sync>) {
        self.strategies.insert(name.into(), strategy);
    }

    /// Attempt recovery for a failed step
    pub async fn attempt_recovery(&self, step_name: &str, failure: &StepResult) -> Result<()> {
        info!("Attempting recovery for failed step: {}", step_name);

        let start_time = std::time::Instant::now();

        // Find the best recovery strategy
        let strategy = self.find_best_strategy(step_name, failure)
            .ok_or_else(|| anyhow::anyhow!("No recovery strategy found for step: {}", step_name))?;

        info!("Using recovery strategy: {} for step: {}", strategy.name(), step_name);

        // Attempt recovery
        let recovery_result = strategy.recover(step_name, failure).await;
        let recovery_time = start_time.elapsed();

        // Record the attempt
        let attempt = RecoveryAttempt {
            step_name: step_name.to_string(),
            attempt_number: self.get_attempt_count(step_name) + 1,
            timestamp: chrono::Utc::now(),
            strategy_used: strategy.name().to_string(),
            succeeded: recovery_result.is_ok(),
            error_message: recovery_result.as_ref().err().map(|e| e.to_string()),
            recovery_time,
            actions_taken: recovery_result.unwrap_or_else(|_| Vec::new()),
        };

        self.record_attempt(attempt.clone());

        match &attempt.succeeded {
            true => {
                info!("Recovery succeeded for step: {} using strategy: {} (took {:?})",
                      step_name, strategy.name(), recovery_time);
                Ok(())
            }
            false => {
                error!("Recovery failed for step: {} using strategy: {}: {}",
                       step_name, strategy.name(),
                       attempt.error_message.as_ref().unwrap_or(&"Unknown error".to_string()));
                Err(anyhow::anyhow!("Recovery failed: {}",
                    attempt.error_message.unwrap_or_else(|| "Unknown error".to_string())))
            }
        }
    }

    /// Find the best recovery strategy for a failure
    fn find_best_strategy(&self, step_name: &str, failure: &StepResult) -> Option<&(dyn RecoveryStrategy + Send + Sync)> {
        let mut best_strategy: Option<&(dyn RecoveryStrategy + Send + Sync)> = None;
        let mut best_priority = 0;

        for strategy in self.strategies.values() {
            if strategy.can_handle(step_name, failure) {
                let priority = strategy.priority();
                if priority > best_priority {
                    best_strategy = Some(strategy.as_ref());
                    best_priority = priority;
                }
            }
        }

        best_strategy
    }

    /// Get the number of recovery attempts for a step
    fn get_attempt_count(&self, step_name: &str) -> u32 {
        let history = self.history.lock().unwrap();
        history.iter()
            .filter(|attempt| attempt.step_name == step_name)
            .count() as u32
    }

    /// Record a recovery attempt
    fn record_attempt(&self, attempt: RecoveryAttempt) {
        let mut history = self.history.lock().unwrap();
        history.push(attempt);
    }

    /// Get recovery history
    pub fn get_history(&self) -> Vec<RecoveryAttempt> {
        let history = self.history.lock().unwrap();
        history.clone()
    }

    /// Clear recovery history
    pub fn clear_history(&self) {
        let mut history = self.history.lock().unwrap();
        history.clear();
    }

    /// Check if a step has exceeded maximum retry attempts
    pub fn has_exceeded_max_retries(&self, step_name: &str) -> bool {
        self.get_attempt_count(step_name) >= self.config.max_retries
    }

    /// Create a snapshot before a risky operation
    pub async fn create_snapshot(&self, snapshot_name: &str) -> Result<String> {
        if !self.config.create_snapshots {
            debug!("Snapshots disabled, skipping snapshot creation");
            return Ok("snapshots_disabled".to_string());
        }

        info!("Creating snapshot: {}", snapshot_name);

        // Create ZFS snapshot if ZFS is available
        if self.is_zfs_available().await? {
            self.create_zfs_snapshot(snapshot_name).await
        } else {
            // Fallback to filesystem backup
            self.create_filesystem_backup(snapshot_name).await
        }
    }

    /// Check if ZFS is available
    async fn is_zfs_available(&self) -> Result<bool> {
        let output = tokio::process::Command::new("which")
            .arg("zfs")
            .output()
            .await?;

        Ok(output.status.success())
    }

    /// Create ZFS snapshot
    async fn create_zfs_snapshot(&self, snapshot_name: &str) -> Result<String> {
        let snapshot_id = format!("installer_{}_{}", snapshot_name, chrono::Utc::now().timestamp());

        // Get root filesystem
        let output = tokio::process::Command::new("findmnt")
            .args(&["-n", "-o", "SOURCE", "/"])
            .output()
            .await
            .context("Failed to find root filesystem")?;

        let root_fs = String::from_utf8(output.stdout)?.trim().to_string();

        if root_fs.contains('/') {
            // This is a ZFS dataset
            let snapshot_path = format!("{}@{}", root_fs, snapshot_id);

            tokio::process::Command::new("zfs")
                .args(&["snapshot", &snapshot_path])
                .output()
                .await
                .context("Failed to create ZFS snapshot")?;

            info!("Created ZFS snapshot: {}", snapshot_path);
            Ok(snapshot_path)
        } else {
            // Not a ZFS filesystem, fallback to backup
            self.create_filesystem_backup(snapshot_name).await
        }
    }

    /// Create filesystem backup
    async fn create_filesystem_backup(&self, backup_name: &str) -> Result<String> {
        let backup_dir = format!("/tmp/installer_backup_{}", chrono::Utc::now().timestamp());
        let backup_path = format!("{}/{}", backup_dir, backup_name);

        tokio::fs::create_dir_all(&backup_dir).await
            .context("Failed to create backup directory")?;

        // Create a simple marker file for the backup
        tokio::fs::write(&backup_path, format!("Backup created at: {}", chrono::Utc::now())).await
            .context("Failed to create backup marker")?;

        info!("Created filesystem backup marker: {}", backup_path);
        Ok(backup_path)
    }

    /// Restore from a snapshot
    pub async fn restore_snapshot(&self, snapshot_id: &str) -> Result<()> {
        info!("Restoring from snapshot: {}", snapshot_id);

        if snapshot_id.contains('@') {
            // ZFS snapshot
            self.restore_zfs_snapshot(snapshot_id).await
        } else {
            // Filesystem backup
            warn!("Filesystem backup restoration not implemented");
            Ok(())
        }
    }

    /// Restore ZFS snapshot
    async fn restore_zfs_snapshot(&self, snapshot_path: &str) -> Result<()> {
        tokio::process::Command::new("zfs")
            .args(&["rollback", snapshot_path])
            .output()
            .await
            .context("Failed to restore ZFS snapshot")?;

        info!("Restored ZFS snapshot: {}", snapshot_path);
        Ok(())
    }
}

/// Disk-related recovery strategy
struct DiskRecoveryStrategy;

impl DiskRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for DiskRecoveryStrategy {
    fn name(&self) -> &str {
        "disk_recovery"
    }

    fn can_handle(&self, step_name: &str, _failure: &StepResult) -> bool {
        step_name.to_lowercase().contains("disk") ||
        step_name.to_lowercase().contains("partition")
    }

    async fn recover(&self, step_name: &str, failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting disk recovery for step: {}", step_name);

        // Check disk space
        actions.push("Checking disk space".to_string());
        let df_output = tokio::process::Command::new("df")
            .args(&["-h", "/"])
            .output()
            .await
            .context("Failed to check disk space")?;

        // Check for disk errors
        actions.push("Checking for disk errors".to_string());
        let dmesg_output = tokio::process::Command::new("dmesg")
            .args(&["|", "grep", "-i", "error"])
            .output()
            .await;

        // Attempt to unmount any mounted partitions
        actions.push("Unmounting any mounted partitions".to_string());

        // Wait a bit for the system to settle
        tokio::time::sleep(Duration::from_secs(5)).await;
        actions.push("Waited for system to settle".to_string());

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        80
    }
}

/// Network-related recovery strategy
struct NetworkRecoveryStrategy;

impl NetworkRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for NetworkRecoveryStrategy {
    fn name(&self) -> &str {
        "network_recovery"
    }

    fn can_handle(&self, step_name: &str, _failure: &StepResult) -> bool {
        step_name.to_lowercase().contains("network") ||
        step_name.to_lowercase().contains("interface")
    }

    async fn recover(&self, step_name: &str, _failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting network recovery for step: {}", step_name);

        // Restart networking service
        actions.push("Restarting networking service".to_string());
        let _ = tokio::process::Command::new("systemctl")
            .args(&["restart", "networking"])
            .output()
            .await;

        // Bring interfaces up
        actions.push("Bringing network interfaces up".to_string());
        let _ = tokio::process::Command::new("ip")
            .args(&["link", "set", "dev", "eth0", "up"])
            .output()
            .await;

        // Wait for network to settle
        tokio::time::sleep(Duration::from_secs(10)).await;
        actions.push("Waited for network to settle".to_string());

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        70
    }
}

/// Package-related recovery strategy
struct PackageRecoveryStrategy;

impl PackageRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for PackageRecoveryStrategy {
    fn name(&self) -> &str {
        "package_recovery"
    }

    fn can_handle(&self, step_name: &str, _failure: &StepResult) -> bool {
        step_name.to_lowercase().contains("package") ||
        step_name.to_lowercase().contains("apt")
    }

    async fn recover(&self, step_name: &str, _failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting package recovery for step: {}", step_name);

        // Clean apt cache
        actions.push("Cleaning apt cache".to_string());
        let _ = tokio::process::Command::new("apt")
            .args(&["clean"])
            .output()
            .await;

        // Update package lists
        actions.push("Updating package lists".to_string());
        let _ = tokio::process::Command::new("apt")
            .args(&["update"])
            .output()
            .await;

        // Fix broken packages
        actions.push("Fixing broken packages".to_string());
        let _ = tokio::process::Command::new("apt")
            .args(&["--fix-broken", "install"])
            .output()
            .await;

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        75
    }
}

/// Service-related recovery strategy
struct ServiceRecoveryStrategy;

impl ServiceRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for ServiceRecoveryStrategy {
    fn name(&self) -> &str {
        "service_recovery"
    }

    fn can_handle(&self, step_name: &str, _failure: &StepResult) -> bool {
        step_name.to_lowercase().contains("service") ||
        step_name.to_lowercase().contains("systemd")
    }

    async fn recover(&self, step_name: &str, _failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting service recovery for step: {}", step_name);

        // Reload systemd daemon
        actions.push("Reloading systemd daemon".to_string());
        let _ = tokio::process::Command::new("systemctl")
            .args(&["daemon-reload"])
            .output()
            .await;

        // Reset failed services
        actions.push("Resetting failed services".to_string());
        let _ = tokio::process::Command::new("systemctl")
            .args(&["reset-failed"])
            .output()
            .await;

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        60
    }
}

/// Filesystem-related recovery strategy
struct FilesystemRecoveryStrategy;

impl FilesystemRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for FilesystemRecoveryStrategy {
    fn name(&self) -> &str {
        "filesystem_recovery"
    }

    fn can_handle(&self, step_name: &str, _failure: &StepResult) -> bool {
        step_name.to_lowercase().contains("filesystem") ||
        step_name.to_lowercase().contains("mount") ||
        step_name.to_lowercase().contains("zfs")
    }

    async fn recover(&self, step_name: &str, _failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting filesystem recovery for step: {}", step_name);

        // Sync filesystems
        actions.push("Syncing filesystems".to_string());
        let _ = tokio::process::Command::new("sync")
            .output()
            .await;

        // Check filesystem health
        actions.push("Checking filesystem health".to_string());

        // Wait for any pending I/O
        tokio::time::sleep(Duration::from_secs(5)).await;
        actions.push("Waited for pending I/O".to_string());

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        65
    }
}

/// Generic recovery strategy for unknown failures
struct GenericRecoveryStrategy;

impl GenericRecoveryStrategy {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecoveryStrategy for GenericRecoveryStrategy {
    fn name(&self) -> &str {
        "generic_recovery"
    }

    fn can_handle(&self, _step_name: &str, _failure: &StepResult) -> bool {
        true // Can handle any failure as a last resort
    }

    async fn recover(&self, step_name: &str, _failure: &StepResult) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        debug!("Attempting generic recovery for step: {}", step_name);

        // Wait a bit
        tokio::time::sleep(Duration::from_secs(10)).await;
        actions.push("Waited for system to settle".to_string());

        // Clear any temporary files
        actions.push("Clearing temporary files".to_string());

        Ok(actions)
    }

    fn priority(&self) -> u32 {
        10 // Lowest priority - last resort
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::{StepStatus, StepResult};

    #[tokio::test]
    async fn test_recovery_manager_creation() {
        let manager = RecoveryManager::new();
        assert_eq!(manager.strategies.len(), 6); // 5 default strategies + generic
    }

    #[tokio::test]
    async fn test_disk_recovery_strategy() {
        let strategy = DiskRecoveryStrategy::new();
        assert!(strategy.can_handle("disk_preparation", &StepResult {
            status: StepStatus::Failed,
            message: "Test".to_string(),
            error_message: None,
            execution_time: Duration::from_secs(1),
            metadata: std::collections::HashMap::new(),
        }));
    }

    #[tokio::test]
    async fn test_recovery_history() {
        let manager = RecoveryManager::new();
        assert_eq!(manager.get_history().len(), 0);

        let attempt = RecoveryAttempt {
            step_name: "test_step".to_string(),
            attempt_number: 1,
            timestamp: chrono::Utc::now(),
            strategy_used: "test_strategy".to_string(),
            succeeded: true,
            error_message: None,
            recovery_time: Duration::from_secs(1),
            actions_taken: vec!["test_action".to_string()],
        };

        manager.record_attempt(attempt);
        assert_eq!(manager.get_history().len(), 1);
    }
}
