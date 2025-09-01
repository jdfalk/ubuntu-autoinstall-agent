// file: src/security/audit.rs
// version: 1.0.0
// guid: d4e5f6a7-b8c9-0123-def4-456789012345

//! Security audit logging module
//!
//! This module provides comprehensive audit logging for all command executions
//! to help detect and investigate potential security incidents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tracing::{error, info, warn};

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    CommandExecution,
    SecurityViolation,
    AccessDenied,
    SuspiciousActivity,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub command: String,
    pub arguments: Vec<String>,
    pub user_context: UserContext,
    pub result: ExecutionResult,
    pub security_notes: Vec<String>,
}

/// User context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub working_directory: String,
    pub environment_summary: EnvironmentSummary,
}

/// Summary of relevant environment variables (sanitized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSummary {
    pub path_entries_count: usize,
    pub has_suspicious_vars: bool,
    pub shell: Option<String>,
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    Allowed,
    Blocked,
    Error(String),
}

/// Log a command execution attempt
pub fn log_command_execution(command: &str, args: &[String]) {
    let entry = AuditEntry {
        timestamp: Utc::now(),
        event_type: AuditEventType::CommandExecution,
        command: command.to_string(),
        arguments: args.to_vec(),
        user_context: capture_user_context(),
        result: ExecutionResult::Allowed,
        security_notes: Vec::new(),
    };

    write_audit_entry(&entry);
    info!("AUDIT: Command execution logged: {} {:?}", command, args);
}

/// Log a security violation
pub fn log_security_violation(command: &str, args: &[String], reason: &str) {
    let entry = AuditEntry {
        timestamp: Utc::now(),
        event_type: AuditEventType::SecurityViolation,
        command: command.to_string(),
        arguments: args.to_vec(),
        user_context: capture_user_context(),
        result: ExecutionResult::Blocked,
        security_notes: vec![reason.to_string()],
    };

    write_audit_entry(&entry);
    warn!("AUDIT: Security violation logged: {} - {}", command, reason);
}

/// Log an access denied event
pub fn log_access_denied(command: &str, reason: &str) {
    let entry = AuditEntry {
        timestamp: Utc::now(),
        event_type: AuditEventType::AccessDenied,
        command: command.to_string(),
        arguments: Vec::new(),
        user_context: capture_user_context(),
        result: ExecutionResult::Blocked,
        security_notes: vec![reason.to_string()],
    };

    write_audit_entry(&entry);
    warn!("AUDIT: Access denied logged: {} - {}", command, reason);
}

/// Log suspicious activity
pub fn log_suspicious_activity(description: &str, context: &[String]) {
    let entry = AuditEntry {
        timestamp: Utc::now(),
        event_type: AuditEventType::SuspiciousActivity,
        command: "SUSPICIOUS".to_string(),
        arguments: context.to_vec(),
        user_context: capture_user_context(),
        result: ExecutionResult::Blocked,
        security_notes: vec![description.to_string()],
    };

    write_audit_entry(&entry);
    warn!("AUDIT: Suspicious activity logged: {}", description);
}

/// Capture current user context for audit logging
fn capture_user_context() -> UserContext {
    let working_directory = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "UNKNOWN".to_string());

    let environment_summary = capture_environment_summary();

    UserContext {
        working_directory,
        environment_summary,
    }
}

/// Capture a summary of the environment (without sensitive data)
fn capture_environment_summary() -> EnvironmentSummary {
    let path_entries_count = std::env::var("PATH")
        .map(|path| path.split(':').count())
        .unwrap_or(0);

    // Check for suspicious environment variables
    let suspicious_vars = [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "PYTHONPATH",
    ];

    let has_suspicious_vars = suspicious_vars
        .iter()
        .any(|var| std::env::var(var).is_ok());

    let shell = std::env::var("SHELL").ok();

    EnvironmentSummary {
        path_entries_count,
        has_suspicious_vars,
        shell,
    }
}

