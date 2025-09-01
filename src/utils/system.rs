// file: src/utils/system.rs
// version: 1.0.0
// guid: z8a9b0c1-d2e3-4567-8901-123456789012

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// System utilities for the installation agent
pub struct SystemUtils;

impl SystemUtils {
    /// Get system information
    pub async fn get_system_info() -> Result<SystemInfo> {
        debug!("Gathering system information");

        let hostname = Self::get_hostname().await?;
        let kernel_version = Self::get_kernel_version().await?;
        let os_release = Self::get_os_release().await?;
        let uptime = Self::get_uptime().await?;
        let load_average = Self::get_load_average().await?;
        let memory_info = Self::get_memory_info().await?;
        let cpu_info = Self::get_cpu_info().await?;
        let boot_time = Self::get_boot_time().await?;

        Ok(SystemInfo {
            hostname,
            kernel_version,
            os_release,
            uptime,
            load_average,
            memory_info,
            cpu_info,
            boot_time,
        })
    }

    /// Get hostname
    pub async fn get_hostname() -> Result<String> {
        let output = Command::new("hostname")
            .output()
            .await
            .context("Failed to execute hostname command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("hostname command failed"));
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Set hostname
    pub async fn set_hostname(hostname: &str) -> Result<()> {
        info!("Setting hostname to: {}", hostname);

        // Set current hostname
        let status = Command::new("hostnamectl")
            .args(&["set-hostname", hostname])
            .status()
            .await
            .context("Failed to execute hostnamectl")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to set hostname"));
        }

        // Update /etc/hosts
        let hosts_content = format!(
            "127.0.0.1\tlocalhost\n127.0.1.1\t{}\n\n# IPv6\n::1\tlocalhost ip6-localhost ip6-loopback\nff02::1\tip6-allnodes\nff02::2\tip6-allrouters\n",
            hostname
        );

        tokio::fs::write("/etc/hosts", hosts_content).await
            .context("Failed to update /etc/hosts")?;

        info!("Hostname set successfully");
        Ok(())
    }

    /// Get kernel version
    pub async fn get_kernel_version() -> Result<String> {
        let output = Command::new("uname")
            .arg("-r")
            .output()
            .await
            .context("Failed to execute uname command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("uname command failed"));
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Get OS release information
    pub async fn get_os_release() -> Result<OsRelease> {
        let content = tokio::fs::read_to_string("/etc/os-release").await
            .context("Failed to read /etc/os-release")?;

        let mut os_release = OsRelease::default();

        for line in content.lines() {
            if line.starts_with("NAME=") {
                os_release.name = line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string();
            } else if line.starts_with("VERSION=") {
                os_release.version = line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string();
            } else if line.starts_with("ID=") {
                os_release.id = line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string();
            } else if line.starts_with("VERSION_ID=") {
                os_release.version_id = line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string();
            } else if line.starts_with("PRETTY_NAME=") {
                os_release.pretty_name = line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string();
            } else if line.starts_with("VERSION_CODENAME=") {
                os_release.version_codename = Some(line.split('=').nth(1).unwrap_or("").trim_matches('"').to_string());
            }
        }

        Ok(os_release)
    }

    /// Get system uptime in seconds
    pub async fn get_uptime() -> Result<u64> {
        let content = tokio::fs::read_to_string("/proc/uptime").await
            .context("Failed to read /proc/uptime")?;

        let uptime_str = content.split_whitespace().next()
            .ok_or_else(|| anyhow::anyhow!("Invalid uptime format"))?;

        let uptime: f64 = uptime_str.parse()
            .context("Failed to parse uptime")?;

        Ok(uptime as u64)
    }

    /// Get load average
    pub async fn get_load_average() -> Result<LoadAverage> {
        let content = tokio::fs::read_to_string("/proc/loadavg").await
            .context("Failed to read /proc/loadavg")?;

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Invalid loadavg format"));
        }

        Ok(LoadAverage {
            load1: parts[0].parse().context("Failed to parse 1min load")?,
            load5: parts[1].parse().context("Failed to parse 5min load")?,
            load15: parts[2].parse().context("Failed to parse 15min load")?,
        })
    }

    /// Get memory information
    pub async fn get_memory_info() -> Result<MemoryInfo> {
        let content = tokio::fs::read_to_string("/proc/meminfo").await
            .context("Failed to read /proc/meminfo")?;

        let mut mem_info = MemoryInfo::default();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let value_kb: u64 = parts[1].parse().unwrap_or(0);
            let value_bytes = value_kb * 1024;

            match parts[0] {
                "MemTotal:" => mem_info.total = value_bytes,
                "MemFree:" => mem_info.free = value_bytes,
                "MemAvailable:" => mem_info.available = value_bytes,
                "Buffers:" => mem_info.buffers = value_bytes,
                "Cached:" => mem_info.cached = value_bytes,
                "SwapTotal:" => mem_info.swap_total = value_bytes,
                "SwapFree:" => mem_info.swap_free = value_bytes,
                _ => {}
            }
        }

        mem_info.used = mem_info.total - mem_info.free;
        mem_info.swap_used = mem_info.swap_total - mem_info.swap_free;

        Ok(mem_info)
    }

    /// Get CPU information
    pub async fn get_cpu_info() -> Result<CpuInfo> {
        let content = tokio::fs::read_to_string("/proc/cpuinfo").await
            .context("Failed to read /proc/cpuinfo")?;

        let mut cpu_info = CpuInfo::default();
        let mut core_count = 0;

        for line in content.lines() {
            if line.starts_with("processor") {
                core_count += 1;
            } else if line.starts_with("model name") {
                if cpu_info.model_name.is_empty() {
                    cpu_info.model_name = line.split(':').nth(1).unwrap_or("").trim().to_string();
                }
            } else if line.starts_with("cpu MHz") {
                if let Ok(freq) = line.split(':').nth(1).unwrap_or("0").trim().parse::<f64>() {
                    cpu_info.frequency_mhz = freq;
                }
            } else if line.starts_with("cache size") {
                if cpu_info.cache_size.is_empty() {
                    cpu_info.cache_size = line.split(':').nth(1).unwrap_or("").trim().to_string();
                }
            } else if line.starts_with("flags") {
                cpu_info.flags = line.split(':').nth(1).unwrap_or("")
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        cpu_info.core_count = core_count;
        Ok(cpu_info)
    }

    /// Get boot time as Unix timestamp
    pub async fn get_boot_time() -> Result<u64> {
        let content = tokio::fs::read_to_string("/proc/stat").await
            .context("Failed to read /proc/stat")?;

        for line in content.lines() {
            if line.starts_with("btime ") {
                let boot_time_str = line.split_whitespace().nth(1)
                    .ok_or_else(|| anyhow::anyhow!("Invalid btime format"))?;

                return boot_time_str.parse::<u64>()
                    .context("Failed to parse boot time");
            }
        }

        Err(anyhow::anyhow!("Boot time not found in /proc/stat"))
    }

    /// Check if a service is running
    pub async fn is_service_running(service_name: &str) -> Result<bool> {
        let output = Command::new("systemctl")
            .args(&["is-active", service_name])
            .output()
            .await
            .context("Failed to execute systemctl")?;

        Ok(output.status.success() &&
           String::from_utf8_lossy(&output.stdout).trim() == "active")
    }

    /// Start a systemd service
    pub async fn start_service(service_name: &str) -> Result<()> {
        info!("Starting service: {}", service_name);

        let status = Command::new("systemctl")
            .args(&["start", service_name])
            .status()
            .await
            .context("Failed to execute systemctl start")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to start service: {}", service_name));
        }

        Ok(())
    }

    /// Stop a systemd service
    pub async fn stop_service(service_name: &str) -> Result<()> {
        info!("Stopping service: {}", service_name);

        let status = Command::new("systemctl")
            .args(&["stop", service_name])
            .status()
            .await
            .context("Failed to execute systemctl stop")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to stop service: {}", service_name));
        }

        Ok(())
    }

    /// Enable a systemd service
    pub async fn enable_service(service_name: &str) -> Result<()> {
        info!("Enabling service: {}", service_name);

        let status = Command::new("systemctl")
            .args(&["enable", service_name])
            .status()
            .await
            .context("Failed to execute systemctl enable")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to enable service: {}", service_name));
        }

        Ok(())
    }

