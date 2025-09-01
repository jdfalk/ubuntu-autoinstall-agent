// file: src/security/allowlist.rs
// version: 1.0.0
// guid: e5f6a7b8-c9d0-1234-ef56-567890123456

//! Command allowlist management module
//!
//! This module manages the allowlist of commands that can be executed by the utility.
//! It provides a centralized location for defining safe commands and their restrictions.

use crate::error::{AgentError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};

/// Command allowlist configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowlistConfig {
    /// Commands that are always allowed
    pub always_allowed: HashSet<String>,
    /// Commands that are conditionally allowed based on arguments
    pub conditionally_allowed: HashMap<String, CommandRestrictions>,
    /// Commands that are explicitly blocked
    pub blocked: HashSet<String>,
    /// Whether to allow unknown commands in permissive mode
    pub permissive_mode: bool,
}

/// Restrictions for a specific command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRestrictions {
    /// Maximum number of arguments allowed
    pub max_args: Option<usize>,
    /// Required arguments that must be present
    pub required_args: Vec<String>,
    /// Forbidden arguments that must not be present
    pub forbidden_args: Vec<String>,
    /// Allowed argument patterns (regex)
    pub allowed_patterns: Vec<String>,
    /// Forbidden argument patterns (regex)
    pub forbidden_patterns: Vec<String>,
    /// Whether this command requires elevated privileges
    pub requires_elevation: bool,
    /// Custom validation function name
    pub custom_validator: Option<String>,
}

impl Default for AllowlistConfig {
    fn default() -> Self {
        Self::secure_default()
    }
}

impl AllowlistConfig {
    /// Create a secure default allowlist configuration
    pub fn secure_default() -> Self {
        let mut config = Self {
            always_allowed: HashSet::new(),
            conditionally_allowed: HashMap::new(),
            blocked: Self::default_blocked_commands(),
            permissive_mode: false,
        };

        // Add safe commands
        config.add_safe_development_commands();
        config.add_safe_file_commands();
        config.add_conditional_commands();

        config
    }

    /// Get the default set of blocked commands
    fn default_blocked_commands() -> HashSet<String> {
        let mut blocked = HashSet::new();

        // Shell and interpreters
        blocked.insert("bash".to_string());
        blocked.insert("sh".to_string());
        blocked.insert("zsh".to_string());
        blocked.insert("fish".to_string());
        blocked.insert("csh".to_string());
        blocked.insert("tcsh".to_string());
        blocked.insert("ksh".to_string());
        blocked.insert("dash".to_string());
        blocked.insert("cmd".to_string());
        blocked.insert("powershell".to_string());
        blocked.insert("pwsh".to_string());

        // Network tools
        blocked.insert("curl".to_string());
        blocked.insert("wget".to_string());
        blocked.insert("nc".to_string());
        blocked.insert("netcat".to_string());
        blocked.insert("ncat".to_string());
        blocked.insert("socat".to_string());
        blocked.insert("telnet".to_string());
        blocked.insert("ftp".to_string());
        blocked.insert("sftp".to_string());
        blocked.insert("scp".to_string());
        blocked.insert("rsync".to_string());

        // System administration
        blocked.insert("sudo".to_string());
        blocked.insert("su".to_string());
        blocked.insert("doas".to_string());
        blocked.insert("pkexec".to_string());
        blocked.insert("systemctl".to_string());
        blocked.insert("service".to_string());
        blocked.insert("chroot".to_string());
        blocked.insert("mount".to_string());
        blocked.insert("umount".to_string());
        blocked.insert("fdisk".to_string());
        blocked.insert("mkfs".to_string());
        blocked.insert("fsck".to_string());

        // Process and system control
        blocked.insert("kill".to_string());
        blocked.insert("killall".to_string());
        blocked.insert("pkill".to_string());
        blocked.insert("reboot".to_string());
        blocked.insert("shutdown".to_string());
        blocked.insert("halt".to_string());
        blocked.insert("init".to_string());

        // Package managers (system-wide)
        blocked.insert("apt".to_string());
        blocked.insert("apt-get".to_string());
        blocked.insert("yum".to_string());
        blocked.insert("dnf".to_string());
        blocked.insert("pacman".to_string());
        blocked.insert("zypper".to_string());
        blocked.insert("emerge".to_string());
        blocked.insert("portage".to_string());
        blocked.insert("brew".to_string()); // Can be dangerous if system-wide

        // Database tools
        blocked.insert("mysql".to_string());
        blocked.insert("psql".to_string());
        blocked.insert("mongo".to_string());
        blocked.insert("redis-cli".to_string());
        blocked.insert("sqlite3".to_string());

        // Compilers and interpreters that can execute arbitrary code
        blocked.insert("gcc".to_string());
        blocked.insert("clang".to_string());
        blocked.insert("cc".to_string());
        blocked.insert("ld".to_string());
        blocked.insert("as".to_string());
        blocked.insert("nasm".to_string());
        blocked.insert("perl".to_string());
        blocked.insert("ruby".to_string());
        blocked.insert("php".to_string());
        blocked.insert("lua".to_string());
        blocked.insert("tcl".to_string());

        // Text editors that can execute commands
        blocked.insert("vim".to_string());
        blocked.insert("vi".to_string());
        blocked.insert("emacs".to_string());
        blocked.insert("nano".to_string()); // Usually safe, but being conservative

        blocked
    }