/// Write an audit entry to the audit log file
fn write_audit_entry(entry: &AuditEntry) {
    if let Err(e) = write_audit_entry_impl(entry) {
        error!("Failed to write audit entry: {}", e);
        // Fallback to stderr if file writing fails
        eprintln!("AUDIT FALLBACK: {:?}", entry);
    }
}

/// Implementation of audit entry writing
fn write_audit_entry_impl(entry: &AuditEntry) -> std::io::Result<()> {
    let log_dir = get_audit_log_directory();
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("security_audit.jsonl");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    let json_line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    writeln!(file, "{}", json_line)?;
    file.flush()?;

    Ok(())
}

/// Get the audit log directory
fn get_audit_log_directory() -> PathBuf {
    // Try to use a dedicated audit log directory
    if let Ok(audit_dir) = std::env::var("COPILOT_AUDIT_DIR") {
        return PathBuf::from(audit_dir);
    }

    // Fall back to logs directory in current working directory
    let mut log_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    log_dir.push("logs");
    log_dir.push("security");
    log_dir
}

/// Rotate audit logs if they get too large
pub fn rotate_audit_logs() -> std::io::Result<()> {
    let log_dir = get_audit_log_directory();
    let log_file = log_dir.join("security_audit.jsonl");

    if !log_file.exists() {
        return Ok(());
    }

    let metadata = std::fs::metadata(&log_file)?;
    let file_size = metadata.len();

    // Rotate if file is larger than 10MB
    if file_size > 10 * 1024 * 1024 {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let rotated_name = format!("security_audit_{}.jsonl", timestamp);
        let rotated_path = log_dir.join(rotated_name);

        std::fs::rename(&log_file, rotated_path)?;
        info!("Rotated audit log due to size: {} bytes", file_size);
    }

    Ok(())
}

/// Clean up old audit logs (keep last 30 days)
pub fn cleanup_old_audit_logs() -> std::io::Result<()> {
    let log_dir = get_audit_log_directory();

    if !log_dir.exists() {
        return Ok(());
    }

    let cutoff = Utc::now() - chrono::Duration::days(30);
    let mut removed_count = 0;

    for entry in std::fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name() {
            if let Some(name_str) = file_name.to_str() {
                if name_str.starts_with("security_audit_") && name_str.ends_with(".jsonl") {
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            let modified_datetime: DateTime<Utc> = modified.into();
                            if modified_datetime < cutoff {
                                if std::fs::remove_file(&path).is_ok() {
                                    removed_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if removed_count > 0 {
        info!("Cleaned up {} old audit log files", removed_count);
    }

    Ok(())
}

/// Initialize audit logging system
pub fn initialize_audit_system() -> std::io::Result<()> {
    // Create audit log directory
    let log_dir = get_audit_log_directory();
    std::fs::create_dir_all(&log_dir)?;

    // Rotate logs if needed
    rotate_audit_logs()?;

    // Clean up old logs
    cleanup_old_audit_logs()?;

    info!("Security audit system initialized. Logs: {}", log_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            event_type: AuditEventType::CommandExecution,
            command: "git".to_string(),
            arguments: vec!["status".to_string()],
            user_context: UserContext {
                working_directory: "/tmp".to_string(),
                environment_summary: EnvironmentSummary {
                    path_entries_count: 5,
                    has_suspicious_vars: false,
                    shell: Some("bash".to_string()),
                },
            },
            result: ExecutionResult::Allowed,
            security_notes: vec!["test".to_string()],
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.command, deserialized.command);
        assert_eq!(entry.arguments, deserialized.arguments);
    }

    #[test]
    fn test_environment_capture() {
        let env_summary = capture_environment_summary();
        assert!(env_summary.path_entries_count > 0);
    }

    #[test]
    fn test_user_context_capture() {
        let context = capture_user_context();
        assert!(!context.working_directory.is_empty());
    }
}
