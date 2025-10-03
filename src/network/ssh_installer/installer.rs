// file: src/network/ssh_installer/installer.rs
// version: 1.11.1
// guid: sshins01-2345-6789-abcd-ef0123456789

//! Main SSH installer orchestrating all installation phases

use super::config::{InstallationConfig, SystemInfo};
use super::disk_ops::DiskManager;
use super::investigation::SystemInvestigator;
use super::packages::PackageManager;
use super::system_setup::SystemConfigurator;
use super::zfs_ops::ZfsManager;
use crate::network::SshClient;
use crate::Result;
use std::collections::HashMap;
use tracing::{error, info};

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

    /// Perform installation with additional options (e.g., hold-on-failure, pause-after-storage)
    pub async fn perform_installation_with_options_and_pause(
        &mut self,
        config: &InstallationConfig,
        hold_on_failure: bool,
        pause_after_storage: bool,
    ) -> Result<()> {
        if !hold_on_failure && !pause_after_storage {
            return self.perform_installation(config).await;
        }

        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string(),
            ));
        }

        info!(
            "Starting full ZFS + LUKS installation for {} (hold-on-failure={}, pause-after-storage={})",
            config.hostname, hold_on_failure, pause_after_storage
        );

        let mut failed_phases: Vec<String> = Vec::new();
        let mut successful_phases: Vec<&str> = Vec::new();

        // Preflight checks (continue for diagnostics even if failing)
        if let Err(e) = self.preflight_checks(config).await {
            error!("✗ Preflight checks failed: {}", e);
            // Do not enter hold on preflight to allow env setup and better diagnostics
        } else {
            info!("✓ Preflight checks passed");
        }

        // Phase 0: Setup installation variables
        if let Err(e) = self.setup_installation_variables(config).await {
            failed_phases.push(format!("Phase 0: Setup variables - {}", e));
            return self
                .enter_hold_mode("Phase 0 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 0: Setup variables");
        }

        // Phase 1: Package installation
        if let Err(e) = self.phase_1_package_installation().await {
            failed_phases.push(format!("Phase 1: Package installation - {}", e));
            return self
                .enter_hold_mode("Phase 1 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 1: Package installation");
        }

        // Phase 2: Disk preparation
        if let Err(e) = self.phase_2_disk_preparation(config).await {
            failed_phases.push(format!("Phase 2: Disk preparation - {}", e));
            return self
                .enter_hold_mode("Phase 2 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 2: Disk preparation");
        }

        // Phase 3: ZFS pool creation
        if let Err(e) = self.phase_3_zfs_creation(config).await {
            failed_phases.push(format!("Phase 3: ZFS creation - {}", e));
            return self
                .enter_hold_mode("Phase 3 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 3: ZFS creation");
        }

        // Optional pause after storage creation to allow manual verification and steps
        if pause_after_storage {
            self.print_next_commands_after_storage(config).await?;
            return self
                .enter_hold_mode(
                    "Paused after storage per user request",
                    &successful_phases,
                    &failed_phases,
                )
                .await;
        }

        // Phase 4: Base system installation
        if let Err(e) = self.phase_4_base_system(config).await {
            failed_phases.push(format!("Phase 4: Base system - {}", e));
            return self
                .enter_hold_mode("Phase 4 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 4: Base system");
        }

        // Phase 5: System configuration
        if let Err(e) = self.phase_5_system_configuration(config).await {
            failed_phases.push(format!("Phase 5: System configuration - {}", e));
            return self
                .enter_hold_mode("Phase 5 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 5: System configuration");
        }

        // Phase 6: Final setup — in hold mode we still want to complete when all previous phases succeeded
        if let Err(e) = self.phase_6_final_setup(config).await {
            failed_phases.push(format!("Phase 6: Final setup - {}", e));
            return self
                .enter_hold_mode("Phase 6 failed", &successful_phases, &failed_phases)
                .await;
        } else {
            successful_phases.push("Phase 6: Final setup");
        }

        // All good
        self.generate_installation_report(&successful_phases, &failed_phases)
            .await;
        info!(
            "🎉 Installation completed successfully for {}",
            config.hostname
        );
        Ok(())
    }

    /// Print the next commands that would be executed post-storage so the user can run them manually
    async fn print_next_commands_after_storage(
        &mut self,
        config: &InstallationConfig,
    ) -> Result<()> {
        use tracing::warn;
        warn!("=== PAUSE AFTER STORAGE REQUESTED ===");
        warn!(
            "The installer has completed: partitioning, formatting (ESP/ext4), LUKS setup, and ZFS pools/datasets."
        );
        warn!(
            "The next commands that would be executed are listed below. You can run them manually on the target."
        );

        let cmds = build_next_commands_after_storage(config);
        for c in cmds {
            warn!("  {}", c);
        }
        warn!("=== END OF NEXT COMMANDS ===");
        Ok(())
    }

    /// Enter hold mode: stop immediately, write logs, generate report, and keep SSH session open
    async fn enter_hold_mode(
        &mut self,
        reason: &str,
        successful_phases: &[&str],
        failed_phases: &[String],
    ) -> Result<()> {
        error!(
            "🔒 Hold-on-failure is enabled — stopping immediately: {}",
            reason
        );
        self.collect_and_log_debug_info().await;
        self.generate_installation_report(successful_phases, failed_phases)
            .await;

        // IMPORTANT: Do NOT cleanup/unmount/export anything here — leave the system as-is
        // Keep the SSH session alive for live debugging by running a long-lived no-op on the target
        // We intentionally block here to keep the process and SSH session open
        let keepalive_cmd = "bash -lc 'echo \"[uaa] Hold mode active — leaving system mounted for debugging.\"; echo \"Press Ctrl-C locally when done.\"; while true; do sleep 3600; done'";
        let _ = self.ssh.execute(keepalive_cmd).await;

        Err(crate::error::AutoInstallError::InstallationError(
            "Installation halted due to failure (hold-on-failure)".to_string(),
        ))
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
                "Not connected to target system".to_string(),
            ));
        }

        let mut investigator = SystemInvestigator::new(&mut self.ssh);
        investigator.investigate_system().await
    }

    /// Perform full ZFS + LUKS installation with comprehensive error handling
    pub async fn perform_installation(&mut self, config: &InstallationConfig) -> Result<()> {
        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string(),
            ));
        }

        info!(
            "Starting full ZFS + LUKS installation for {}",
            config.hostname
        );

        let mut failed_phases = Vec::new();
        let mut successful_phases = Vec::new();

        // Preflight checks (Phase -1)
        match self.preflight_checks(config).await {
            Ok(_) => {
                info!("✓ Preflight checks passed");
            }
            Err(e) => {
                error!("✗ Preflight checks failed: {}", e);
                self.collect_and_log_debug_info().await;
                // Continue to attempt installation for maximum diagnostics
            }
        }

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
        self.generate_installation_report(&successful_phases, &failed_phases)
            .await;

        if failed_phases.is_empty() {
            info!(
                "🎉 Installation completed successfully for {}",
                config.hostname
            );
            Ok(())
        } else {
            error!(
                "❌ Installation completed with {} failed phases out of 6 total phases",
                failed_phases.len()
            );
            error!("💡 SSH session remains active for manual debugging and investigation");
            error!("💡 You can inspect logs, retry specific phases, or analyze the system state");

            // Don't disconnect - let the user investigate
            Err(crate::error::AutoInstallError::InstallationError(format!(
                "Installation failed: {} phases failed",
                failed_phases.len()
            )))
        }
    }

    /// Preflight validation: networking, mirrors, mountpoints, and existing state
    async fn preflight_checks(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Running preflight checks");

        // 1) Basic network connectivity
        let ping_status = self
            .ssh
            .execute(
                "ping -c 1 -w 2 1.1.1.1 >/dev/null 2>&1 || ping -c 1 -w 2 8.8.8.8 >/dev/null 2>&1",
            )
            .await;
        if ping_status.is_err() {
            return Err(crate::error::AutoInstallError::ValidationError(
                "No basic network connectivity (ICMP)".to_string(),
            ));
        }

        // 2) Check debootstrap mirror reachability
        let release = config.debootstrap_release.as_deref().unwrap_or("plucky");
        let mirror = config
            .debootstrap_mirror
            .as_deref()
            .unwrap_or("http://archive.ubuntu.com/ubuntu/");
        let release_url = format!("{}/dists/{}/Release", mirror.trim_end_matches('/'), release);
        let head_cmd = format!("curl -fsI '{}' >/dev/null", release_url);
        if self.ssh.execute(&head_cmd).await.is_err() {
            // Try old-releases as backup if not already
            let fallback_url = format!(
                "http://old-releases.ubuntu.com/ubuntu/dists/{}/Release",
                release
            );
            let fallback_cmd = format!("curl -fsI '{}' >/dev/null", fallback_url);
            if self.ssh.execute(&fallback_cmd).await.is_err() {
                return Err(crate::error::AutoInstallError::ValidationError(format!(
                    "Debootstrap mirror not reachable for {}",
                    release
                )));
            } else {
                info!("Mirror check: primary unreachable; old-releases is reachable");
            }
        }

        // 3) Ensure target mount path is sane
        // Create if missing, and warn if non-empty
        self.ssh.execute("mkdir -p /mnt/targetos").await?;
        let non_empty_check = self
            .ssh
            .execute("test -z \"$(ls -A /mnt/targetos 2>/dev/null)\"")
            .await;
        if non_empty_check.is_err() {
            info!("Preflight: /mnt/targetos is not empty; installation will proceed carefully");
        }

        // 4) Detect existing pools to avoid duplicate creation
        let has_bpool = self
            .ssh
            .check_silent("zpool list -H bpool >/dev/null 2>&1")
            .await
            .unwrap_or(false);
        let has_rpool = self
            .ssh
            .check_silent("zpool list -H rpool >/dev/null 2>&1")
            .await
            .unwrap_or(false);
        if has_bpool || has_rpool {
            info!(
                "Preflight: existing pools detected: bpool={} rpool={}",
                has_bpool, has_rpool
            );
        }

        // 5) LUKS and residual mounts check; recover if needed
        let luks_active = self
            .ssh
            .check_silent("cryptsetup status luks >/dev/null 2>&1")
            .await
            .unwrap_or(false);
        let luks_mounted = false; // we do not mount the LUKS mapper as a filesystem
        let target_has_mounts = self
            .ssh
            .check_silent("mount | grep -q '/mnt/targetos' ")
            .await
            .unwrap_or(false);
        let pools_exist = self
            .ssh
            .check_silent("zpool list -H bpool >/dev/null 2>&1")
            .await
            .unwrap_or(false)
            || self
                .ssh
                .check_silent("zpool list -H rpool >/dev/null 2>&1")
                .await
                .unwrap_or(false);

        if luks_active || luks_mounted || target_has_mounts || pools_exist {
            info!(
                "Preflight: residual state detected (luks_active={}, luks_mounted={}, target_mounts={}, pools_exist={}); attempting recovery/reset",
                luks_active, luks_mounted, target_has_mounts, pools_exist
            );
            let mut disk_manager = DiskManager::new(&mut self.ssh);
            // Best-effort recovery; if it fails we'll still attempt to proceed to capture diagnostics
            let _ = disk_manager.recover_after_failure_and_wipe(config).await;
        } else {
            info!("Preflight: no residual mounts or LUKS/ZFS state detected");
        }

        Ok(())
    }

    /// Collect and log debug information
    async fn collect_and_log_debug_info(&mut self) {
        info!("Collecting debug information for troubleshooting...");
        match self.ssh.collect_debug_info().await {
            Ok(debug_info) => {
                error!("=== DEBUG INFORMATION ===");
                error!("{}", debug_info);
                error!("=== END DEBUG INFORMATION ===");

                // Persist logs remotely and fetch them locally for archives
                // Create a timestamp from UNIX epoch seconds (avoid extra deps)
                let ts = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(dur) => dur.as_secs().to_string(),
                    Err(_) => "0".to_string(),
                };
                let remote_dir = "/var/tmp/uaalogs";
                let remote_path = format!("{}/install-debug-{}.log", remote_dir, ts);
                let _ = self.ssh.execute(&format!("mkdir -p {}", remote_dir)).await;
                let _ = self
                    .ssh
                    .execute(&format!(
                        "bash -lc 'cat > {} <<\'EOF\'\n{}\nEOF'",
                        remote_path,
                        debug_info.replace("'", "'\\''")
                    ))
                    .await;

                // Download to local logs folder
                let base_dir = std::env::current_dir()
                    .ok()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| ".".to_string());
                let local_dir = format!(
                    "{}/logs/{}",
                    base_dir,
                    self.variables
                        .get("HOSTNAME")
                        .cloned()
                        .unwrap_or_else(|| "unknown-host".to_string())
                );
                let _ = std::fs::create_dir_all(&local_dir);
                let local_path = format!(
                    "{}/{}",
                    local_dir,
                    std::path::Path::new(&remote_path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                );
                if let Err(e) = self.ssh.download_file(&remote_path, &local_path).await {
                    error!("Failed to download debug log: {}", e);
                } else {
                    info!("Saved debug log to {}", local_path);
                }
            }
            Err(e) => {
                error!("Failed to collect debug information: {}", e);
            }
        }
    }

    /// Generate comprehensive installation report
    async fn generate_installation_report(
        &mut self,
        successful_phases: &[&str],
        failed_phases: &[String],
    ) {
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
        self.ssh
            .execute(&format!("timedatectl set-timezone {}", config.timezone))
            .await?;
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
            self.ssh
                .execute(&format!("export {}='{}'", key, value))
                .await?;
            self.variables.insert(key.to_string(), value.to_string());
        }

        // Set nameservers array
        let nameservers = config.network_nameservers.join(" ");
        self.ssh
            .execute(&format!("export NET_ET_NAMESERVERS=({})", nameservers))
            .await?;

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
        info!(
            "Installation of {} completed successfully!",
            config.hostname
        );
        Ok(())
    }
}

