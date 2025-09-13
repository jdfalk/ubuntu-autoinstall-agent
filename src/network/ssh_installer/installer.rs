// file: src/network/ssh_installer/installer.rs
// version: 1.0.0
// guid: sshins01-2345-6789-abcd-ef0123456789

//! Main SSH installer orchestrating all installation phases

use std::collections::HashMap;
use tracing::{info, error};
use crate::network::SshClient;
use crate::Result;
use super::config::{InstallationConfig, SystemInfo};
use super::investigation::SystemInvestigator;
use super::packages::PackageManager;
use super::disk_ops::DiskManager;
use super::zfs_ops::ZfsManager;
use super::system_setup::SystemConfigurator;

/// SSH-based installer for Ubuntu with ZFS and LUKS
pub struct SshInstaller {
    ssh: SshClient,
    connected: bool,
    variables: HashMap<String, String>,
}

impl SshInstaller {
    /// Create a new SSH installer
    pub fn new() -> Self {
        Self {
            ssh: SshClient::new(),
            connected: false,
            variables: HashMap::new(),
        }
    }

    /// Connect to target system
    pub async fn connect(&mut self, host: &str, username: &str) -> Result<()> {
        self.ssh.connect(host, username).await?;
        self.connected = true;
        info!("Successfully connected to {}@{}", username, host);
        Ok(())
    }

