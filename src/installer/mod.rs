// file: src/installer/mod.rs
// version: 1.0.0
// guid: g7h8i9j0-k1l2-3456-7890-abcdef123456

use crate::config::InstallConfig;
use crate::recovery::RecoveryManager;
use crate::reporter::StatusReporter;
use crate::steps::{InstallStep, StepContext, StepResult, StepStatus};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

pub mod disk;
pub mod network;
pub mod packages;
pub mod services;
pub mod zfs;
pub mod encryption;
pub mod users;

/// Main installer orchestrator
pub struct Installer {
    /// Installation configuration
    config: InstallConfig,

    /// Current installation session ID
    session_id: Uuid,

    /// Status reporter for progress updates
    reporter: Arc<StatusReporter>,

    /// Recovery manager for handling failures
    recovery: Arc<RecoveryManager>,

    /// Current installation state
    state: Arc<RwLock<InstallerState>>,

    /// Installation steps to execute
    steps: Vec<Box<dyn InstallStep + Send + Sync>>,
}

/// Current state of the installation process
#[derive(Debug, Clone)]
pub struct InstallerState {
    /// Current step being executed
    pub current_step: usize,

    /// Total number of steps
    pub total_steps: usize,

    /// Overall installation status
    pub status: InstallationStatus,

    /// Timestamp when installation started
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Timestamp when installation completed/failed
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Error message if installation failed
    pub error_message: Option<String>,

    /// Progress percentage (0-100)
    pub progress: u8,

    /// Currently executing step name
    pub current_step_name: String,

    /// Results from completed steps
    pub step_results: Vec<StepResult>,
}

/// Overall installation status
#[derive(Debug, Clone, PartialEq)]
pub enum InstallationStatus {
    /// Installation is preparing
    Preparing,

    /// Installation is in progress
    Running,

    /// Installation completed successfully
    Completed,

    /// Installation failed
    Failed,

    /// Installation was cancelled
    Cancelled,

    /// Installation is being recovered
    Recovering,
}

impl Installer {
    /// Create a new installer instance
    pub async fn new(
        config: InstallConfig,
        reporter: Arc<StatusReporter>,
        recovery: Arc<RecoveryManager>,
    ) -> Result<Self> {
        let session_id = Uuid::new_v4();
        info!("Creating new installer session: {}", session_id);

        let state = Arc::new(RwLock::new(InstallerState {
            current_step: 0,
            total_steps: 0,
            status: InstallationStatus::Preparing,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error_message: None,
            progress: 0,
            current_step_name: "Initializing".to_string(),
            step_results: Vec::new(),
        }));

        let installer = Self {
            config,
            session_id,
            reporter,
            recovery,
            state,
            steps: Vec::new(),
        };

        Ok(installer)
    }

    /// Initialize the installer with the appropriate steps
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing installer with configuration");

        // Build the list of installation steps based on configuration
        self.build_step_sequence().await?;

        // Update total steps count
        {
            let mut state = self.state.write().await;
            state.total_steps = self.steps.len();
            state.current_step_name = "Ready to start".to_string();
        }

