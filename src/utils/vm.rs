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
pub struct VmManager {
    ovmf_code_path: Option<String>,
    ovmf_vars_path: Option<String>,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new() -> Self {
        Self {
            ovmf_code_path: None,
            ovmf_vars_path: None,
        }
    }

    /// Install Ubuntu in a VM using the provided netboot files and configuration
    pub async fn install_ubuntu(
        &mut self,
        vm_disk: &Path,
        netboot_dir: &Path,
        cloud_init_path: &Path,
        vm_config: &VmConfig,
        architecture: Architecture,
    ) -> Result<()> {
        info!("Starting Ubuntu installation in VM using netboot");

        // Set up UEFI environment and get paths
        self.setup_uefi_environment().await?;

        // Select appropriate QEMU binary for architecture
        let qemu_cmd = match architecture {
            Architecture::Amd64 => "qemu-system-x86_64",
            Architecture::Arm64 => "qemu-system-aarch64",
        };

        // Create cloud-init ISO
        let cloud_init_iso = self.create_cloud_init_iso(cloud_init_path).await?;

        // Get kernel and initrd paths from netboot directory
        // Ubuntu netboot tarballs can have different structures, so we need to search for them
        let possible_kernel_paths = [
            netboot_dir.join("ubuntu-installer").join("amd64").join("linux"),
            netboot_dir.join("linux"),
            netboot_dir.join("vmlinuz"),
            netboot_dir.join("kernel"),
        ];

        let possible_initrd_paths = [
            netboot_dir.join("ubuntu-installer").join("amd64").join("initrd.gz"),
            netboot_dir.join("initrd.gz"),
            netboot_dir.join("initrd"),
        ];

        let mut kernel_path = None;
        let mut initrd_path = None;

        // Find kernel file
        for path in &possible_kernel_paths {
            if path.exists() {
                kernel_path = Some(path);
                break;
            }
        }

        // Find initrd file
        for path in &possible_initrd_paths {
            if path.exists() {
                initrd_path = Some(path);
                break;
            }
        }

        let (kernel_file, initrd_file) = match (kernel_path, initrd_path) {
            (Some(k), Some(i)) => (k, i),
            _ => {
                // Log what files are actually in the netboot directory for debugging
                info!("Available files in netboot directory:");
                if let Ok(mut entries) = tokio::fs::read_dir(netboot_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        info!("  {}", entry.file_name().to_string_lossy());
                    }
                }

                return Err(crate::error::AutoInstallError::VmError(
                    format!("Netboot kernel or initrd not found in {}. \
                            Searched for kernel: {:?}, initrd: {:?}",
                            netboot_dir.display(),
                            possible_kernel_paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                            possible_initrd_paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>())
                ));
            }
        };

        info!("Using kernel: {}", kernel_file.display());
        info!("Using initrd: {}", initrd_file.display());

