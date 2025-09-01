// file: src/error.rs
// version: 2.0.0
// guid: 57b83a63-07b6-4534-aa6c-51e8797254e0

use thiserror::Error;

/// Result type alias for the application
pub type Result<T> = std::result::Result<T, AgentError>;

/// Error types for the Copilot Agent Utility
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Command execution error: {0}")]
    Execution(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("System error: {0}")]
    System(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl AgentError {
    /// Create a new configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new execution error
    pub fn execution(msg: impl Into<String>) -> Self {
        Self::Execution(msg.into())
    }

    /// Create a new validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create a new permission error
    pub fn permission(msg: impl Into<String>) -> Self {
        Self::Permission(msg.into())
    }

    /// Create a new file not found error
    pub fn file_not_found(msg: impl Into<String>) -> Self {
        Self::FileNotFound(msg.into())
    }

    /// Create a new invalid argument error
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }

    /// Create a new timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create a new network error
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    /// Create a new security error
    pub fn security(msg: impl Into<String>) -> Self {
        Self::Security(msg.into())
    }

    /// Create a new system error
    pub fn system(msg: impl Into<String>) -> Self {
        Self::System(msg.into())
    }

    /// Create a new other error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