    /// Disable a systemd service
    pub async fn disable_service(service_name: &str) -> Result<()> {
        info!("Disabling service: {}", service_name);

        let status = Command::new("systemctl")
            .args(&["disable", service_name])
            .status()
            .await
            .context("Failed to execute systemctl disable")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to disable service: {}", service_name));
        }

        Ok(())
    }

    /// Restart a systemd service
    pub async fn restart_service(service_name: &str) -> Result<()> {
        info!("Restarting service: {}", service_name);

        let status = Command::new("systemctl")
            .args(&["restart", service_name])
            .status()
            .await
            .context("Failed to execute systemctl restart")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to restart service: {}", service_name));
        }

        Ok(())
    }

    /// Get service status
    pub async fn get_service_status(service_name: &str) -> Result<ServiceStatus> {
        let output = Command::new("systemctl")
            .args(&["show", service_name, "--property=ActiveState,LoadState,SubState"])
            .output()
            .await
            .context("Failed to execute systemctl show")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get service status for: {}", service_name));
        }

        let output_str = String::from_utf8(output.stdout)?;
        let mut active_state = String::new();
        let mut load_state = String::new();
        let mut sub_state = String::new();

        for line in output_str.lines() {
            if line.starts_with("ActiveState=") {
                active_state = line.split('=').nth(1).unwrap_or("").to_string();
            } else if line.starts_with("LoadState=") {
                load_state = line.split('=').nth(1).unwrap_or("").to_string();
            } else if line.starts_with("SubState=") {
                sub_state = line.split('=').nth(1).unwrap_or("").to_string();
            }
        }

        Ok(ServiceStatus {
            name: service_name.to_string(),
            active_state,
            load_state,
            sub_state,
        })
    }

    /// List all systemd services
    pub async fn list_services() -> Result<Vec<ServiceStatus>> {
        let output = Command::new("systemctl")
            .args(&["list-units", "--type=service", "--all", "--no-legend", "--plain"])
            .output()
            .await
            .context("Failed to execute systemctl list-units")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to list services"));
        }

        let output_str = String::from_utf8(output.stdout)?;
        let mut services = Vec::new();

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let service_name = parts[0].trim_end_matches(".service");
                services.push(ServiceStatus {
                    name: service_name.to_string(),
                    active_state: parts[2].to_string(),
                    load_state: parts[1].to_string(),
                    sub_state: parts[3].to_string(),
                });
            }
        }

        Ok(services)
    }

    /// Check if system is running under systemd
    pub async fn is_systemd() -> Result<bool> {
        Ok(Path::new("/run/systemd/system").exists())
    }

    /// Get environment variables
    pub fn get_environment() -> HashMap<String, String> {
        std::env::vars().collect()
    }

    /// Set environment variable
    pub fn set_environment_var(key: &str, value: &str) {
        std::env::set_var(key, value);
    }

    /// Get current user ID
    pub fn get_current_uid() -> u32 {
        unsafe { libc::getuid() }
    }

    /// Get current group ID
    pub fn get_current_gid() -> u32 {
        unsafe { libc::getgid() }
    }

    /// Check if running as root
    pub fn is_root() -> bool {
        Self::get_current_uid() == 0
    }

    /// Get current working directory
    pub fn get_current_dir() -> Result<PathBuf> {
        std::env::current_dir()
            .context("Failed to get current directory")
    }

    /// Change working directory
    pub fn change_dir<P: AsRef<Path>>(path: P) -> Result<()> {
        std::env::set_current_dir(path)
            .context("Failed to change directory")
    }

    /// Execute a command with output capture
    pub async fn execute_command(cmd: &str, args: &[&str]) -> Result<CommandOutput> {
        let start_time = std::time::Instant::now();

        let output = Command::new(cmd)
            .args(args)
            .output()
            .await
            .with_context(|| format!("Failed to execute command: {} {:?}", cmd, args))?;

        let duration = start_time.elapsed();

        Ok(CommandOutput {
            command: format!("{} {}", cmd, args.join(" ")),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration,
            success: output.status.success(),
        })
    }

    /// Execute a command with real-time output
    pub async fn execute_command_streaming(
        cmd: &str,
        args: &[&str],
        timeout: Option<Duration>,
    ) -> Result<i32> {
        let mut child = Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn command: {} {:?}", cmd, args))?;

        let exit_status = if let Some(timeout_duration) = timeout {
            match tokio::time::timeout(timeout_duration, child.wait()).await {
                Ok(status) => status?,
                Err(_) => {
                    warn!("Command timed out: {} {:?}", cmd, args);
                    child.kill().await?;
                    return Err(anyhow::anyhow!("Command timed out after {:?}", timeout_duration));
                }
            }
        } else {
            child.wait().await?
        };

        Ok(exit_status.code().unwrap_or(-1))
    }

    /// Check if a process is running
    pub async fn is_process_running(process_name: &str) -> Result<bool> {
        let output = Command::new("pgrep")
            .arg(process_name)
            .output()
            .await
            .context("Failed to execute pgrep")?;

        Ok(output.status.success() && !output.stdout.is_empty())
    }

    /// Kill a process by name
    pub async fn kill_process(process_name: &str, signal: Option<&str>) -> Result<()> {
        let signal = signal.unwrap_or("TERM");

        let status = Command::new("pkill")
            .args(&["-f", &format!("-{}", signal), process_name])
            .status()
            .await
            .context("Failed to execute pkill")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to kill process: {}", process_name));
        }

        Ok(())
    }

    /// Get system timezone
    pub async fn get_timezone() -> Result<String> {
        let output = Command::new("timedatectl")
            .args(&["show", "--property=Timezone", "--value"])
            .output()
            .await
            .context("Failed to execute timedatectl")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get timezone"));
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Set system timezone
    pub async fn set_timezone(timezone: &str) -> Result<()> {
        info!("Setting timezone to: {}", timezone);

        let status = Command::new("timedatectl")
            .args(&["set-timezone", timezone])
            .status()
            .await
            .context("Failed to execute timedatectl")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to set timezone: {}", timezone));
        }

        Ok(())
    }

    /// Get current system time as Unix timestamp
    pub fn get_current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Sync system clock
    pub async fn sync_system_clock() -> Result<()> {
        info!("Syncing system clock");

        let status = Command::new("timedatectl")
            .args(&["set-ntp", "true"])
            .status()
            .await
            .context("Failed to execute timedatectl set-ntp")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to enable NTP sync"));
        }

        Ok(())
    }

    /// Reboot the system
    pub async fn reboot(delay_seconds: Option<u32>) -> Result<()> {
        let delay = delay_seconds.unwrap_or(0);

        warn!("System reboot scheduled in {} seconds", delay);

        if delay > 0 {
            tokio::time::sleep(Duration::from_secs(delay as u64)).await;
        }

        let status = Command::new("systemctl")
            .arg("reboot")
            .status()
            .await
            .context("Failed to execute reboot")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to reboot system"));
        }

        Ok(())
    }

    /// Shutdown the system
    pub async fn shutdown(delay_seconds: Option<u32>) -> Result<()> {
        let delay = delay_seconds.unwrap_or(0);

        warn!("System shutdown scheduled in {} seconds", delay);

        if delay > 0 {
            tokio::time::sleep(Duration::from_secs(delay as u64)).await;
        }

        let status = Command::new("systemctl")
            .arg("poweroff")
            .status()
            .await
            .context("Failed to execute shutdown")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to shutdown system"));
        }

        Ok(())
    }
}