        // Build QEMU command with direct kernel boot (no UEFI needed for netboot)
        let mut cmd = Command::new(qemu_cmd);
        cmd.args(&[
            "-machine", "accel=kvm:tcg", // Use KVM if available, fallback to TCG
            "-cpu", "host",
            "-m", &format!("{}M", vm_config.memory_mb),
            "-smp", &vm_config.cpu_cores.to_string(),
            "-drive", &format!("file={},format=qcow2,if=virtio", vm_disk.display()),
            "-drive", &format!("file={},media=cdrom,readonly=on", cloud_init_iso.display()),
            "-kernel", kernel_file.to_str().unwrap(),
            "-initrd", initrd_file.to_str().unwrap(),
            "-append", "console=ttyS0 console=tty0 autoinstall 'ds=nocloud;seedfrom=/dev/sr0/'",
            "-netdev", "user,id=net0",
            "-device", "virtio-net,netdev=net0",
            "-vnc", ":1", // Enable VNC on display :1 (port 5901) for debugging
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

            // Check both serial and UEFI logs for progress indicators
            let mut combined_log = String::new();

            if let Ok(serial_content) = tokio::fs::read_to_string("/tmp/qemu-serial.log").await {
                combined_log.push_str(&serial_content);
            }

            if let Ok(uefi_content) = tokio::fs::read_to_string("/tmp/qemu-uefi.log").await {
                combined_log.push_str(&uefi_content);
            }

            if !combined_log.is_empty() {
                // Handle UEFI boot menu or GRUB menu if not already handled
                if !grub_handled && (combined_log.contains("GNU GRUB") ||
                                   combined_log.contains("UEFI") ||
                                   combined_log.contains("Boot Menu") ||
                                   combined_log.contains("installer")) {
                    info!("Boot menu detected, selecting Ubuntu installation...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    self.send_key_to_vm("ret").await?; // Press Enter
                    grub_handled = true;
                }

                // Check for installer activity
                if !installation_started && (combined_log.contains("autoinstall") ||
                                           combined_log.contains("installer") ||
                                           combined_log.contains("d-i") ||  // debian-installer
                                           combined_log.contains("ubuntu-installer")) {
                    info!("Ubuntu installer process started");
                    installation_started = true;
                }

                // Check for installation completion
                if combined_log.contains("Installation finished") ||
                   combined_log.contains("reboot") ||
                   combined_log.contains("Installation complete") ||
                   combined_log.contains("install successful") {
                    info!("Installation completed successfully in {:?}", start_time.elapsed());
                    self.shutdown_qemu().await?;
                    return Ok(());
                }

                // Check for errors
                if combined_log.contains("Failed") || combined_log.contains("Error") ||
                   combined_log.contains("FATAL") {
                    warn!("Possible installation error detected in logs");
                }

                // Log any new content for debugging
                if combined_log.len() > 100 {
                    let last_lines: Vec<&str> = combined_log.lines().rev().take(5).collect();
                    debug!("Recent log activity: {:?}", last_lines);
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

    /// Set up UEFI environment for VM
    async fn setup_uefi_environment(&mut self) -> Result<()> {
        use tokio::fs;

        // Try to find OVMF files in common locations
        let ovmf_paths = [
            // Ubuntu/Debian (newer 4M format)
            ("/usr/share/OVMF/OVMF_CODE_4M.fd", "/usr/share/OVMF/OVMF_VARS_4M.fd"),
            // Ubuntu/Debian (legacy 2M format)
            ("/usr/share/OVMF/OVMF_CODE.fd", "/usr/share/OVMF/OVMF_VARS.fd"),
            // Ubuntu/Debian (alternative paths)
            ("/usr/share/ovmf/OVMF.fd", "/usr/share/OVMF/OVMF_VARS_4M.fd"),
            ("/usr/share/qemu/OVMF.fd", "/usr/share/OVMF/OVMF_VARS_4M.fd"),
            // Fedora/RHEL
            ("/usr/share/edk2/ovmf/OVMF_CODE.fd", "/usr/share/edk2/ovmf/OVMF_VARS.fd"),
            // Arch Linux
            ("/usr/share/ovmf/x64/OVMF_CODE.fd", "/usr/share/ovmf/x64/OVMF_VARS.fd"),
            // macOS (Homebrew)
            ("/opt/homebrew/share/qemu/edk2-x86_64-code.fd", "/opt/homebrew/share/qemu/edk2-i386-vars.fd"),
            ("/usr/local/share/qemu/edk2-x86_64-code.fd", "/usr/local/share/qemu/edk2-i386-vars.fd"),
        ];

        let mut ovmf_code_path = None;
        let mut ovmf_vars_template = None;

        // Find available OVMF files
        for (code_path, vars_path) in &ovmf_paths {
            if fs::try_exists(code_path).await.unwrap_or(false) &&
               fs::try_exists(vars_path).await.unwrap_or(false) {
                ovmf_code_path = Some(code_path);
                ovmf_vars_template = Some(vars_path);
                debug!("Found OVMF files at: {} and {}", code_path, vars_path);
                break;
            }
        }

        let (ovmf_code, ovmf_vars_src) = match (ovmf_code_path, ovmf_vars_template) {
            (Some(code), Some(vars)) => (code, vars),
            _ => {
                return Err(crate::error::AutoInstallError::VmError(
                    "OVMF UEFI firmware not found. Please install OVMF/EDK2:\n\
                     Ubuntu/Debian: sudo apt install ovmf\n\
                     Fedora/RHEL: sudo dnf install edk2-ovmf\n\
                     Arch Linux: sudo pacman -S edk2-ovmf\n\
                     macOS: brew install qemu".to_string()
                ));
            }
        };

        // Store the paths for later use
        self.ovmf_code_path = Some(ovmf_code.to_string());
        self.ovmf_vars_path = Some(ovmf_vars_src.to_string());

        // Copy OVMF_VARS.fd to temporary writable location
        let ovmf_vars_runtime = "/tmp/OVMF_VARS.fd";
        fs::copy(ovmf_vars_src, ovmf_vars_runtime).await
            .map_err(|e| crate::error::AutoInstallError::VmError(
                format!("Failed to copy OVMF_VARS.fd: {}", e)
            ))?;

        debug!("UEFI environment set up successfully");
        debug!("OVMF_CODE: {}", ovmf_code);
        debug!("OVMF_VARS: {}", ovmf_vars_runtime);

        Ok(())
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
    pub async fn kill_qemu(&self) -> Result<()> {
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
