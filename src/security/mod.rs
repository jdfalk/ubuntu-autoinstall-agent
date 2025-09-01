// file: src/security/mod.rs
// version: 1.0.0
// guid: a1b2c3d4-e5f6-7890-abcd-ef1234567890

//! Security module for the Copilot Agent Utility
//!
//! This module provides comprehensive security controls to prevent abuse of the utility
//! for remote code execution or other malicious activities.

pub mod allowlist;
pub mod sanitizer;
pub mod validator;
pub mod audit;

use crate::error::{AgentError, Result};
use std::collections::HashSet;
use tracing::{info, warn};

/// Security configuration and enforcement
#[derive(Debug, Clone)]
pub struct SecurityManager {
    allowed_commands: HashSet<String>,
    audit_enabled: bool,
    strict_mode: bool,
}

impl SecurityManager {
    /// Create a new security manager with default safe configuration
    pub fn new() -> Self {
        Self {
            allowed_commands: Self::default_allowed_commands(),
            audit_enabled: true,
            strict_mode: true,
        }
    }

    /// Get the default set of allowed commands
    fn default_allowed_commands() -> HashSet<String> {
        let mut commands = HashSet::new();

        // Git operations
        commands.insert("git".to_string());

        // Protocol buffers
        commands.insert("buf".to_string());

        // Go development
        commands.insert("go".to_string());

        // Rust development
        commands.insert("cargo".to_string());
        commands.insert("rustc".to_string());
        commands.insert("clippy".to_string());
        commands.insert("rustfmt".to_string());

        // Python development
        commands.insert("python".to_string());
        commands.insert("python3".to_string());
        commands.insert("pip".to_string());
        commands.insert("pip3".to_string());

        // Node.js development
        commands.insert("node".to_string());
        commands.insert("npm".to_string());
        commands.insert("yarn".to_string());
        commands.insert("pnpm".to_string());

        // Safe file operations (uutils implementations)
        commands.insert("ls".to_string());
        commands.insert("cat".to_string());
        commands.insert("cp".to_string());
        commands.insert("mv".to_string());
        commands.insert("rm".to_string());
        commands.insert("mkdir".to_string());
        commands.insert("find".to_string());
        commands.insert("grep".to_string());

        // Development tools
        commands.insert("make".to_string());
        commands.insert("cmake".to_string());
        commands.insert("docker".to_string());
        commands.insert("kubectl".to_string());

        // Linting and formatting
        commands.insert("eslint".to_string());
        commands.insert("prettier".to_string());
        commands.insert("black".to_string());
        commands.insert("flake8".to_string());
        commands.insert("mypy".to_string());
        commands.insert("golangci-lint".to_string());
        commands.insert("shellcheck".to_string());
        commands.insert("hadolint".to_string());
        commands.insert("yamllint".to_string());
        commands.insert("markdownlint".to_string());

        commands
    }

    /// Check if a command is allowed to execute
    pub fn is_command_allowed(&self, command: &str) -> bool {
        let result = self.allowed_commands.contains(command);

        if self.audit_enabled {
            if result {
                info!("Security check PASSED for command: {}", command);
            } else {
                warn!("Security check FAILED for command: {} (not in allowlist)", command);
            }
        }

        result
    }

    /// Validate and sanitize command arguments
    pub fn validate_arguments(&self, command: &str, args: &[String]) -> Result<Vec<String>> {
        if !self.is_command_allowed(command) {
            return Err(AgentError::security(
                format!("Command '{}' is not allowed for security reasons", command)
            ));
        }

        let sanitized_args = sanitizer::sanitize_arguments(command, args)?;
        validator::validate_command_arguments(command, &sanitized_args)?;

        if self.audit_enabled {
            audit::log_command_execution(command, &sanitized_args);
        }

        Ok(sanitized_args)
    }

    /// Check for potential security risks in the execution context
    pub fn validate_execution_context(&self) -> Result<()> {
        // Check for suspicious environment variables
        if let Ok(value) = std::env::var("LD_PRELOAD") {
            warn!("Suspicious LD_PRELOAD detected: {}", value);
            if self.strict_mode {
                return Err(AgentError::security("LD_PRELOAD detected in strict mode"));
            }
        }

        // Check for shell injection attempts in PATH
        if let Ok(path) = std::env::var("PATH") {
            if path.contains("..") || path.contains(";") || path.contains("&&") {
                warn!("Suspicious PATH modification detected: {}", path);
                if self.strict_mode {
                    return Err(AgentError::security("Suspicious PATH detected in strict mode"));
                }
            }
        }

        Ok(())
    }

    /// Enable or disable strict security mode
    pub fn set_strict_mode(&mut self, enabled: bool) {
        self.strict_mode = enabled;
        info!("Strict security mode {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Add a command to the allowlist (use with caution)
    pub fn add_allowed_command(&mut self, command: String) -> Result<()> {
        if self.strict_mode {
            warn!("Attempt to add command '{}' in strict mode - rejected", command);
            return Err(AgentError::security("Cannot modify allowlist in strict mode"));
        }

        self.allowed_commands.insert(command.clone());
        warn!("Added command '{}' to allowlist", command);
        Ok(())
    }

    /// Get list of allowed commands (for debugging/audit purposes)
    pub fn get_allowed_commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = self.allowed_commands.iter().cloned().collect();
        commands.sort();
        commands
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_commands() {
        let security = SecurityManager::new();

        // These should be allowed
        assert!(security.is_command_allowed("git"));
        assert!(security.is_command_allowed("buf"));
        assert!(security.is_command_allowed("cargo"));
        assert!(security.is_command_allowed("go"));

        // These should NOT be allowed
        assert!(!security.is_command_allowed("bash"));
        assert!(!security.is_command_allowed("sh"));
        assert!(!security.is_command_allowed("zsh"));
        assert!(!security.is_command_allowed("cmd"));
        assert!(!security.is_command_allowed("powershell"));
        assert!(!security.is_command_allowed("curl"));
        assert!(!security.is_command_allowed("wget"));
        assert!(!security.is_command_allowed("nc"));
        assert!(!security.is_command_allowed("netcat"));
        assert!(!security.is_command_allowed("ssh"));
        assert!(!security.is_command_allowed("scp"));
        assert!(!security.is_command_allowed("rsync"));
        assert!(!security.is_command_allowed("sudo"));
        assert!(!security.is_command_allowed("su"));
        assert!(!security.is_command_allowed("chmod"));
        assert!(!security.is_command_allowed("chown"));
    }

    #[test]
    fn test_strict_mode() {
        let mut security = SecurityManager::new();

        // Should reject adding commands in strict mode
        assert!(security.add_allowed_command("dangerous_command".to_string()).is_err());

        // Disable strict mode and try again
        security.set_strict_mode(false);
        assert!(security.add_allowed_command("dangerous_command".to_string()).is_ok());
        assert!(security.is_command_allowed("dangerous_command"));
    }
}