        info!("Installer initialized with {} steps", self.steps.len());
        Ok(())
    }

    /// Build the sequence of installation steps based on configuration
    async fn build_step_sequence(&mut self) -> Result<()> {
        debug!("Building installation step sequence");

        // 1. Disk preparation
        self.steps.push(Box::new(disk::DiskPreparationStep::new(&self.config)?));

        // 2. ZFS setup (if enabled)
        if !self.config.zfs.root_pool.is_empty() {
            self.steps.push(Box::new(zfs::ZfsSetupStep::new(&self.config)?));
        }

        // 3. Encryption setup (if enabled)
        if self.config.encryption.enabled {
            self.steps.push(Box::new(encryption::EncryptionSetupStep::new(&self.config)?));
        }

        // 4. Network configuration
        self.steps.push(Box::new(network::NetworkSetupStep::new(&self.config)?));

        // 5. Package installation
        if !self.config.packages.is_empty() {
            self.steps.push(Box::new(packages::PackageInstallationStep::new(&self.config)?));
        }

        // 6. User creation
        if !self.config.users.is_empty() {
            self.steps.push(Box::new(users::UserCreationStep::new(&self.config)?));
        }

        // 7. Service configuration
        if !self.config.services.is_empty() {
            self.steps.push(Box::new(services::ServiceConfigurationStep::new(&self.config)?));
        }

        // 8. Custom scripts execution
        if !self.config.custom_scripts.is_empty() {
            // Add custom script steps for each stage
            for (stage, scripts) in &self.config.custom_scripts {
                if !scripts.is_empty() {
                    self.steps.push(Box::new(CustomScriptStep::new(stage, scripts)?));
                }
            }
        }

        debug!("Built sequence with {} steps", self.steps.len());
        Ok(())
    }

    /// Start the installation process
    pub async fn start_installation(&self) -> Result<()> {
        info!("Starting installation for session: {}", self.session_id);

        // Update state to running
        {
            let mut state = self.state.write().await;
            state.status = InstallationStatus::Running;
            state.started_at = chrono::Utc::now();
        }

        // Report start
        self.reporter.report_installation_started(self.session_id).await?;

        // Execute all steps
        let result = self.execute_steps().await;

        // Update final state
        {
            let mut state = self.state.write().await;
            state.completed_at = Some(chrono::Utc::now());

            match &result {
                Ok(_) => {
                    state.status = InstallationStatus::Completed;
                    state.progress = 100;
                    state.current_step_name = "Installation completed".to_string();
                    info!("Installation completed successfully");
                }
                Err(e) => {
                    state.status = InstallationStatus::Failed;
                    state.error_message = Some(e.to_string());
                    state.current_step_name = "Installation failed".to_string();
                    error!("Installation failed: {}", e);
                }
            }
        }

        // Report completion
        match &result {
            Ok(_) => self.reporter.report_installation_completed(self.session_id).await?,
            Err(e) => self.reporter.report_installation_failed(self.session_id, e).await?,
        }

        result
    }

    /// Execute all installation steps
    async fn execute_steps(&self) -> Result<()> {
        info!("Executing {} installation steps", self.steps.len());

        for (index, step) in self.steps.iter().enumerate() {
            // Update current step state
            {
                let mut state = self.state.write().await;
                state.current_step = index;
                state.current_step_name = step.name().to_string();
                state.progress = ((index as f32 / self.steps.len() as f32) * 100.0) as u8;
            }

            info!("Executing step {}/{}: {}", index + 1, self.steps.len(), step.name());

            // Create step context
            let context = StepContext {
                session_id: self.session_id,
                config: &self.config,
                step_number: index + 1,
                total_steps: self.steps.len(),
            };

            // Report step start
            self.reporter.report_step_started(step.name()).await?;

            // Execute step with recovery
            let step_result = self.execute_step_with_recovery(step.as_ref(), &context).await;

            // Store step result
            {
                let mut state = self.state.write().await;
                state.step_results.push(step_result.clone());
            }

            // Report step completion
            match &step_result.status {
                StepStatus::Completed => {
                    self.reporter.report_step_completed(step.name()).await?;
                    info!("Step completed: {}", step.name());
                }
                StepStatus::Failed => {
                    self.reporter.report_step_failed(step.name(), &step_result.error_message.as_ref().unwrap_or(&"Unknown error".to_string())).await?;
                    error!("Step failed: {} - {}", step.name(), step_result.error_message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    return Err(anyhow::anyhow!("Installation step failed: {}", step.name()));
                }
                StepStatus::Skipped => {
                    self.reporter.report_step_skipped(step.name()).await?;
                    info!("Step skipped: {}", step.name());
                }
                _ => {}
            }
        }

        info!("All installation steps completed successfully");
        Ok(())
    }

    /// Execute a single step with recovery support
    async fn execute_step_with_recovery(
        &self,
        step: &dyn InstallStep,
        context: &StepContext<'_>,
    ) -> StepResult {
        let mut attempts = 0;
        let max_retries = self.config.recovery.max_retries;

        loop {
            attempts += 1;
            debug!("Executing step '{}' (attempt {}/{})", step.name(), attempts, max_retries + 1);

            // Execute the step
            let result = step.execute(context).await;

            match &result.status {
                StepStatus::Completed => {
                    return result;
                }
                StepStatus::Failed => {
                    if attempts <= max_retries && self.config.recovery.auto_recovery {
                        warn!("Step '{}' failed (attempt {}/{}), attempting recovery",
                              step.name(), attempts, max_retries + 1);

                        // Report recovery attempt
                        if let Err(e) = self.reporter.report_step_recovery_attempt(step.name(), attempts).await {
                            error!("Failed to report recovery attempt: {}", e);
                        }

                        // Attempt recovery
                        if let Err(e) = self.recovery.attempt_recovery(step.name(), &result).await {
                            error!("Recovery failed for step '{}': {}", step.name(), e);
                        }

                        // Wait before retry
                        tokio::time::sleep(tokio::time::Duration::from_secs(
                            self.config.recovery.retry_delay as u64
                        )).await;

                        continue;
                    } else {
                        error!("Step '{}' failed after {} attempts", step.name(), attempts);
                        return result;
                    }
                }
                _ => {
                    return result;
                }
            }
        }
    }

    /// Get current installation state
    pub async fn get_state(&self) -> InstallerState {
        self.state.read().await.clone()
    }

    /// Cancel the installation
    pub async fn cancel_installation(&self) -> Result<()> {
        warn!("Cancelling installation for session: {}", self.session_id);

        {
            let mut state = self.state.write().await;
            state.status = InstallationStatus::Cancelled;
            state.completed_at = Some(chrono::Utc::now());
            state.current_step_name = "Installation cancelled".to_string();
        }

        self.reporter.report_installation_cancelled(self.session_id).await?;

        Ok(())
    }

    /// Validate the installation before starting
    pub async fn validate_installation(&self) -> Result<Vec<String>> {
        info!("Validating installation configuration");

        let mut warnings = Vec::new();

        // Validate disk availability
        if let Err(e) = self.validate_disk().await {
            warnings.push(format!("Disk validation warning: {}", e));
        }

        // Validate network configuration
        if let Err(e) = self.validate_network().await {
            warnings.push(format!("Network validation warning: {}", e));
        }

        // Validate package repositories
        if let Err(e) = self.validate_repositories().await {
            warnings.push(format!("Repository validation warning: {}", e));
        }

        if warnings.is_empty() {
            info!("Installation validation completed successfully");
        } else {
            warn!("Installation validation completed with {} warnings", warnings.len());
        }

        Ok(warnings)
    }

    /// Validate disk configuration
    async fn validate_disk(&self) -> Result<()> {
        debug!("Validating disk configuration");

        // Check if disk exists
        let disk_path = std::path::Path::new(&self.config.disk);
        if !disk_path.exists() {
            anyhow::bail!("Disk device {} does not exist", self.config.disk);
        }

        // Check if disk is not mounted
        let output = tokio::process::Command::new("mount")
            .output()
            .await
            .context("Failed to check mounted filesystems")?;

        let mount_output = String::from_utf8(output.stdout)?;
        if mount_output.contains(&self.config.disk) {
            anyhow::bail!("Disk device {} is currently mounted", self.config.disk);
        }

        Ok(())
    }

    /// Validate network configuration
    async fn validate_network(&self) -> Result<()> {
        debug!("Validating network configuration");

        // Check if network interface exists
        let interfaces = network_interface::NetworkInterface::show()
            .context("Failed to get network interfaces")?;

        let interface_names: Vec<String> = interfaces.into_iter().map(|i| i.name).collect();

        if !interface_names.contains(&self.config.network.ethernet.interface) {
            anyhow::bail!("Network interface {} does not exist", self.config.network.ethernet.interface);
        }

        Ok(())
    }

    /// Validate package repositories
    async fn validate_repositories(&self) -> Result<()> {
        debug!("Validating package repositories");

        // Simple check - just verify apt is available
        let output = tokio::process::Command::new("apt")
            .arg("list")
            .arg("--installed")
            .arg("apt")
            .output()
            .await
            .context("Failed to check apt availability")?;

        if !output.status.success() {
            anyhow::bail!("Package manager (apt) is not available");
        }

        Ok(())
    }
}