    /// Add safe development commands to the allowlist
    fn add_safe_development_commands(&mut self) {
        let safe_commands = [
            "git", "buf", "cargo", "rustc", "rustfmt", "clippy",
            "go", "gofmt", "goimports", "golint",
            "node", "npm", "yarn", "pnpm", "npx",
            "python", "python3", "pip", "pip3",
            "make", "cmake", "ninja",
            "mvn", "gradle", "sbt",
            "dotnet", "nuget",
        ];

        for cmd in &safe_commands {
            self.always_allowed.insert(cmd.to_string());
        }
    }

    /// Add safe file operation commands
    fn add_safe_file_commands(&mut self) {
        let file_commands = [
            "ls", "cat", "head", "tail", "wc", "sort", "uniq",
            "grep", "awk", "sed", "cut", "tr", "tee",
            "find", "locate", "which", "whereis", "type",
            "file", "stat", "du", "df", "pwd", "dirname", "basename",
        ];

        for cmd in &file_commands {
            self.always_allowed.insert(cmd.to_string());
        }
    }

    /// Add commands that are conditionally allowed
    fn add_conditional_commands(&mut self) {
        // Docker with restrictions
        self.conditionally_allowed.insert("docker".to_string(), CommandRestrictions {
            max_args: Some(20),
            required_args: vec![],
            forbidden_args: vec!["--privileged".to_string()],
            allowed_patterns: vec![],
            forbidden_patterns: vec![
                r"--user.*root".to_string(),
                r"--volume.*:/".to_string(),
                r"--mount.*source=/".to_string(),
            ],
            requires_elevation: false,
            custom_validator: Some("validate_docker".to_string()),
        });

        // Python with restrictions (no -c flag)
        self.conditionally_allowed.insert("python".to_string(), CommandRestrictions {
            max_args: Some(10),
            required_args: vec![],
            forbidden_args: vec!["-c".to_string(), "--command".to_string()],
            allowed_patterns: vec![],
            forbidden_patterns: vec![r"-c\s+".to_string()],
            requires_elevation: false,
            custom_validator: Some("validate_python".to_string()),
        });

        // File operations with restrictions
        for cmd in &["cp", "mv", "rm", "mkdir", "rmdir"] {
            self.conditionally_allowed.insert(cmd.to_string(), CommandRestrictions {
                max_args: Some(100),
                required_args: vec![],
                forbidden_args: vec![],
                allowed_patterns: vec![],
                forbidden_patterns: vec![
                    r"^/etc/".to_string(),
                    r"^/bin/".to_string(),
                    r"^/sbin/".to_string(),
                    r"^/usr/bin/".to_string(),
                    r"^/usr/sbin/".to_string(),
                    r"^/boot/".to_string(),
                    r"^/root/".to_string(),
                    r"^/sys/".to_string(),
                    r"^/proc/".to_string(),
                    r"^/dev/".to_string(),
                ],
                requires_elevation: false,
                custom_validator: Some("validate_file_ops".to_string()),
            });
        }
    }

    /// Check if a command is allowed
    pub fn is_command_allowed(&self, command: &str) -> bool {
        // Check if explicitly blocked
        if self.blocked.contains(command) {
            debug!("Command '{}' is explicitly blocked", command);
            return false;
        }

        // Check if always allowed
        if self.always_allowed.contains(command) {
            debug!("Command '{}' is always allowed", command);
            return true;
        }

        // Check if conditionally allowed
        if self.conditionally_allowed.contains_key(command) {
            debug!("Command '{}' is conditionally allowed", command);
            return true;
        }

        // Check permissive mode
        if self.permissive_mode {
            warn!("Command '{}' allowed due to permissive mode", command);
            return true;
        }

        debug!("Command '{}' not found in allowlist", command);
        false
    }

    /// Validate command with arguments against restrictions
    pub fn validate_command(&self, command: &str, args: &[String]) -> Result<()> {
        if !self.is_command_allowed(command) {
            return Err(AgentError::security(format!(
                "Command '{}' is not allowed",
                command
            )));
        }

        // Apply restrictions if they exist
        if let Some(restrictions) = self.conditionally_allowed.get(command) {
            self.apply_restrictions(command, args, restrictions)?;
        }

        Ok(())
    }