    /// Perform comprehensive system investigation
    pub async fn investigate_system(&mut self) -> Result<SystemInfo> {
        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string()
            ));
        }

        let mut investigator = SystemInvestigator::new(&mut self.ssh);
        investigator.investigate_system().await
    }

    /// Perform full ZFS + LUKS installation with comprehensive error handling
    pub async fn perform_installation(&mut self, config: &InstallationConfig) -> Result<()> {
        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string()
            ));
        }

        info!("Starting full ZFS + LUKS installation for {}", config.hostname);

        let mut failed_phases = Vec::new();
        let mut successful_phases = Vec::new();

        // Phase 0: Setup installation variables
        match self.setup_installation_variables(config).await {
            Ok(_) => {
                info!("✓ Phase 0 completed: Setup variables");
                successful_phases.push("Phase 0: Setup variables");
            }
            Err(e) => {
                error!("✗ Phase 0 failed - Setup variables: {}", e);
                failed_phases.push(format!("Phase 0: Setup variables - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Phase 1: Package installation (continue even if previous phase failed)
        match self.phase_1_package_installation().await {
            Ok(_) => {
                info!("✓ Phase 1 completed: Package installation");
                successful_phases.push("Phase 1: Package installation");
            }
            Err(e) => {
                error!("✗ Phase 1 failed - Package installation: {}", e);
                failed_phases.push(format!("Phase 1: Package installation - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Phase 2: Disk preparation
        match self.phase_2_disk_preparation(config).await {
            Ok(_) => {
                info!("✓ Phase 2 completed: Disk preparation");
                successful_phases.push("Phase 2: Disk preparation");
            }
            Err(e) => {
                error!("✗ Phase 2 failed - Disk preparation: {}", e);
                failed_phases.push(format!("Phase 2: Disk preparation - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Phase 3: ZFS pool creation
        match self.phase_3_zfs_creation(config).await {
            Ok(_) => {
                info!("✓ Phase 3 completed: ZFS creation");
                successful_phases.push("Phase 3: ZFS creation");
            }
            Err(e) => {
                error!("✗ Phase 3 failed - ZFS creation: {}", e);
                failed_phases.push(format!("Phase 3: ZFS creation - {}", e));
                self.collect_and_log_debug_info().await;
                // Continue to next phases for complete error analysis
            }
        }

        // Phase 4: Base system installation
        match self.phase_4_base_system(config).await {
            Ok(_) => {
                info!("✓ Phase 4 completed: Base system");
                successful_phases.push("Phase 4: Base system");
            }
            Err(e) => {
                error!("✗ Phase 4 failed - Base system: {}", e);
                failed_phases.push(format!("Phase 4: Base system - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Phase 5: System configuration
        match self.phase_5_system_configuration(config).await {
            Ok(_) => {
                info!("✓ Phase 5 completed: System configuration");
                successful_phases.push("Phase 5: System configuration");
            }
            Err(e) => {
                error!("✗ Phase 5 failed - System configuration: {}", e);
                failed_phases.push(format!("Phase 5: System configuration - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Phase 6: Final setup
        match self.phase_6_final_setup(config).await {
            Ok(_) => {
                info!("✓ Phase 6 completed: Final setup");
                successful_phases.push("Phase 6: Final setup");
            }
            Err(e) => {
                error!("✗ Phase 6 failed - Final setup: {}", e);
                failed_phases.push(format!("Phase 6: Final setup - {}", e));
                self.collect_and_log_debug_info().await;
            }
        }

        // Generate comprehensive installation report
        self.generate_installation_report(&successful_phases, &failed_phases).await;

        if failed_phases.is_empty() {
            info!("🎉 Installation completed successfully for {}", config.hostname);
            Ok(())
        } else {
            error!("❌ Installation completed with {} failed phases out of 6 total phases", failed_phases.len());
            error!("💡 SSH session remains active for manual debugging and investigation");
            error!("💡 You can inspect logs, retry specific phases, or analyze the system state");

            // Don't disconnect - let the user investigate
            Err(crate::error::AutoInstallError::InstallationError(
                format!("Installation failed: {} phases failed", failed_phases.len())
            ))
        }
    }

    /// Collect and log debug information
    async fn collect_and_log_debug_info(&mut self) {
        info!("Collecting debug information for troubleshooting...");
        match self.ssh.collect_debug_info().await {
            Ok(debug_info) => {
                error!("=== DEBUG INFORMATION ===");
                error!("{}", debug_info);
                error!("=== END DEBUG INFORMATION ===");
            }
            Err(e) => {
                error!("Failed to collect debug information: {}", e);
            }
        }
    }

    /// Generate comprehensive installation report
    async fn generate_installation_report(&mut self, successful_phases: &[&str], failed_phases: &[String]) {
        info!("=== INSTALLATION REPORT ===");
        info!("Total phases: 6");
        info!("Successful phases: {}", successful_phases.len());
        info!("Failed phases: {}", failed_phases.len());

        if !successful_phases.is_empty() {
            info!("✓ SUCCESSFUL PHASES:");
            for phase in successful_phases {
                info!("  ✓ {}", phase);
            }
        }

        if !failed_phases.is_empty() {
            error!("✗ FAILED PHASES:");
            for phase in failed_phases {
                error!("  ✗ {}", phase);
            }

            error!("📋 DEBUGGING GUIDE:");
            error!("  • SSH session is still active - you can manually inspect the system");
            error!("  • Check /var/log/syslog for system messages");
            error!("  • Run 'dmesg' for kernel messages");
            error!("  • Check 'zpool status' for ZFS pool information");
            error!("  • Check 'cryptsetup status luks' for LUKS status");
            error!("  • Use 'lsblk' to see current disk layout");
            error!("  • Run 'mount' to see mounted filesystems");

            error!("🔧 COMMON FIXES:");
            error!("  • For ZFS issues: Check if all required packages are installed");
            error!("  • For disk issues: Verify the correct disk device path");
            error!("  • For LUKS issues: Check if cryptsetup is working properly");
            error!("  • For mount issues: Check if mount points exist and are accessible");
        }

        info!("=== END INSTALLATION REPORT ===");
    }

    /// Setup installation variables
    async fn setup_installation_variables(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up installation variables");

        // Stop unnecessary services
        self.ssh.execute("systemctl stop zed || true").await?;

        // Configure timezone
        self.ssh.execute(&format!("timedatectl set-timezone {}", config.timezone)).await?;
        self.ssh.execute("timedatectl set-ntp on").await?;

        // Set environment variables
        let vars = vec![
            ("DISK", &config.disk_device),
            ("TIMEZONE", &config.timezone),
            ("HOSTNAME", &config.hostname),
            ("LUKS_KEY", &config.luks_key),
            ("ROOT_PASSWORD", &config.root_password),
            ("NET_ET_INTERFACE", &config.network_interface),
            ("NET_ET_ADDRESS", &config.network_address),
            ("NET_ET_GATEWAY", &config.network_gateway),
            ("NET_ET_SEARCH", &config.network_search),
        ];

        for (key, value) in vars {
            self.ssh.execute(&format!("export {}='{}'", key, value)).await?;
            self.variables.insert(key.to_string(), value.to_string());
        }

        // Set nameservers array
        let nameservers = config.network_nameservers.join(" ");
        self.ssh.execute(&format!("export NET_ET_NAMESERVERS=({})", nameservers)).await?;

        Ok(())
    }

    /// Phase 1: Install required packages
    async fn phase_1_package_installation(&mut self) -> Result<()> {
        info!("Phase 1: Package installation");

        let mut package_manager = PackageManager::new(&mut self.ssh);
        package_manager.install_required_packages().await?;

        info!("Phase 1 completed: Required packages installed");
        Ok(())
    }

    /// Phase 2: Disk preparation and partitioning
    async fn phase_2_disk_preparation(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 2: Disk preparation and partitioning");

        let mut disk_manager = DiskManager::new(&mut self.ssh);
        disk_manager.prepare_disk(config).await?;

        info!("Phase 2 completed: Disk preparation and partitioning");
        Ok(())
    }

    /// Phase 3: ZFS pool and dataset creation
    async fn phase_3_zfs_creation(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 3: ZFS pool and dataset creation");

        let mut zfs_manager = ZfsManager::new(&mut self.ssh, &mut self.variables);
        zfs_manager.create_zfs_pools(config).await?;

        info!("Phase 3 completed: ZFS pools and datasets created");
        Ok(())
    }

    /// Phase 4: Base system installation
    async fn phase_4_base_system(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 4: Base system installation");

        let mut system_configurator = SystemConfigurator::new(&mut self.ssh);
        system_configurator.install_base_system(config).await?;

        info!("Phase 4 completed: Base system installed");
        Ok(())
    }

    /// Phase 5: System configuration
    async fn phase_5_system_configuration(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 5: System configuration");

        let mut system_configurator = SystemConfigurator::new(&mut self.ssh);

        // Configure ZFS
        system_configurator.configure_zfs_in_chroot().await?;

        // Configure GRUB
        system_configurator.configure_grub_in_chroot(config).await?;

        // Setup LUKS key
        system_configurator.setup_luks_key_in_chroot(config).await?;

        info!("Phase 5 completed: System configuration");
        Ok(())
    }

    /// Phase 6: Final setup and cleanup
    async fn phase_6_final_setup(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 6: Final setup and cleanup");

        let mut system_configurator = SystemConfigurator::new(&mut self.ssh);
        system_configurator.final_cleanup(config).await?;

        info!("Phase 6 completed: Final setup and cleanup");
        info!("Installation of {} completed successfully!", config.hostname);
        Ok(())
    }
}

impl Default for SshInstaller {
    fn default() -> Self {
        Self::new()
    }
}