/// Custom script execution step
#[derive(Debug)]
pub struct CustomScriptStep {
    stage: String,
    scripts: Vec<String>,
}

impl CustomScriptStep {
    pub fn new(stage: &str, scripts: &[String]) -> Result<Self> {
        Ok(Self {
            stage: stage.to_string(),
            scripts: scripts.to_vec(),
        })
    }
}

#[async_trait::async_trait]
impl InstallStep for CustomScriptStep {
    fn name(&self) -> &str {
        &format!("Custom Scripts ({})", self.stage)
    }

    fn description(&self) -> &str {
        "Execute custom scripts for this stage"
    }

    async fn execute(&self, context: &StepContext<'_>) -> StepResult {
        info!("Executing custom scripts for stage: {}", self.stage);

        for (index, script) in self.scripts.iter().enumerate() {
            info!("Executing script {}/{}: {}", index + 1, self.scripts.len(), script);

            let output = match tokio::process::Command::new("bash")
                .arg("-c")
                .arg(script)
                .output()
                .await
            {
                Ok(output) => output,
                Err(e) => {
                    return StepResult {
                        status: StepStatus::Failed,
                        message: format!("Failed to execute script: {}", e),
                        error_message: Some(e.to_string()),
                        execution_time: std::time::Duration::from_secs(0),
                        metadata: std::collections::HashMap::new(),
                    };
                }
            };

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return StepResult {
                    status: StepStatus::Failed,
                    message: format!("Script execution failed: {}", stderr),
                    error_message: Some(stderr.to_string()),
                    execution_time: std::time::Duration::from_secs(0),
                    metadata: std::collections::HashMap::new(),
                };
            }
        }

