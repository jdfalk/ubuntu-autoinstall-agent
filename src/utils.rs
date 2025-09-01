// file: src/utils.rs
// version: 1.0.0
// guid: 0784e6f5-a659-4507-bd5d-dd33b38f6974

//! Utility functions for the Copilot Agent Utility

use crate::error::{AgentError, Result};
use std::path::Path;

/// Validate that a path is safe to operate on
pub fn validate_path(path: &Path) -> Result<()> {
    // Check for path traversal attempts
    if path.to_string_lossy().contains("..") {
        return Err(AgentError::validation("Path contains traversal sequences"));
    }

    // Check for absolute paths in unsafe contexts
    if path.is_absolute() {
        // This might be okay in some contexts, but we should be careful
        tracing::warn!("Operating on absolute path: {}", path.display());
    }

    Ok(())
}

/// Sanitize a string for safe use in commands
pub fn sanitize_string(input: &str) -> String {
    // Remove or escape potentially dangerous characters
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_.:/".contains(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_validate_path_safe() {
        let path = PathBuf::from("src/main.rs");
        assert!(validate_path(&path).is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        let path = PathBuf::from("../../../etc/passwd");
        assert!(validate_path(&path).is_err());
    }

    #[test]
    fn test_sanitize_string() {
        let input = "test-file.txt; rm -rf /";
        let sanitized = sanitize_string(input);
        assert!(!sanitized.contains(';'));
    }
}
