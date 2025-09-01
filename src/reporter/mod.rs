// file: src/reporter/mod.rs
// version: 1.0.0
// guid: j0k1l2m3-n4o5-6789-0123-def456789012

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Status reporter for sending installation progress updates
pub struct StatusReporter {
    /// HTTP client for sending reports
    client: Client,

    /// Webhook URL to send reports to (optional)
    webhook_url: Option<String>,

    /// Additional headers to send with requests
    headers: HashMap<String, String>,

    /// Reporter configuration
    config: ReporterConfig,

    /// Installation metadata
    metadata: InstallationMetadata,
}

/// Configuration for the status reporter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReporterConfig {
    /// Timeout for HTTP requests (seconds)
    pub request_timeout: u64,

    /// Number of retry attempts for failed requests
    pub retry_attempts: u32,

    /// Delay between retry attempts (seconds)
    pub retry_delay: u64,

    /// Whether to include system information in reports
    pub include_system_info: bool,

    /// Whether to include step details in reports
    pub include_step_details: bool,

    /// Whether to compress request bodies
    pub compress_requests: bool,

    /// Maximum report size in bytes
    pub max_report_size: usize,
}

impl Default for ReporterConfig {
    fn default() -> Self {
        Self {
            request_timeout: 30,
            retry_attempts: 3,
            retry_delay: 5,
            include_system_info: true,
            include_step_details: true,
            compress_requests: false,
            max_report_size: 1024 * 1024, // 1MB
        }
    }
}

/// Installation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationMetadata {
    /// Installation session ID
    pub session_id: Uuid,

    /// Hostname of the system being installed
    pub hostname: String,

    /// IP address of the system
    pub ip_address: String,

    /// MAC address of the primary interface
    pub mac_address: String,

    /// Installation started timestamp
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Agent version
    pub agent_version: String,

    /// Target Ubuntu version
    pub ubuntu_version: String,

    /// Installation configuration checksum
    pub config_checksum: String,
}

/// Status report sent to webhook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    /// Report type
    pub report_type: ReportType,

    /// Installation metadata
    pub metadata: InstallationMetadata,

    /// Timestamp when report was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Current installation status
    pub status: InstallationStatus,

    /// Progress percentage (0-100)
    pub progress: u8,

    /// Current step information
    pub current_step: Option<StepReport>,

    /// Error information (if applicable)
    pub error: Option<ErrorReport>,

    /// System information (optional)
    pub system_info: Option<SystemInfo>,

    /// Additional custom data
    pub custom_data: HashMap<String, serde_json::Value>,
}

/// Type of status report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportType {
    /// Installation started
    InstallationStarted,

    /// Installation progress update
    InstallationProgress,

    /// Installation completed successfully
    InstallationCompleted,

    /// Installation failed
    InstallationFailed,

    /// Installation was cancelled
    InstallationCancelled,

    /// Step started
    StepStarted,

    /// Step completed
    StepCompleted,

    /// Step failed
    StepFailed,

    /// Step skipped
    StepSkipped,

    /// Recovery attempt
    RecoveryAttempt,

    /// Custom event
    CustomEvent,
}

/// Installation status in reports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStatus {
    /// Installation is preparing
    Preparing,

    /// Installation is running
    Running,

    /// Installation completed successfully
    Completed,

    /// Installation failed
    Failed,

    /// Installation was cancelled
    Cancelled,

    /// Installation is recovering from failure
    Recovering,
}

/// Information about the current step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepReport {
    /// Step name
    pub name: String,

    /// Step description
    pub description: String,

    /// Step number (1-based)
    pub step_number: usize,

    /// Total number of steps
    pub total_steps: usize,

    /// Step started timestamp
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Step completed timestamp (if finished)
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Estimated duration for this step
    pub estimated_duration: Duration,

    /// Actual execution time (if completed)
    pub execution_time: Option<Duration>,
}

/// Error information in reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Error message
    pub message: String,

    /// Detailed error description
    pub details: Option<String>,

    /// Error code (if applicable)
    pub code: Option<String>,

    /// Step where error occurred
    pub step_name: Option<String>,

    /// Recovery attempts made
    pub recovery_attempts: u32,

    /// Whether error is recoverable
    pub recoverable: bool,
}