    /// Apply restrictions to a command
    fn apply_restrictions(
        &self,
        command: &str,
        args: &[String],
        restrictions: &CommandRestrictions,
    ) -> Result<()> {
        // Check maximum arguments
        if let Some(max_args) = restrictions.max_args {
            if args.len() > max_args {
                return Err(AgentError::validation(format!(
                    "Command '{}' has too many arguments ({} > {})",
                    command,
                    args.len(),
                    max_args
                )));
            }
        }

        // Check required arguments
        for required in &restrictions.required_args {
            if !args.contains(required) {
                return Err(AgentError::validation(format!(
                    "Command '{}' missing required argument: {}",
                    command, required
                )));
            }
        }

        // Check forbidden arguments
        for forbidden in &restrictions.forbidden_args {
            if args.contains(forbidden) {
                return Err(AgentError::security(format!(
                    "Command '{}' contains forbidden argument: {}",
                    command, forbidden
                )));
            }
        }

        // Check forbidden patterns
        for pattern in &restrictions.forbidden_patterns {
            let regex = regex::Regex::new(pattern)
                .map_err(|e| AgentError::validation(format!("Invalid regex pattern: {}", e)))?;

            for arg in args {
                if regex.is_match(arg) {
                    return Err(AgentError::security(format!(
                        "Command '{}' argument '{}' matches forbidden pattern: {}",
                        command, arg, pattern
                    )));
                }
            }
        }

        Ok(())
    }

    /// Add a command to the allowlist
    pub fn add_allowed_command(&mut self, command: String) {
        self.always_allowed.insert(command.clone());
        info!("Added command '{}' to allowlist", command);
    }

    /// Remove a command from the allowlist
    pub fn remove_allowed_command(&mut self, command: &str) {
        self.always_allowed.remove(command);
        self.conditionally_allowed.remove(command);
        info!("Removed command '{}' from allowlist", command);
    }

    /// Block a command explicitly
    pub fn block_command(&mut self, command: String) {
        self.blocked.insert(command.clone());
        self.always_allowed.remove(&command);
        self.conditionally_allowed.remove(&command);
        warn!("Blocked command '{}'", command);
    }

    /// Enable or disable permissive mode
    pub fn set_permissive_mode(&mut self, enabled: bool) {
        self.permissive_mode = enabled;
        if enabled {
            warn!("Permissive mode ENABLED - security reduced!");
        } else {
            info!("Permissive mode disabled - security restored");
        }
    }

    /// Get statistics about the allowlist
    pub fn get_stats(&self) -> AllowlistStats {
        AllowlistStats {
            always_allowed_count: self.always_allowed.len(),
            conditionally_allowed_count: self.conditionally_allowed.len(),
            blocked_count: self.blocked.len(),
            permissive_mode: self.permissive_mode,
        }
    }
}

/// Statistics about the allowlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowlistStats {
    pub always_allowed_count: usize,
    pub conditionally_allowed_count: usize,
    pub blocked_count: usize,
    pub permissive_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_allowlist() {
        let config = AllowlistConfig::secure_default();

        // Should allow safe commands
        assert!(config.is_command_allowed("git"));
        assert!(config.is_command_allowed("cargo"));
        assert!(config.is_command_allowed("ls"));

        // Should block dangerous commands
        assert!(!config.is_command_allowed("bash"));
        assert!(!config.is_command_allowed("curl"));
        assert!(!config.is_command_allowed("sudo"));
    }

    #[test]
    fn test_command_validation() {
        let config = AllowlistConfig::secure_default();

        // Should allow safe git commands
        assert!(config.validate_command("git", &["status".to_string()]).is_ok());

        // Should block dangerous python usage
        assert!(config.validate_command("python", &["-c".to_string(), "print('hi')".to_string()]).is_err());
    }

    #[test]
    fn test_permissive_mode() {
        let mut config = AllowlistConfig::secure_default();

        // Should block unknown command by default
        assert!(!config.is_command_allowed("unknown_command"));

        // Should allow unknown command in permissive mode
        config.set_permissive_mode(true);
        assert!(config.is_command_allowed("unknown_command"));

        // Should still block explicitly blocked commands
        assert!(!config.is_command_allowed("bash"));
    }

    #[test]
    fn test_restrictions() {
        let mut config = AllowlistConfig::secure_default();

        // Add a command with restrictions
        config.conditionally_allowed.insert("test_cmd".to_string(), CommandRestrictions {
            max_args: Some(2),
            required_args: vec!["--required".to_string()],
            forbidden_args: vec!["--forbidden".to_string()],
            allowed_patterns: vec![],
            forbidden_patterns: vec![],
            requires_elevation: false,
            custom_validator: None,
        });

        // Should reject too many args
        assert!(config.validate_command("test_cmd", &[
            "arg1".to_string(),
            "arg2".to_string(),
            "arg3".to_string()
        ]).is_err());

        // Should reject missing required arg
        assert!(config.validate_command("test_cmd", &["arg1".to_string()]).is_err());

        // Should reject forbidden arg
        assert!(config.validate_command("test_cmd", &[
            "--required".to_string(),
            "--forbidden".to_string()
        ]).is_err());

        // Should accept valid args
        assert!(config.validate_command("test_cmd", &[
            "--required".to_string(),
            "valid_arg".to_string()
        ]).is_ok());
    }
}
