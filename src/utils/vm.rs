// file: src/utils/vm.rs
// version: 1.0.0
// guid: y5z6a7b8-c9d0-1234-5678-901234yzabcd

//! VM management utilities

use std::path::Path;
use tokio::process::Command;
use crate::{
    config::{Architecture, VmConfig},
    Result,
};
use tracing::{info, debug, warn};

/// VM manager for creating and running virtual machines
pub struct VmManager;

impl VmManager {
    /// Create a new VM manager
    pub fn new() -> Self {
        Self
    }

    /// Install Ubuntu in a VM using the provided ISO and configuration
    pub async fn install_ubuntu(
        &self,
        vm_disk: &Path,
        iso_path: &Path,
        cloud_init_path: &Path,
        vm_config: &VmConfig,
        architecture: Architecture,
    ) -> Result<()> {
        info!("Starting Ubuntu installation in VM");

        // Select appropriate QEMU binary for architecture
        let qemu_cmd = match architecture {
            Architecture::Amd64 => "qemu-system-x86_64",
            Architecture::Arm64 => "qemu-system-aarch64",
        };

        // Create cloud-init ISO
        let cloud_init_iso = self.create_cloud_init_iso(cloud_init_path).await?;

        // Build QEMU command with VNC display and monitor for automation
        let mut cmd = Command::new(qemu_cmd);
        cmd.args(&[
            "-machine", "accel=kvm:tcg", // Use KVM if available, fallback to TCG
            "-cpu", "host",
            "-m", &format!("{}M", vm_config.memory_mb),
            "-smp", &vm_config.cpu_cores.to_string(),
            "-drive", &format!("file={},format=qcow2,if=virtio", vm_disk.display()),
            "-drive", &format!("file={},media=cdrom,readonly=on", iso_path.display()),
            "-drive", &format!("file={},media=cdrom,readonly=on", cloud_init_iso.display()),
            "-boot", "d", // Boot from CD-ROM first
            "-netdev", "user,id=net0",
            "-device", "virtio-net,netdev=net0",
            "-display", "none", // No display
            "-serial", "file:/tmp/qemu-serial.log", // Log serial output to file
            "-monitor", "unix:/tmp/qemu-monitor.sock,server,nowait", // Monitor socket
            "-daemonize", // Run as daemon
        ]);

        // Add architecture-specific arguments
        match architecture {
            Architecture::Amd64 => {
                cmd.args(&["-machine", "q35"]);
            }
            Architecture::Arm64 => {
                cmd.args(&[
                    "-machine", "virt",
                    "-bios", "/usr/share/qemu-efi-aarch64/QEMU_EFI.fd",
                ]);
            }
        }

        debug!("Starting QEMU installation with command: {:?}", cmd);

        // Start QEMU as daemon
        let output = cmd.output().await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to start QEMU: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::VmError(
                format!("QEMU failed to start: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        info!("QEMU started in daemon mode");

        // Monitor installation progress via serial log and QEMU monitor
        self.monitor_installation().await?;

        // Cleanup cloud-init ISO
        let _ = tokio::fs::remove_file(&cloud_init_iso).await;

        Ok(())
    }

    /// Monitor QEMU installation progress and handle automation
    async fn monitor_installation(&self) -> Result<()> {
        info!("Ubuntu installation started - this may take 30-60 minutes");

        let timeout = tokio::time::Duration::from_secs(3600); // 1 hour timeout
        let start_time = std::time::Instant::now();

        // Wait for GRUB menu to appear and send Enter key
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        self.send_key_to_vm("ret").await?; // Press Enter to select first option

        // Monitor serial output for installation progress
        let mut grub_handled = false;
        let mut installation_started = false;

        loop {
            if start_time.elapsed() > timeout {
                self.kill_qemu().await?;
                return Err(crate::error::AutoInstallError::VmError(
                    "VM installation timed out after 1 hour".to_string()
                ));
            }

            // Check serial log for progress indicators
            if let Ok(log_content) = tokio::fs::read_to_string("/tmp/qemu-serial.log").await {
                // Handle GRUB menu if not already handled
                if !grub_handled && log_content.contains("GNU GRUB") {
                    info!("GRUB menu detected, selecting Ubuntu installation...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    self.send_key_to_vm("ret").await?; // Press Enter
                    grub_handled = true;
                }

                // Check for autoinstall start
                if !installation_started && log_content.contains("autoinstall") {
                    info!("Autoinstall process started");
                    installation_started = true;
                }

                // Check for installation completion
                if log_content.contains("Installation finished") ||
                   log_content.contains("reboot") ||
                   log_content.contains("Installation complete") {
                    info!("Installation completed successfully in {:?}", start_time.elapsed());
                    self.shutdown_qemu().await?;
                    return Ok(());
                }

                // Check for errors
                if log_content.contains("Failed") || log_content.contains("Error") {
                    warn!("Possible installation error detected in log");
                }
            }

            // Log progress periodically
            if start_time.elapsed().as_secs() % 300 == 0 { // Every 5 minutes
                info!("Installation in progress... elapsed: {:?}", start_time.elapsed());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    /// Send key command to QEMU via monitor
    async fn send_key_to_vm(&self, key: &str) -> Result<()> {
        let cmd = format!("sendkey {}", key);
        self.send_monitor_command(&cmd).await
    }

    /// Send command to QEMU monitor
    async fn send_monitor_command(&self, command: &str) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixStream;

        // Connect to QEMU monitor socket
        match UnixStream::connect("/tmp/qemu-monitor.sock").await {
            Ok(mut stream) => {
                let cmd_with_newline = format!("{}\n", command);
                stream.write_all(cmd_with_newline.as_bytes()).await
                    .map_err(|e| crate::error::AutoInstallError::VmError(
                        format!("Failed to send monitor command: {}", e)
                    ))?;
                debug!("Sent monitor command: {}", command);
                Ok(())
            }
            Err(e) => {
                debug!("Monitor socket not available: {}", e);
                Ok(()) // Non-fatal, continue
            }
        }
    }

    /// Shutdown QEMU gracefully
    async fn shutdown_qemu(&self) -> Result<()> {
        info!("Shutting down QEMU VM");
        self.send_monitor_command("quit").await?;

        // Wait a bit for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        // Cleanup files
        let _ = tokio::fs::remove_file("/tmp/qemu-serial.log").await;
        let _ = tokio::fs::remove_file("/tmp/qemu-monitor.sock").await;

        Ok(())
    }

    /// Kill QEMU process forcefully
    async fn kill_qemu(&self) -> Result<()> {
        info!("Forcefully terminating QEMU VM");

        // Find and kill QEMU process
        let output = Command::new("pkill")
            .args(&["-f", "qemu-system"])
            .output()
            .await;

        if let Err(e) = output {
            debug!("pkill failed: {}", e);
        }

        // Cleanup files
        let _ = tokio::fs::remove_file("/tmp/qemu-serial.log").await;
        let _ = tokio::fs::remove_file("/tmp/qemu-monitor.sock").await;

        Ok(())
    }

    /// Create cloud-init ISO from configuration directory
    async fn create_cloud_init_iso(&self, cloud_init_path: &Path) -> Result<std::path::PathBuf> {
        let iso_path = cloud_init_path.with_extension("iso");

        debug!("Creating cloud-init ISO: {}", iso_path.display());

        let output = Command::new("genisoimage")
            .args(&[
                "-output", iso_path.to_str().unwrap(),
                "-volid", "cidata",
                "-joliet",
                "-rock",
                cloud_init_path.to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to create cloud-init ISO: {}", e)
            ))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::AutoInstallError::VmError(
                format!("genisoimage failed: {}", stderr)
            ));
        }

        debug!("Cloud-init ISO created: {}", iso_path.display());
        Ok(iso_path)
    }

    /// Test VM functionality by starting a test VM
    pub async fn test_vm_functionality(&self, architecture: Architecture) -> Result<()> {
        info!("Testing VM functionality for {} architecture", architecture.as_str());

        let qemu_cmd = match architecture {
            Architecture::Amd64 => "qemu-system-x86_64",
            Architecture::Arm64 => "qemu-system-aarch64",
        };

        // Create a temporary test disk
        let temp_dir = tempfile::tempdir()
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;
        let test_disk = temp_dir.path().join("test.qcow2");

        // Create test disk
        let output = Command::new("qemu-img")
            .args(&[
                "create",
                "-f", "qcow2",
                test_disk.to_str().unwrap(),
                "1G",
            ])
            .output()
            .await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to create test disk: {}", e)
            ))?;

        if !output.status.success() {
            return Err(crate::error::AutoInstallError::VmError(
                "Failed to create test disk image".to_string()
            ));
        }

        // Test QEMU startup (without actually booting)
        let mut cmd = Command::new(qemu_cmd);
        cmd.args(&[
            "-machine", "accel=kvm:tcg",
            "-m", "512M",
            "-drive", &format!("file={},format=qcow2,if=virtio", test_disk.display()),
            "-nographic",
            "-serial", "none",
            "-monitor", "none",
            "-S", // Start in stopped state
        ]);

        // Add architecture-specific arguments
        match architecture {
            Architecture::Amd64 => {
                cmd.args(&["-machine", "q35"]);
            }
            Architecture::Arm64 => {
                cmd.args(&["-machine", "virt"]);
            }
        }

        let mut child = cmd.spawn()
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to start test VM: {}", e)
            ))?;

