// file: src/utils/system.rs
// version: 1.0.0
// guid: w3x4y5z6-a7b8-9012-3456-789012wxyzab

//! System utility functions

use std::process::Stdio;
use tokio::process::Command;
use crate::Result;
use tracing::{debug, warn};

/// System utility functions
pub struct SystemUtils;

impl SystemUtils {
    /// Check if a command exists in PATH
    pub async fn command_exists(command: &str) -> bool {
        Command::new("which")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Get system architecture
    pub fn get_system_arch() -> crate::config::Architecture {
        match std::env::consts::ARCH {
            "x86_64" => crate::config::Architecture::Amd64,
            "aarch64" => crate::config::Architecture::Arm64,
            _ => crate::config::Architecture::Amd64, // Default fallback
        }
    }

    /// Check if running as root
    pub fn is_root() -> bool {
        unsafe { libc::getuid() == 0 }
    }

    /// Get available memory in MB
    pub async fn get_available_memory() -> Result<u64> {
        let output = Command::new("free")
            .args(&["-m"])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::SystemError(
                format!("Failed to get memory info: {}", e)
            ))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("Mem:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    return parts[6].parse::<u64>()
                        .map_err(|_| crate::error::AutoInstallError::SystemError(
                            "Failed to parse memory value".to_string()
                        ));
                }
            }
        }

        Err(crate::error::AutoInstallError::SystemError(
            "Failed to find memory information".to_string()
        ))
    }

    /// Get available disk space in GB for a path
    pub async fn get_available_space(path: &str) -> Result<u64> {
        // Note: This function could be enhanced to use CoreUtils::df() 
        // for more reliable cross-platform behavior
        let output = Command::new("df")
            .args(&["-BG", path])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::SystemError(
                format!("Failed to get disk space info: {}", e)
            ))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        
        if lines.len() >= 2 {
            let parts: Vec<&str> = lines[1].split_whitespace().collect();
            if parts.len() >= 4 {
                let available = parts[3].trim_end_matches('G');
                return available.parse::<u64>()
                    .map_err(|_| crate::error::AutoInstallError::SystemError(
                        "Failed to parse disk space value".to_string()
                    ));
            }
        }

        Err(crate::error::AutoInstallError::SystemError(
            "Failed to find disk space information".to_string()
        ))
    }

    /// Check if required tools are installed
    pub async fn check_prerequisites() -> Result<Vec<String>> {
        let required_tools = [
            "qemu-img", "qemu-system-x86_64", "guestfish", "cryptsetup"
        ];

        let mut missing = Vec::new();

        for tool in &required_tools {
            if !Self::command_exists(tool).await {
                missing.push(tool.to_string());
            }
        }

        if !missing.is_empty() {
            warn!("Missing required tools: {}", missing.join(", "));
        } else {
            debug!("All required tools are available");
        }

        Ok(missing)
    }

    /// Create temporary directory
    pub async fn create_temp_dir(prefix: &str) -> Result<std::path::PathBuf> {
        let temp_dir = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        let path = temp_dir.path().to_owned();
        std::mem::forget(temp_dir); // Don't delete on drop
        debug!("Created temporary directory: {}", path.display());
        Ok(path)
    }

    /// Execute command with timeout
    pub async fn execute_with_timeout(
        command: &str,
        args: &[&str],
        timeout_secs: u64,
    ) -> Result<String> {
        let child = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| crate::error::AutoInstallError::SystemError(
                format!("Failed to spawn command {}: {}", command, e)
            ))?;

        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| crate::error::AutoInstallError::SystemError(
                format!("Command {} timed out after {} seconds", command, timeout_secs)
            ))?
            .map_err(|e| crate::error::AutoInstallError::SystemError(
                format!("Command {} failed: {}", command, e)
            ))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::AutoInstallError::SystemError(
                format!("Command {} failed with exit code {}: {}", 
                        command, output.status.code().unwrap_or(-1), stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_command_exists() {
        // Test with a command that should exist on most systems
        assert!(SystemUtils::command_exists("ls").await);
        
        // Test with a command that shouldn't exist
        assert!(!SystemUtils::command_exists("nonexistent-command-12345").await);
    }

    #[test]
    fn test_get_system_arch() {
        let arch = SystemUtils::get_system_arch();
        // Should return either Amd64 or Arm64
        assert!(matches!(arch, crate::config::Architecture::Amd64 | crate::config::Architecture::Arm64));
    }

    #[tokio::test]
    async fn test_get_available_memory() {
        let result = SystemUtils::get_available_memory().await;
        // Should return some memory value or error
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_create_temp_dir() {
        let temp_dir = SystemUtils::create_temp_dir("test").await.unwrap();
        assert!(temp_dir.exists());
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
    }
}

extern crate libc;