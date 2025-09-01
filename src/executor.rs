// file: src/executor.rs
// version: 2.0.0
// guid: bb371682-35cb-4f34-b318-8bf69ec125bd

use crate::config::Config;
use crate::security::{SecurityManager, audit};
use crate::error::{AgentError, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

/// Safe command executor with comprehensive security controls
pub struct Executor {
    config: Config,
    security: SecurityManager,
}

impl Executor {
    /// Create a new executor with the given configuration
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize security audit system
        audit::initialize_audit_system()
            .map_err(|e| AgentError::system(format!("Failed to initialize audit system: {}", e)))?;

        let security = SecurityManager::new();

        // Log the security configuration
        info!("Executor initialized with security controls enabled");
        info!("Security stats: {:?}", security.get_allowed_commands().len());

        Ok(Self { config, security })
    }

    /// Execute a command with full security validation
    pub async fn execute_secure<T: AsRef<str>>(&self, command: &str, args: &[T]) -> anyhow::Result<()> {
        // Validate execution context first
        self.security.validate_execution_context().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Convert args to String vector for security validation
        let string_args: Vec<String> = args.iter().map(|s| s.as_ref().to_string()).collect();

        // Validate and sanitize the command and arguments
        let sanitized_args = self.security.validate_arguments(command, &string_args).map_err(|e| anyhow::anyhow!("{}", e))?;

        info!(
            "Executing secure command: {} with sanitized args: {:?}",
            command, sanitized_args
        );

        if self.config.safety.dry_run {
            println!("DRY RUN: Would execute: {} {:?}", command, sanitized_args);
            audit::log_command_execution(command, &sanitized_args);
            return Ok(());
        }

        // Validate command exists
        if which::which(command).is_err() {
            let error_msg = format!("Command not found: {}", command);
            audit::log_security_violation(command, &string_args, &error_msg);
            return Err(anyhow::anyhow!("{}", error_msg));
        }

        // Execute command with security monitoring
        self.execute_command_impl(command, &sanitized_args).await.map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Execute a raw command with arguments (DEPRECATED - use execute_secure instead)
    #[deprecated(since = "2.0.0", note = "Use execute_secure instead for better security")]
    pub async fn execute_raw(&self, args: &[&str]) -> Result<()> {
        warn!("DEPRECATED: execute_raw called - consider migrating to execute_secure");

        if args.is_empty() {
            return Err(AgentError::validation("No command provided"));
        }

        let command = args[0];
        let string_args: Vec<String> = args[1..].iter().map(|s| s.to_string()).collect();

        // Route through secure execution
        self.execute_secure(command, &string_args).await
            .map_err(|e| AgentError::execution(e.to_string()))
    }

    /// Internal implementation of command execution
    async fn execute_command_impl(&self, command: &str, args: &[String]) -> Result<()> {
        // Create command
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Set working directory if specified
        if let Some(ref wd) = self.config.general.working_directory {
            cmd.current_dir(wd);
        }

        // Set environment variables with security filtering
        self.set_secure_environment(&mut cmd)?;

        // Execute with timeout
        let status = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.general.timeout_seconds),
            cmd.status()
        )
        .await
        .map_err(|_| AgentError::timeout("Command execution timed out"))?
        .map_err(|e| AgentError::execution(format!("Failed to execute command: {}", e)))?;

        if !status.success() {
            let error_msg = format!(
                "Command failed with exit code: {:?}",
                status.code()
            );
            audit::log_security_violation(command, args, &error_msg);
            return Err(AgentError::execution(error_msg));
        }

        audit::log_command_execution(command, args);
        info!("Command executed successfully");
        Ok(())
    }

    /// Set environment variables with security filtering
    fn set_secure_environment(&self, cmd: &mut Command) -> Result<()> {
        // Remove potentially dangerous environment variables
        let dangerous_vars = [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "PYTHONPATH", // Can be dangerous if not carefully managed
        ];

        for var in &dangerous_vars {
            if std::env::var(var).is_ok() {
                warn!("Removing dangerous environment variable: {}", var);
                cmd.env_remove(var);
            }
        }

        // Ensure PATH is clean and doesn't contain suspicious entries
        if let Ok(path) = std::env::var("PATH") {
            let clean_path = self.sanitize_path(&path)?;
            if clean_path != path {
                info!("Sanitized PATH environment variable");
                cmd.env("PATH", clean_path);
            }
        }

        Ok(())
    }

    /// Sanitize PATH environment variable
    fn sanitize_path(&self, path: &str) -> Result<String> {
        let entries: Vec<&str> = path.split(':').collect();
        let mut clean_entries = Vec::new();

        for entry in entries {
            // Skip suspicious path entries
            if entry.contains("..") || entry.contains(";") || entry.contains("&&") {
                warn!("Skipping suspicious PATH entry: {}", entry);
                audit::log_suspicious_activity(
                    "Suspicious PATH entry detected",
                    &[entry.to_string()]
                );
                continue;
            }

            // Skip relative paths that could be dangerous
            if !entry.starts_with('/') && !entry.is_empty() {
                warn!("Skipping relative PATH entry: {}", entry);
                continue;
            }

            clean_entries.push(entry);
        }

        Ok(clean_entries.join(":"))
    }

    /// Get security manager for advanced operations
    pub fn security(&self) -> &SecurityManager {
        &self.security
    }

    /// Get mutable security manager (use with caution)
    pub fn security_mut(&mut self) -> &mut SecurityManager {
        &mut self.security
    }
}