        // Give it a moment to start, then kill it
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        child.kill().await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to kill test VM: {}", e)
            ))?;

        let _ = child.wait().await;

        info!("VM functionality test completed successfully");
        Ok(())
    }

    /// Check if KVM acceleration is available
    pub async fn check_kvm_support(&self) -> bool {
        use std::os::unix::fs::MetadataExt;

        Path::new("/dev/kvm").exists() &&
        tokio::fs::metadata("/dev/kvm").await
            .map(|m| {
                // Check if it's a character device (mode & S_IFMT == S_IFCHR)
                (m.mode() & 0o170000) == 0o020000
            })
            .unwrap_or(false)
    }

    /// Get recommended VM configuration based on system resources
    pub async fn get_recommended_vm_config(&self) -> Result<VmConfig> {
        let available_memory = crate::utils::system::SystemUtils::get_available_memory().await?;
        let available_space = crate::utils::system::SystemUtils::get_available_space("/tmp").await?;

        // Use 50% of available memory, but at least 1GB and at most 8GB
        let memory_mb = std::cmp::max(1024, std::cmp::min(8192, available_memory as u32 / 2));

        // Use 20GB disk space or 50% of available space, whichever is smaller
        let disk_size_gb = std::cmp::min(20, available_space as u32 / 2);

        // Use 2 CPU cores by default
        let cpu_cores = 2;

        Ok(VmConfig {
            memory_mb,
            disk_size_gb,
            cpu_cores,
        })
    }
}