/// Complete system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub kernel_version: String,
    pub os_release: OsRelease,
    pub uptime: u64,
    pub load_average: LoadAverage,
    pub memory_info: MemoryInfo,
    pub cpu_info: CpuInfo,
    pub boot_time: u64,
}

/// Operating system release information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsRelease {
    pub name: String,
    pub version: String,
    pub id: String,
    pub version_id: String,
    pub pretty_name: String,
    pub version_codename: Option<String>,
}

/// System load average
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAverage {
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
}

/// Memory information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub free: u64,
    pub available: u64,
    pub used: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub swap_used: u64,
}

/// CPU information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model_name: String,
    pub core_count: u32,
    pub frequency_mhz: f64,
    pub cache_size: String,
    pub flags: Vec<String>,
}

/// Systemd service status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub active_state: String,
    pub load_state: String,
    pub sub_state: String,
}

/// Command execution output
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_hostname() {
        match SystemUtils::get_hostname().await {
            Ok(hostname) => {
                assert!(!hostname.is_empty());
                println!("Hostname: {}", hostname);
            }
            Err(e) => {
                println!("Could not get hostname: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_kernel_version() {
        match SystemUtils::get_kernel_version().await {
            Ok(version) => {
                assert!(!version.is_empty());
                println!("Kernel version: {}", version);
            }
            Err(e) => {
                println!("Could not get kernel version: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_os_release() {
        match SystemUtils::get_os_release().await {
            Ok(os_release) => {
                assert!(!os_release.name.is_empty());
                println!("OS: {} {}", os_release.name, os_release.version);
            }
            Err(e) => {
                println!("Could not get OS release: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_uptime() {
        match SystemUtils::get_uptime().await {
            Ok(uptime) => {
                assert!(uptime > 0);
                println!("Uptime: {} seconds", uptime);
            }
            Err(e) => {
                println!("Could not get uptime: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_memory_info() {
        match SystemUtils::get_memory_info().await {
            Ok(mem_info) => {
                assert!(mem_info.total > 0);
                println!("Memory: {} MB total, {} MB free",
                    mem_info.total / 1024 / 1024,
                    mem_info.free / 1024 / 1024);
            }
            Err(e) => {
                println!("Could not get memory info: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_is_systemd() {
        let is_systemd = SystemUtils::is_systemd().await.unwrap_or(false);
        println!("Running under systemd: {}", is_systemd);
    }

    #[test]
    fn test_get_current_uid() {
        let uid = SystemUtils::get_current_uid();
        println!("Current UID: {}", uid);
    }

    #[test]
    fn test_is_root() {
        let is_root = SystemUtils::is_root();
        println!("Running as root: {}", is_root);
    }

    #[tokio::test]
    async fn test_execute_command() {
        match SystemUtils::execute_command("echo", &["hello", "world"]).await {
            Ok(output) => {
                assert!(output.success);
                assert_eq!(output.stdout.trim(), "hello world");
                println!("Command output: {}", output.stdout.trim());
            }
            Err(e) => {
                println!("Command execution failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_system_info() {
        match SystemUtils::get_system_info().await {
            Ok(info) => {
                println!("System info gathered successfully");
                println!("Hostname: {}", info.hostname);
                println!("OS: {}", info.os_release.pretty_name);
                println!("Kernel: {}", info.kernel_version);
                println!("Uptime: {} seconds", info.uptime);
                println!("Load: {:.2} {:.2} {:.2}",
                    info.load_average.load1,
                    info.load_average.load5,
                    info.load_average.load15);
                println!("Memory: {} MB total", info.memory_info.total / 1024 / 1024);
                println!("CPU: {} ({} cores)",
                    info.cpu_info.model_name,
                    info.cpu_info.core_count);
            }
            Err(e) => {
                println!("Could not get system info: {}", e);
            }
        }
    }
}