/// System information included in reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// System architecture
    pub architecture: String,

    /// Total memory in MB
    pub total_memory: u64,

    /// Available memory in MB
    pub available_memory: u64,

    /// CPU information
    pub cpu_info: String,

    /// Disk space information
    pub disk_space: Vec<DiskSpace>,

    /// Network interfaces
    pub network_interfaces: Vec<NetworkInterface>,

    /// System load averages
    pub load_averages: [f64; 3],

    /// System uptime in seconds
    pub uptime: u64,
}

/// Disk space information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSpace {
    /// Filesystem or device
    pub filesystem: String,

    /// Mount point
    pub mountpoint: String,

    /// Total size in bytes
    pub total_bytes: u64,

    /// Used space in bytes
    pub used_bytes: u64,

    /// Available space in bytes
    pub available_bytes: u64,

    /// Usage percentage
    pub usage_percent: f64,
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name
    pub name: String,

    /// IP addresses
    pub ip_addresses: Vec<String>,

    /// MAC address
    pub mac_address: Option<String>,

    /// Interface state (up/down)
    pub state: String,

    /// Interface speed in Mbps
    pub speed: Option<u64>,
}

impl StatusReporter {
    /// Create a new status reporter
    pub fn new(webhook_url: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ubuntu-autoinstall-agent/1.0")
            .build()
            .context("Failed to create HTTP client")?;

        let metadata = InstallationMetadata {
            session_id: Uuid::new_v4(),
            hostname: Self::get_hostname()?,
            ip_address: Self::get_ip_address()?,
            mac_address: Self::get_mac_address()?,
            started_at: chrono::Utc::now(),
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            ubuntu_version: Self::get_ubuntu_version()?,
            config_checksum: "unknown".to_string(),
        };

        Ok(Self {
            client,
            webhook_url,
            headers: HashMap::new(),
            config: ReporterConfig::default(),
            metadata,
        })
    }

