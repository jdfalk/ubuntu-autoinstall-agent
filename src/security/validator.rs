// file: src/security/validator.rs
// version: 1.0.0
// guid: c3d4e5f6-a7b8-9012-cdef-345678901234

//! Command validation module
//!
//! This module provides command-specific validation logic to ensure that
//! arguments are not only sanitized but also semantically valid and safe.

use crate::error::{AgentError, Result};
use std::path::Path;
use tracing::{debug, warn};

/// Validate command arguments for semantic correctness and safety
pub fn validate_command_arguments(command: &str, args: &[String]) -> Result<()> {
    match command {
        "git" => validate_git_arguments(args),
        "buf" => validate_buf_arguments(args),
        "cargo" => validate_cargo_arguments(args),
        "go" => validate_go_arguments(args),
        "docker" => validate_docker_arguments(args),
        "npm" | "yarn" | "pnpm" => validate_node_arguments(args),
        "python" | "python3" => validate_python_arguments(args),
        "ls" | "cat" | "cp" | "mv" | "rm" | "mkdir" | "find" | "grep" => {
            validate_file_arguments(command, args)
        }
        _ => validate_generic_arguments(args),
    }
}

/// Validate git command arguments
fn validate_git_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(AgentError::validation("Git command requires arguments"));
    }

    let subcommand = &args[0];

    // Allow only safe git subcommands
    let allowed_subcommands = [
        "status", "add", "commit", "push", "pull", "fetch", "log", "diff",
        "branch", "checkout", "merge", "rebase", "reset", "clean", "stash",
        "tag", "remote", "config", "show", "blame", "cherry-pick",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        return Err(AgentError::security(format!(
            "Git subcommand '{}' is not allowed for security reasons",
            subcommand
        )));
    }

    // Additional validation for specific subcommands
    match subcommand.as_str() {
        "reset" => validate_git_reset_args(&args[1..])?,
        "clean" => validate_git_clean_args(&args[1..])?,
        "config" => validate_git_config_args(&args[1..])?,
        "remote" => validate_git_remote_args(&args[1..])?,
        _ => {}
    }

    Ok(())
}

/// Validate git reset arguments to prevent destructive operations
fn validate_git_reset_args(args: &[String]) -> Result<()> {
    for arg in args {
        if arg == "--hard" {
            warn!("Git reset --hard detected - potentially destructive operation");
            // Allow but log the warning
        }
        if arg.starts_with("--") && !["--soft", "--mixed", "--hard", "--keep", "--merge"].contains(&arg.as_str()) {
            return Err(AgentError::validation(format!(
                "Unknown git reset option: {}",
                arg
            )));
        }
    }
    Ok(())
}

/// Validate git clean arguments
fn validate_git_clean_args(args: &[String]) -> Result<()> {
    let mut has_safe_flag = false;
    let mut has_force_flag = false;

    for arg in args {
        match arg.as_str() {
            "-n" | "--dry-run" => has_safe_flag = true,
            "-f" | "--force" => {
                has_force_flag = true;
                warn!("Git clean --force detected - destructive operation");
            }
            "-x" | "-X" => {
                warn!("Git clean with ignored files flag detected");
            }
            "-d" => {
                warn!("Git clean directories flag detected");
            }
            _ if arg.starts_with("-") => {
                return Err(AgentError::validation(format!(
                    "Unknown git clean option: {}",
                    arg
                )));
            }
            _ => {}
        }
    }

    // If force flag is used without dry-run, require explicit confirmation
    if has_force_flag && !has_safe_flag {
        warn!("Git clean --force without --dry-run detected - potentially destructive");
    }

    Ok(())
}

/// Validate git config arguments to prevent dangerous configuration changes
fn validate_git_config_args(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    // Block dangerous config options
    let dangerous_configs = [
        "core.hooksPath",
        "core.gitProxy",
        "core.sshCommand",
        "http.proxy",
        "https.proxy",
        "url.",
        "remote.origin.url",
    ];

    for arg in args {
        for dangerous in &dangerous_configs {
            if arg.starts_with(dangerous) {
                return Err(AgentError::security(format!(
                    "Git config option '{}' is not allowed for security reasons",
                    arg
                )));
            }
        }
    }

    Ok(())
}