        StepResult {
            status: StepStatus::Completed,
            message: format!("Executed {} custom scripts successfully", self.scripts.len()),
            error_message: None,
            execution_time: std::time::Duration::from_secs(0),
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn validate(&self, _context: &StepContext<'_>) -> Result<()> {
        // Validate that scripts are not empty
        if self.scripts.is_empty() {
            anyhow::bail!("No scripts provided for stage: {}", self.stage);
        }

        for script in &self.scripts {
            if script.trim().is_empty() {
                anyhow::bail!("Empty script found in stage: {}", self.stage);
            }
        }

        Ok(())
    }

    async fn cleanup(&self, _context: &StepContext<'_>) -> Result<()> {
        // Custom scripts don't need cleanup
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstallConfig;

    #[tokio::test]
    async fn test_installer_creation() {
        let config = InstallConfig::default();
        let reporter = Arc::new(StatusReporter::new(None).unwrap());
        let recovery = Arc::new(RecoveryManager::new());

        let installer = Installer::new(config, reporter, recovery).await.unwrap();
        assert_eq!(installer.session_id.to_string().len(), 36); // UUID length
    }

    #[tokio::test]
    async fn test_step_sequence_building() {
        let config = InstallConfig::default();
        let reporter = Arc::new(StatusReporter::new(None).unwrap());
        let recovery = Arc::new(RecoveryManager::new());

        let mut installer = Installer::new(config, reporter, recovery).await.unwrap();
        installer.initialize().await.unwrap();

        let state = installer.get_state().await;
        assert!(state.total_steps > 0);
    }
}