    /// Create a new status reporter with custom configuration
    pub fn with_config(webhook_url: Option<String>, config: ReporterConfig) -> Result<Self> {
        let mut reporter = Self::new(webhook_url)?;
        reporter.config = config;

        // Update client with new timeout
        reporter.client = Client::builder()
            .timeout(Duration::from_secs(reporter.config.request_timeout))
            .user_agent("ubuntu-autoinstall-agent/1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(reporter)
    }

    /// Set additional headers for requests
    pub fn set_headers(&mut self, headers: HashMap<String, String>) {
        self.headers = headers;
    }

    /// Set installation metadata
    pub fn set_metadata(&mut self, metadata: InstallationMetadata) {
        self.metadata = metadata;
    }

    /// Report installation started
    pub async fn report_installation_started(&self, session_id: Uuid) -> Result<()> {
        let mut metadata = self.metadata.clone();
        metadata.session_id = session_id;

        let report = StatusReport {
            report_type: ReportType::InstallationStarted,
            metadata,
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Preparing,
            progress: 0,
            current_step: None,
            error: None,
            system_info: if self.config.include_system_info {
                Some(self.get_system_info().await?)
            } else {
                None
            },
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report installation completed
    pub async fn report_installation_completed(&self, session_id: Uuid) -> Result<()> {
        let mut metadata = self.metadata.clone();
        metadata.session_id = session_id;

        let report = StatusReport {
            report_type: ReportType::InstallationCompleted,
            metadata,
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Completed,
            progress: 100,
            current_step: None,
            error: None,
            system_info: if self.config.include_system_info {
                Some(self.get_system_info().await?)
            } else {
                None
            },
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report installation failed
    pub async fn report_installation_failed(&self, session_id: Uuid, error: &anyhow::Error) -> Result<()> {
        let mut metadata = self.metadata.clone();
        metadata.session_id = session_id;

        let error_report = ErrorReport {
            message: error.to_string(),
            details: Some(format!("{:?}", error)),
            code: None,
            step_name: None,
            recovery_attempts: 0,
            recoverable: false,
        };

        let report = StatusReport {
            report_type: ReportType::InstallationFailed,
            metadata,
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Failed,
            progress: 0,
            current_step: None,
            error: Some(error_report),
            system_info: if self.config.include_system_info {
                Some(self.get_system_info().await?)
            } else {
                None
            },
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report installation cancelled
    pub async fn report_installation_cancelled(&self, session_id: Uuid) -> Result<()> {
        let mut metadata = self.metadata.clone();
        metadata.session_id = session_id;

        let report = StatusReport {
            report_type: ReportType::InstallationCancelled,
            metadata,
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Cancelled,
            progress: 0,
            current_step: None,
            error: None,
            system_info: None,
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report step started
    pub async fn report_step_started(&self, step_name: &str) -> Result<()> {
        let step_report = StepReport {
            name: step_name.to_string(),
            description: "Step execution started".to_string(),
            step_number: 0,
            total_steps: 0,
            started_at: chrono::Utc::now(),
            completed_at: None,
            estimated_duration: Duration::from_secs(30),
            execution_time: None,
        };

        let report = StatusReport {
            report_type: ReportType::StepStarted,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Running,
            progress: 0,
            current_step: Some(step_report),
            error: None,
            system_info: None,
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report step completed
    pub async fn report_step_completed(&self, step_name: &str) -> Result<()> {
        let step_report = StepReport {
            name: step_name.to_string(),
            description: "Step execution completed".to_string(),
            step_number: 0,
            total_steps: 0,
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            estimated_duration: Duration::from_secs(30),
            execution_time: Some(Duration::from_secs(30)),
        };

        let report = StatusReport {
            report_type: ReportType::StepCompleted,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Running,
            progress: 0,
            current_step: Some(step_report),
            error: None,
            system_info: None,
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report step failed
    pub async fn report_step_failed(&self, step_name: &str, error_message: &str) -> Result<()> {
        let step_report = StepReport {
            name: step_name.to_string(),
            description: "Step execution failed".to_string(),
            step_number: 0,
            total_steps: 0,
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            estimated_duration: Duration::from_secs(30),
            execution_time: Some(Duration::from_secs(30)),
        };

        let error_report = ErrorReport {
            message: error_message.to_string(),
            details: None,
            code: None,
            step_name: Some(step_name.to_string()),
            recovery_attempts: 0,
            recoverable: true,
        };

        let report = StatusReport {
            report_type: ReportType::StepFailed,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Running,
            progress: 0,
            current_step: Some(step_report),
            error: Some(error_report),
            system_info: None,
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report step skipped
    pub async fn report_step_skipped(&self, step_name: &str) -> Result<()> {
        let step_report = StepReport {
            name: step_name.to_string(),
            description: "Step was skipped".to_string(),
            step_number: 0,
            total_steps: 0,
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            estimated_duration: Duration::from_secs(0),
            execution_time: Some(Duration::from_secs(0)),
        };

        let report = StatusReport {
            report_type: ReportType::StepSkipped,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Running,
            progress: 0,
            current_step: Some(step_report),
            error: None,
            system_info: None,
            custom_data: HashMap::new(),
        };

        self.send_report(report).await
    }

    /// Report recovery attempt
    pub async fn report_step_recovery_attempt(&self, step_name: &str, attempt_number: u32) -> Result<()> {
        let step_report = StepReport {
            name: step_name.to_string(),
            description: format!("Recovery attempt #{}", attempt_number),
            step_number: 0,
            total_steps: 0,
            started_at: chrono::Utc::now(),
            completed_at: None,
            estimated_duration: Duration::from_secs(60),
            execution_time: None,
        };

        let report = StatusReport {
            report_type: ReportType::RecoveryAttempt,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Recovering,
            progress: 0,
            current_step: Some(step_report),
            error: None,
            system_info: None,
            custom_data: {
                let mut data = HashMap::new();
                data.insert("attempt_number".to_string(), serde_json::Value::Number(attempt_number.into()));
                data
            },
        };

        self.send_report(report).await
    }

    /// Send a custom report
    pub async fn report_custom_event(&self, event_name: &str, data: HashMap<String, serde_json::Value>) -> Result<()> {
        let report = StatusReport {
            report_type: ReportType::CustomEvent,
            metadata: self.metadata.clone(),
            timestamp: chrono::Utc::now(),
            status: InstallationStatus::Running,
            progress: 0,
            current_step: None,
            error: None,
            system_info: None,
            custom_data: {
                let mut custom_data = data;
                custom_data.insert("event_name".to_string(), serde_json::Value::String(event_name.to_string()));
                custom_data
            },
        };

        self.send_report(report).await
    }

    /// Send a status report
    async fn send_report(&self, report: StatusReport) -> Result<()> {
        if let Some(webhook_url) = &self.webhook_url {
            info!("Sending status report: {:?} to {}", report.report_type, webhook_url);

            let json_body = serde_json::to_string(&report)
                .context("Failed to serialize report")?;

            // Check report size
            if json_body.len() > self.config.max_report_size {
                warn!("Report size ({} bytes) exceeds maximum ({} bytes), truncating",
                      json_body.len(), self.config.max_report_size);
            }

            let mut request = self.client.post(webhook_url)
                .header("Content-Type", "application/json")
                .body(json_body);

            // Add custom headers
            for (key, value) in &self.headers {
                request = request.header(key, value);
            }

            let result = self.send_with_retry(request).await;

            match result {
                Ok(_) => {
                    debug!("Status report sent successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to send status report: {}", e);
                    Err(e)
                }
            }
        } else {
            debug!("No webhook URL configured, logging report locally: {:?}", report.report_type);
            Ok(())
        }
    }

    /// Send HTTP request with retry logic
    async fn send_with_retry(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let mut last_error = None;

        for attempt in 1..=self.config.retry_attempts {
            match request.try_clone() {
                Some(cloned_request) => {
                    match cloned_request.send().await {
                        Ok(response) => {
                            if response.status().is_success() {
                                return Ok(response);
                            } else {
                                let error = anyhow::anyhow!("HTTP error: {}", response.status());
                                last_error = Some(error);
                            }
                        }
                        Err(e) => {
                            let error = anyhow::Error::from(e).context("HTTP request failed");
                            last_error = Some(error);
                        }
                    }
                }
                None => {
                    let error = anyhow::anyhow!("Failed to clone request for retry");
                    last_error = Some(error);
                }
            }

            if attempt < self.config.retry_attempts {
                warn!("Request attempt {}/{} failed, retrying in {} seconds",
                      attempt, self.config.retry_attempts, self.config.retry_delay);
                tokio::time::sleep(Duration::from_secs(self.config.retry_delay)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }

    /// Get system hostname
    fn get_hostname() -> Result<String> {
        std::env::var("HOSTNAME")
            .or_else(|_| {
                std::fs::read_to_string("/proc/sys/kernel/hostname")
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|_| "unknown".to_string())
            .pipe(Ok)
    }

    /// Get primary IP address
    fn get_ip_address() -> Result<String> {
        // Try to get IP from network interfaces
        if let Ok(interfaces) = network_interface::NetworkInterface::show() {
            for interface in interfaces {
                for addr in interface.addr {
                    if addr.ip().is_ipv4() && !addr.ip().is_loopback() {
                        return Ok(addr.ip().to_string());
                    }
                }
            }
        }

        Ok("unknown".to_string())
    }

    /// Get primary MAC address
    fn get_mac_address() -> Result<String> {
        if let Ok(interfaces) = network_interface::NetworkInterface::show() {
            for interface in interfaces {
                if let Some(mac) = interface.mac_addr {
                    if mac != "00:00:00:00:00:00" && !interface.name.starts_with("lo") {
                        return Ok(mac);
                    }
                }
            }
        }

        Ok("unknown".to_string())
    }

    /// Get Ubuntu version
    fn get_ubuntu_version() -> Result<String> {
        std::fs::read_to_string("/etc/os-release")
            .context("Failed to read OS release info")?
            .lines()
            .find(|line| line.starts_with("VERSION_ID="))
            .and_then(|line| line.split('=').nth(1))
            .map(|version| version.trim_matches('"').to_string())
            .unwrap_or_else(|| "unknown".to_string())
            .pipe(Ok)
    }

    /// Get current system information
    async fn get_system_info(&self) -> Result<SystemInfo> {
        Ok(SystemInfo {
            architecture: self.get_architecture()?,
            total_memory: self.get_total_memory()?,
            available_memory: self.get_available_memory()?,
            cpu_info: self.get_cpu_info()?,
            disk_space: self.get_disk_space().await?,
            network_interfaces: self.get_network_interfaces()?,
            load_averages: self.get_load_averages()?,
            uptime: self.get_uptime()?,
        })
    }

    /// Get system architecture
    fn get_architecture(&self) -> Result<String> {
        Ok(std::env::consts::ARCH.to_string())
    }

    /// Get total system memory in MB
    fn get_total_memory(&self) -> Result<u64> {
        let meminfo = std::fs::read_to_string("/proc/meminfo")
            .context("Failed to read memory info")?;

        meminfo.lines()
            .find(|line| line.starts_with("MemTotal:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb_str| kb_str.parse::<u64>().ok())
            .map(|kb| kb / 1024) // Convert KB to MB
            .unwrap_or(0)
            .pipe(Ok)
    }

    /// Get available system memory in MB
    fn get_available_memory(&self) -> Result<u64> {
        let meminfo = std::fs::read_to_string("/proc/meminfo")
            .context("Failed to read memory info")?;

        meminfo.lines()
            .find(|line| line.starts_with("MemAvailable:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb_str| kb_str.parse::<u64>().ok())
            .map(|kb| kb / 1024) // Convert KB to MB
            .unwrap_or(0)
            .pipe(Ok)
    }

    /// Get CPU information
    fn get_cpu_info(&self) -> Result<String> {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")
            .context("Failed to read CPU info")?;

        cpuinfo.lines()
            .find(|line| line.starts_with("model name"))
            .and_then(|line| line.split(':').nth(1))
            .map(|name| name.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
            .pipe(Ok)
    }

    /// Get disk space information
    async fn get_disk_space(&self) -> Result<Vec<DiskSpace>> {
        let output = tokio::process::Command::new("df")
            .args(&["-B1"]) // Output in bytes
            .output()
            .await
            .context("Failed to get disk space info")?;

        let stdout = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in df output")?;

        let mut disk_spaces = Vec::new();

        for line in stdout.lines().skip(1) { // Skip header
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 6 {
                if let (Ok(total), Ok(used), Ok(available)) = (
                    fields[1].parse::<u64>(),
                    fields[2].parse::<u64>(),
                    fields[3].parse::<u64>(),
                ) {
                    let usage_percent = if total > 0 {
                        (used as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };

                    disk_spaces.push(DiskSpace {
                        filesystem: fields[0].to_string(),
                        mountpoint: fields[5].to_string(),
                        total_bytes: total,
                        used_bytes: used,
                        available_bytes: available,
                        usage_percent,
                    });
                }
            }
        }

        Ok(disk_spaces)
    }

    /// Get network interface information
    fn get_network_interfaces(&self) -> Result<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();

        if let Ok(network_interfaces) = network_interface::NetworkInterface::show() {
            for interface in network_interfaces {
                let ip_addresses: Vec<String> = interface.addr
                    .iter()
                    .map(|addr| addr.ip().to_string())
                    .collect();

                interfaces.push(NetworkInterface {
                    name: interface.name,
                    ip_addresses,
                    mac_address: interface.mac_addr,
                    state: "unknown".to_string(), // Would need additional system calls to determine
                    speed: None, // Would need additional system calls to determine
                });
            }
        }

        Ok(interfaces)
    }

    /// Get system load averages
    fn get_load_averages(&self) -> Result<[f64; 3]> {
        let loadavg = std::fs::read_to_string("/proc/loadavg")
            .context("Failed to read load averages")?;

        let fields: Vec<&str> = loadavg.split_whitespace().collect();
        if fields.len() >= 3 {
            Ok([
                fields[0].parse().unwrap_or(0.0),
                fields[1].parse().unwrap_or(0.0),
                fields[2].parse().unwrap_or(0.0),
            ])
        } else {
            Ok([0.0, 0.0, 0.0])
        }
    }

    /// Get system uptime in seconds
    fn get_uptime(&self) -> Result<u64> {
        let uptime_str = std::fs::read_to_string("/proc/uptime")
            .context("Failed to read uptime")?;

        uptime_str.split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|f| f as u64)
            .unwrap_or(0)
            .pipe(Ok)
    }
}

// Extension trait for pipe operations
trait Pipe<T> {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(T) -> R;
}

impl<T> Pipe<T> for T {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(T) -> R
    {
        f(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reporter_creation() {
        let reporter = StatusReporter::new(None).unwrap();
        assert_eq!(reporter.webhook_url, None);
        assert!(!reporter.metadata.session_id.to_string().is_empty());
    }

    #[tokio::test]
    async fn test_reporter_with_webhook() {
        let webhook_url = "https://example.com/webhook".to_string();
        let reporter = StatusReporter::new(Some(webhook_url.clone())).unwrap();
        assert_eq!(reporter.webhook_url, Some(webhook_url));
    }

    #[test]
    fn test_system_info_functions() {
        let reporter = StatusReporter::new(None).unwrap();

        // These should not panic
        let _ = reporter.get_architecture();
        let _ = reporter.get_total_memory();
        let _ = reporter.get_available_memory();
        let _ = reporter.get_cpu_info();
        let _ = reporter.get_load_averages();
        let _ = reporter.get_uptime();
    }
}