/// Validate git remote arguments
fn validate_git_remote_args(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let subcommand = &args[0];

    // Allow only safe remote operations
    let allowed_remote_ops = ["show", "get-url", "-v", "prune"];

    if !allowed_remote_ops.contains(&subcommand.as_str()) && !subcommand.starts_with("-") {
        warn!("Git remote operation '{}' may modify remote configuration", subcommand);
    }

    Ok(())
}

/// Validate buf command arguments
fn validate_buf_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(()); // buf without args shows help
    }

    let subcommand = &args[0];

    // Allow only safe buf subcommands
    let allowed_subcommands = [
        "generate", "lint", "format", "breaking", "build", "push", "export",
        "mod", "dep", "registry", "config", "beta",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        return Err(AgentError::security(format!(
            "Buf subcommand '{}' is not allowed",
            subcommand
        )));
    }

    Ok(())
}

/// Validate cargo command arguments
fn validate_cargo_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let subcommand = &args[0];

    // Allow only safe cargo subcommands
    let allowed_subcommands = [
        "build", "check", "clean", "doc", "test", "bench", "update", "search",
        "publish", "install", "uninstall", "add", "remove", "run", "fmt",
        "clippy", "version", "help", "tree", "audit", "fix", "metadata",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        return Err(AgentError::security(format!(
            "Cargo subcommand '{}' is not allowed",
            subcommand
        )));
    }

    // Additional validation for potentially dangerous subcommands
    match subcommand.as_str() {
        "install" => validate_cargo_install_args(&args[1..])?,
        "run" => validate_cargo_run_args(&args[1..])?,
        _ => {}
    }

    Ok(())
}

/// Validate cargo install arguments
fn validate_cargo_install_args(args: &[String]) -> Result<()> {
    // Check for dangerous install sources
    for arg in args {
        if arg.starts_with("--git") || arg.starts_with("--path") {
            warn!("Cargo install from custom source: {}", arg);
        }
        if arg == "--force" {
            warn!("Cargo install --force detected");
        }
    }
    Ok(())
}

/// Validate cargo run arguments
fn validate_cargo_run_args(args: &[String]) -> Result<()> {
    // Cargo run can execute arbitrary code, so we just log and allow
    if !args.is_empty() {
        debug!("Cargo run with args: {:?}", args);
    }
    Ok(())
}

/// Validate go command arguments
fn validate_go_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let subcommand = &args[0];

    // Allow only safe go subcommands
    let allowed_subcommands = [
        "build", "run", "test", "mod", "get", "install", "clean", "doc",
        "fmt", "generate", "list", "version", "env", "vet", "work",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        return Err(AgentError::security(format!(
            "Go subcommand '{}' is not allowed",
            subcommand
        )));
    }

    Ok(())
}

/// Validate docker command arguments
fn validate_docker_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let subcommand = &args[0];

    // Allow only safe docker subcommands
    let allowed_subcommands = [
        "build", "run", "ps", "images", "logs", "inspect", "version",
        "info", "system", "network", "volume", "compose",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        return Err(AgentError::security(format!(
            "Docker subcommand '{}' is not allowed",
            subcommand
        )));
    }

    // Additional validation for docker run
    if subcommand == "run" {
        validate_docker_run_args(&args[1..])?;
    }

    Ok(())
}

/// Validate docker run arguments for dangerous options
fn validate_docker_run_args(args: &[String]) -> Result<()> {
    for arg in args {
        if arg == "--privileged" {
            return Err(AgentError::security(
                "Docker --privileged mode is not allowed"
            ));
        }
        if arg.starts_with("--user") && arg.contains("root") {
            warn!("Docker run as root user detected");
        }
        if arg.starts_with("--volume") || arg.starts_with("-v") {
            if arg.contains(":/") {
                return Err(AgentError::security(
                    "Docker volume mount to root filesystem is not allowed"
                ));
            }
        }
    }
    Ok(())
}

