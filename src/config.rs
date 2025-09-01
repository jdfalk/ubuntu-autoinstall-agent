// file: src/config.rs
// version: 1.0.0
// guid: 6ea31d79-e2bf-4304-a841-22bf1e595512

use crate::error::{AgentError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub logging: LoggingConfig,
    pub safety: SafetyConfig,
    pub git: GitConfig,
    pub execution: ExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub working_directory: Option<PathBuf>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub file_rotation: bool,
    pub max_log_size: String,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub dry_run: bool,
    pub confirm_destructive: bool,
    pub backup_before_delete: bool,
    pub validate_paths: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub auto_stage: bool,
    pub require_message: bool,
    pub push_hooks: bool,
    pub safe_force_push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub shell: Option<String>,
    pub environment_isolation: bool,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<u8>,
    pub max_execution_time: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                working_directory: None,
                timeout_seconds: 300,
                max_retries: 3,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: LogFormat::Pretty,
                file_rotation: true,
                max_log_size: "10MB".to_string(),
                retention_days: 30,
            },
            safety: SafetyConfig {
                dry_run: false,
                confirm_destructive: true,
                backup_before_delete: true,
                validate_paths: true,
            },
            git: GitConfig {
                auto_stage: false,
                require_message: true,
                push_hooks: true,
                safe_force_push: true,
            },
            execution: ExecutionConfig {
                shell: None,
                environment_isolation: false,
                resource_limits: ResourceLimits {
                    max_memory_mb: Some(1024),
                    max_cpu_percent: Some(80),
                    max_execution_time: Some(600),
                },
            },
        }
    }
}

impl Config {
    /// Load configuration from multiple sources
    pub async fn load() -> Result<Self> {
        let mut config = Self::default();

        // Try to load from user config directory
        if let Some(user_config) = Self::user_config_path() {
            if user_config.exists() {
                info!("Loading user configuration from: {}", user_config.display());
                config = Self::load_from_file(&user_config).await?;
            }
        }

        // Try to load from project config
        let project_config = Path::new(".copilot-agent-util.toml");
        if project_config.exists() {
            info!(
                "Loading project configuration from: {}",
                project_config.display()
            );
            let project_config = Self::load_from_file(project_config).await?;
            config = Self::merge_configs(config, project_config);
        }

        // Override with environment variables
        config = Self::apply_env_overrides(config)?;

        debug!("Final configuration: {:#?}", config);
        Ok(config)
    }

    /// Get the user configuration file path
    fn user_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("copilot-agent-util").join("config.toml"))
    }

    /// Load configuration from a TOML file
    async fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            AgentError::config(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        toml::from_str(&content).map_err(|e| {
            AgentError::config(format!(
                "Failed to parse config file {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Merge two configurations, with the second taking precedence
    fn merge_configs(_base: Self, override_config: Self) -> Self {
        // For now, just return the override config
        // In a full implementation, we'd merge each field intelligently
        override_config
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(mut config: Self) -> Result<Self> {
        if let Ok(level) = std::env::var("COPILOT_AGENT_LOG_LEVEL") {
            config.logging.level = level;
        }

        if let Ok(dry_run) = std::env::var("COPILOT_AGENT_DRY_RUN") {
            config.safety.dry_run = dry_run.parse().unwrap_or(false);
        }

        if let Ok(timeout) = std::env::var("COPILOT_AGENT_TIMEOUT") {
            if let Ok(timeout_secs) = timeout.parse::<u64>() {
                config.general.timeout_seconds = timeout_secs;
            }
        }

        Ok(config)
    }
}