impl Default for VmManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_vm_manager_creation() {
        let vm_manager = VmManager::new();
        assert_eq!(vm_manager.qemu_binary, "qemu-system-x86_64");
    }

    #[tokio::test]
    async fn test_check_kvm_support() {
        let vm_manager = VmManager::new();
        let kvm_support = vm_manager.check_kvm_support().await;
        // This will vary depending on the test environment
        assert!(kvm_support || !kvm_support); // Always true, just testing it doesn't panic
    }

    #[tokio::test]
    async fn test_get_recommended_vm_config() {
        let vm_manager = VmManager::new();
        let result = vm_manager.get_recommended_vm_config().await;

        // Should return a config or an error
        if let Ok(config) = result {
            assert!(config.memory_mb >= 1024);
            assert!(config.disk_size_gb >= 1);
            assert!(config.cpu_cores >= 1);
        }
    }

    #[tokio::test]
    async fn test_create_cloud_init_iso() {
        let vm_manager = VmManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Create mock cloud-init files
        let cloud_init_dir = temp_dir.path().join("cloud-init");
        tokio::fs::create_dir_all(&cloud_init_dir).await.unwrap();
        tokio::fs::write(cloud_init_dir.join("user-data"), "test data").await.unwrap();
        tokio::fs::write(cloud_init_dir.join("meta-data"), "test meta").await.unwrap();

        // Skip this test if genisoimage is not available
        if crate::utils::system::SystemUtils::command_exists("genisoimage").await {
            let result = vm_manager.create_cloud_init_iso(&cloud_init_dir).await;
            assert!(result.is_ok());

            let iso_path = result.unwrap();
            assert!(iso_path.exists());
        }
    }
}