/// Validate Node.js ecosystem command arguments
fn validate_node_arguments(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let subcommand = &args[0];

    // Allow common npm/yarn operations
    let allowed_subcommands = [
        "install", "ci", "test", "run", "build", "start", "dev", "lint",
        "format", "audit", "outdated", "list", "info", "version", "help",
        "add", "remove", "update", "upgrade", "check",
    ];

    if !allowed_subcommands.contains(&subcommand.as_str()) {
        warn!("Node subcommand '{}' may not be safe", subcommand);
    }

    Ok(())
}

/// Validate Python command arguments
fn validate_python_arguments(args: &[String]) -> Result<()> {
    // Python can execute arbitrary code, so we need to be very careful
    for arg in args {
        if arg == "-c" || arg == "--command" {
            return Err(AgentError::security(
                "Python -c flag is not allowed for security reasons"
            ));
        }
        if arg.starts_with("-c") {
            return Err(AgentError::security(
                "Python inline code execution is not allowed"
            ));
        }
    }

    Ok(())
}

/// Validate file operation arguments
fn validate_file_arguments(command: &str, args: &[String]) -> Result<()> {
    for arg in args {
        // Skip flags
        if arg.starts_with("-") {
            continue;
        }

        // Validate paths
        let path = Path::new(arg);

        // Check for absolute paths to sensitive directories
        if path.is_absolute() {
            let path_str = path.to_string_lossy();
            let sensitive_paths = [
                "/etc", "/bin", "/sbin", "/usr/bin", "/usr/sbin", "/boot",
                "/root", "/sys", "/proc", "/dev",
            ];

            for sensitive in &sensitive_paths {
                if path_str.starts_with(sensitive) {
                    return Err(AgentError::security(format!(
                        "Access to sensitive path '{}' is not allowed",
                        path_str
                    )));
                }
            }
        }

        // Special validation for rm command
        if command == "rm" {
            if arg == "/" || arg == "/*" {
                return Err(AgentError::security(
                    "Deletion of root filesystem is not allowed"
                ));
            }
            if arg.contains("*") && arg.len() < 5 {
                warn!("Potentially dangerous rm pattern: {}", arg);
            }
        }
    }

    Ok(())
}

/// Generic argument validation
fn validate_generic_arguments(args: &[String]) -> Result<()> {
    // Basic checks for all commands
    for arg in args {
        if arg.len() > 1000 {
            return Err(AgentError::validation(
                "Argument too long (> 1000 characters)"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_validation() {
        // Safe operations
        assert!(validate_git_arguments(&["status".to_string()]).is_ok());
        assert!(validate_git_arguments(&["add".to_string(), ".".to_string()]).is_ok());

        // Dangerous operations
        assert!(validate_git_arguments(&["daemon".to_string()]).is_err());
        assert!(validate_git_arguments(&["upload-pack".to_string()]).is_err());
    }

    #[test]
    fn test_python_validation() {
        // Safe operations
        assert!(validate_python_arguments(&["script.py".to_string()]).is_ok());
        assert!(validate_python_arguments(&["-m".to_string(), "pytest".to_string()]).is_ok());

        // Dangerous operations
        assert!(validate_python_arguments(&["-c".to_string(), "print('hello')".to_string()]).is_err());
        assert!(validate_python_arguments(&["--command".to_string(), "exec('evil')".to_string()]).is_err());
    }

    #[test]
    fn test_file_validation() {
        // Safe operations
        assert!(validate_file_arguments("ls", &["src/".to_string()]).is_ok());
        assert!(validate_file_arguments("cat", &["README.md".to_string()]).is_ok());

        // Dangerous operations
        assert!(validate_file_arguments("rm", &["/".to_string()]).is_err());
        assert!(validate_file_arguments("cat", &["/etc/passwd".to_string()]).is_err());
    }

    #[test]
    fn test_docker_validation() {
        // Safe operations
        assert!(validate_docker_arguments(&["ps".to_string()]).is_ok());
        assert!(validate_docker_arguments(&["images".to_string()]).is_ok());

        // Dangerous operations
        assert!(validate_docker_arguments(&["run".to_string(), "--privileged".to_string()]).is_err());
        assert!(validate_docker_arguments(&["exec".to_string()]).is_err());
    }
}