impl Default for SshInstaller {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the list of commands that would run after storage is prepared, for testing and pause-after-storage preview
pub(super) fn build_next_commands_after_storage(config: &InstallationConfig) -> Vec<String> {
    let esp_part = format!("{}p1", config.disk_device);
    let release = config.debootstrap_release.as_deref().unwrap_or("plucky");
    vec![
        // Mount target root and boot/EFI
        "mkdir -p /mnt/targetos/boot/efi".to_string(),
        format!("mount {} /mnt/targetos/boot/efi", esp_part),

        // Debootstrap base system (release), try primary mirror then old-releases
        format!(
            "debootstrap {} /mnt/targetos {}",
            release,
            config
                .debootstrap_mirror
                .as_deref()
                .unwrap_or("http://archive.ubuntu.com/ubuntu/")
        ),
        format!(
            "debootstrap {} /mnt/targetos {} # fallback if the above fails",
            release, "http://old-releases.ubuntu.com/ubuntu/"
        ),

        // Configure APT Deb822 sources in target
        "mkdir -p /mnt/targetos/etc/apt/sources.list.d".to_string(),
        format!("bash -lc 'cat > /mnt/targetos/etc/apt/sources.list.d/ubuntu.sources <<\'EOF\'\nTypes: deb\nURIs: http://archive.ubuntu.com/ubuntu/\nSuites: {rel}\nComponents: main restricted universe multiverse\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n\nTypes: deb\nURIs: http://security.ubuntu.com/ubuntu\nSuites: {rel}-security\nComponents: main restricted universe multiverse\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\nEOF'", rel=release),
        "rm -f /mnt/targetos/etc/apt/sources.list || true".to_string(),

        // Prepare chroot mounts
        "mount --rbind /dev /mnt/targetos/dev".to_string(),
        "mount --make-private /mnt/targetos/dev".to_string(),
        "mount -t devpts devpts /mnt/targetos/dev/pts || true".to_string(),
        "mount --rbind /proc /mnt/targetos/proc".to_string(),
        "mount --make-private /mnt/targetos/proc".to_string(),
        "mount --rbind /sys /mnt/targetos/sys".to_string(),
        "mount --make-private /mnt/targetos/sys".to_string(),
        "mount --rbind /run /mnt/targetos/run".to_string(),
        "mount --make-private /mnt/targetos/run".to_string(),
        "echo 'nameserver 1.1.1.1' > /mnt/targetos/etc/resolv.conf".to_string(),

        // Add ESP to fstab using UUID
        format!("bash -lc 'ESP_UUID=$(blkid -s UUID -o value {e} 2>/dev/null || true); if [ -n \"$ESP_UUID\" ]; then echo \"UUID=$ESP_UUID /boot/efi vfat umask=0077 0 1\" >> /mnt/targetos/etc/fstab; fi'", e=esp_part),

        // Ensure efivarfs and install core packages
        "chroot /mnt/targetos bash -lc '[ -d /sys/firmware/efi/efivars ] || mkdir -p /sys/firmware/efi/efivars; mountpoint -q /sys/firmware/efi/efivars || mount -t efivarfs efivarfs /sys/firmware/efi/efivars || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'apt update'".to_string(),
        "chroot /mnt/targetos bash -lc 'DEBIAN_FRONTEND=noninteractive apt install -y grub-efi-amd64 grub-efi-amd64-signed linux-image-generic shim-signed zfs-initramfs zfsutils-linux zsys efibootmgr cryptsetup cryptsetup-initramfs dosfstools'".to_string(),
        // Optional cleanups and groups
        "chroot /mnt/targetos bash -lc 'DEBIAN_FRONTEND=noninteractive apt purge -y os-prober || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'addgroup --system lpadmin || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'addgroup --system lxd || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'addgroup --system sambashare || true'".to_string(),

        // Configure crypttab to unlock LUKS at boot via initramfs
        format!("bash -lc 'UUID=$(blkid -s UUID -o value {d}p4 2>/dev/null || true); DEV=\"{d}p4\"; [ -n \"$UUID\" ] && DEV=\"/dev/disk/by-uuid/$UUID\"; echo \"luks $DEV none luks,discard,initramfs\" > /mnt/targetos/etc/crypttab'", d=config.disk_device),
        "chroot /mnt/targetos bash -lc 'update-initramfs -u -k all'".to_string(),

        // ZFS cache seeding and path fix
        "mkdir -p /mnt/targetos/etc/zfs/zfs-list.cache".to_string(),
        "cp -f /etc/zfs/zpool.cache /mnt/targetos/etc/zfs/ 2>/dev/null || true".to_string(),
        "bash -lc 'touch /mnt/targetos/etc/zfs/zfs-list.cache/bpool /mnt/targetos/etc/zfs/zfs-list.cache/rpool'".to_string(),
        "chroot /mnt/targetos bash -lc 'timeout 5 zed -F || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'sed -Ei \"s|/mnt/targetos/?|/|\" /etc/zfs/zfs-list.cache/* || true'".to_string(),
        "chroot /mnt/targetos bash -lc 'update-initramfs -u -k all'".to_string(),

        // GRUB installation with fallbacks
        "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck'".to_string(),
        "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck --no-nvram' # fallback".to_string(),
        "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck --removable' # fallback".to_string(),
        "chroot /mnt/targetos bash -lc 'update-grub'".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config_with_release(release: Option<&str>) -> InstallationConfig {
        InstallationConfig {
            hostname: "test-host".into(),
            disk_device: "/dev/nvme0n1".into(),
            timezone: "UTC".into(),
            luks_key: "key".into(),
            root_password: "root".into(),
            network_interface: "eth0".into(),
            network_address: "192.0.2.10/24".into(),
            network_gateway: "192.0.2.1".into(),
            network_search: "example.test".into(),
            network_nameservers: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            debootstrap_release: release.map(|s| s.to_string()),
            debootstrap_mirror: None,
        }
    }

    #[test]
    fn test_build_next_commands_contains_core_steps_and_order() {
        let cfg = sample_config_with_release(None); // defaults to plucky
        let cmds = build_next_commands_after_storage(&cfg);

        // Presence checks
        assert!(cmds.iter().any(|c| {
            c.starts_with("debootstrap plucky /mnt/targetos http://archive.ubuntu.com/ubuntu/")
        }));
        assert!(cmds
            .iter()
            .any(|c| c.contains("ubuntu.sources") && c.contains("Suites: plucky")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("apt install") && c.contains("dosfstools")));
        assert!(cmds.iter().any(|c| c.contains("apt purge -y os-prober")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("echo \"luks ") && c.contains("none luks,discard,initramfs")));
        assert!(cmds.iter().any(|c| c.contains("grub-install")
            && !c.contains("no-nvram")
            && !c.contains("removable")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("grub-install") && c.contains("--no-nvram")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("grub-install") && c.contains("--removable")));
        assert!(cmds.iter().any(|c| c.contains("update-grub")));

        // Ordering: mounts -> efivars -> apt install -> grub
        let idx_mount_dev = cmds
            .iter()
            .position(|c| c == "mount --rbind /dev /mnt/targetos/dev")
            .unwrap();
        let idx_efivarfs = cmds.iter().position(|c| c.contains("efivarfs")).unwrap();
        let idx_apt_install = cmds
            .iter()
            .position(|c| c.contains("apt install -y"))
            .unwrap();
        let idx_grub = cmds
            .iter()
            .position(|c| {
                c.contains("grub-install") && !c.contains("no-nvram") && !c.contains("removable")
            })
            .unwrap();
        assert!(
            idx_mount_dev < idx_efivarfs,
            "chroot mounts should come before efivarfs"
        );
        assert!(
            idx_efivarfs < idx_apt_install,
            "efivarfs before apt install"
        );
        assert!(idx_apt_install < idx_grub, "apt install before grub");
    }

    #[test]
    fn test_build_next_commands_honors_release_override() {
        let cfg = sample_config_with_release(Some("noble"));
        let cmds = build_next_commands_after_storage(&cfg);
        assert!(cmds.iter().any(|c| {
            c.starts_with("debootstrap noble /mnt/targetos http://archive.ubuntu.com/ubuntu/")
        }));
        assert!(cmds
            .iter()
            .any(|c| c.contains("ubuntu.sources") && c.contains("Suites: noble")));
        assert!(cmds.iter().any(|c| c.contains("Suites: noble-security")));
    }
}
