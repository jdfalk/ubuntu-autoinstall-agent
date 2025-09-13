// file: src/network/ssh_installer/system_setup.rs
// version: 1.13.0
// guid: sshsys01-2345-6789-abcd-ef0123456789

//! System setup and configuration for SSH installation

use tracing::info;
use crate::network::SshClient;
use crate::Result;
use super::config::InstallationConfig;

pub struct SystemConfigurator<'a> {
    ssh: &'a mut SshClient,
}

impl<'a> SystemConfigurator<'a> {
    pub fn new(ssh: &'a mut SshClient) -> Self {
        Self { ssh }
    }

    /// Build the command used to detect the ESP partition by GUID
    fn build_esp_detection_command(guid: &str) -> String {
        // Use lsblk key=value format (-P) and grep/sed to extract PATH for the matching PARTTYPE.
        // Safe quoting: outer bash uses double quotes; sed uses single quotes containing double quotes.
    format!(r##"bash -lc "lsblk -rP -o PATH,PARTTYPE | grep -i 'PARTTYPE="{0}"' | head -n1 | sed -n 's/.*PATH="\([^" ]*\)".*/\1/p'""##, guid)
    }

    /// Decide which ESP partition path to use based on detection output
    fn choose_esp_partition(detected_output: &str, default_disk: &str) -> String {
        let part = detected_output.trim();
        if part.is_empty() {
            format!("{}p1", default_disk)
        } else {
            part.to_string()
        }
    }

    /// Detect the ESP partition path by GUID PARTTYPE; fallback to `${DISK}p1` if not found
    async fn detect_esp_partition_path(&mut self, default_disk: &str) -> Result<String> {
        // EFI System Partition type GUID
        let guid = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
        let cmd = Self::build_esp_detection_command(guid);
        let out = self.ssh.execute_with_output(&cmd).await.unwrap_or_default();
        Ok(Self::choose_esp_partition(&out, default_disk))
    }

    /// Install base system using debootstrap
    pub async fn install_base_system(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Installing base system");

    // Mount ESP partition (auto-detect by PARTTYPE GUID, fallback to ${DISK}p1)
    self.log_and_execute("Creating ESP mount point", "mkdir -p /mnt/targetos/boot/efi").await?;
    let esp_part = self.detect_esp_partition_path(&config.disk_device).await?;
    self.log_and_execute("Mounting ESP", &format!("mount {} /mnt/targetos/boot/efi", esp_part)).await?;

        // Install base system using debootstrap (codename/mirror configurable)
        let release = config.debootstrap_release.as_deref().unwrap_or("plucky");
        let mirror = config.debootstrap_mirror.as_deref().unwrap_or("http://archive.ubuntu.com/ubuntu/");
        let primary_cmd = format!("debootstrap {} /mnt/targetos {}", release, mirror);
        if let Err(_e) = self.log_and_execute("Running debootstrap", &primary_cmd).await {
            // Fallback to old-releases if not already using it
            let fallback_mirror = "http://old-releases.ubuntu.com/ubuntu/";
            if mirror != fallback_mirror {
                let fallback_cmd = format!("debootstrap {} /mnt/targetos {}", release, fallback_mirror);
                self.log_and_execute("Running debootstrap (fallback old-releases)", &fallback_cmd).await?;
            } else {
                // Re-raise the original error
                return Err(_e);
            }
        }

        // Setup basic system files
        self.setup_basic_system_files(config).await?;

        // Configure system in chroot
        self.configure_system_in_chroot(config).await?;

        info!("Base system installation completed");
        Ok(())
    }

    /// Setup basic system files
    async fn setup_basic_system_files(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up basic system files");

        // Hostname
        self.ssh.execute(&format!("echo '{}' > /mnt/targetos/etc/hostname", config.hostname)).await?;

        // Hosts file
        let hosts_content = format!(
            "127.0.0.1 localhost\n127.0.1.1 {}\n::1 localhost ip6-localhost ip6-loopback\nff02::1 ip6-allnodes\nff02::2 ip6-allrouters",
            config.hostname
        );
        self.ssh.execute(&format!("cat > /mnt/targetos/etc/hosts << 'EOF'\n{}\nEOF", hosts_content)).await?;

        // Network configuration
        self.setup_network_configuration(config).await?;

        // Timezone
        self.ssh.execute(&format!("ln -sf /usr/share/zoneinfo/{} /mnt/targetos/etc/localtime", config.timezone)).await?;

        Ok(())
    }

    /// Setup network configuration
    async fn setup_network_configuration(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up network configuration");

        let netplan_config = format!(
            r#"network:
  version: 2
  renderer: networkd
  ethernets:
    {}:
      addresses:
        - {}
      routes:
        - to: default
          via: {}
      nameservers:
        search:
          - {}
        addresses:
{}"#,
            config.network_interface,
            config.network_address,
            config.network_gateway,
            config.network_search,
            config.network_nameservers.iter()
                .map(|ns| format!("          - {}", ns))
                .collect::<Vec<_>>()
                .join("\n")
        );

        self.ssh.execute(&format!(
            "cat > /mnt/targetos/etc/netplan/01-netcfg.yaml << 'EOF'\n{}\nEOF",
            netplan_config
        )).await?;

        Ok(())
    }

    /// Configure system in chroot environment
    async fn configure_system_in_chroot(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Configuring system in chroot");

        // Prepare chroot (align with OpenZFS Ubuntu root-on-ZFS guidance)
        // Use rbind + make-rslave so nested mounts propagate correctly
        let _ = self.log_and_execute(
            "Bind /dev (rbind)",
            "[ -d /mnt/targetos/dev ] || mkdir -p /mnt/targetos/dev; mountpoint -q /mnt/targetos/dev || mount --rbind /dev /mnt/targetos/dev"
        ).await;
        let _ = self.log_and_execute(
            "Make /dev private",
            "mount --make-private /mnt/targetos/dev || true"
        ).await;
        // Ensure devpts exists (rbind should cover it, but this is a safe fallback)
        let _ = self.log_and_execute(
            "Ensuring /dev/pts",
            "[ -d /mnt/targetos/dev/pts ] || mkdir -p /mnt/targetos/dev/pts; mountpoint -q /mnt/targetos/dev/pts || mount -t devpts devpts /mnt/targetos/dev/pts || true"
        ).await;
        let _ = self.log_and_execute(
            "Bind /proc (rbind)",
            "[ -d /mnt/targetos/proc ] || mkdir -p /mnt/targetos/proc; mountpoint -q /mnt/targetos/proc || mount --rbind /proc /mnt/targetos/proc"
        ).await;
        let _ = self.log_and_execute(
            "Make /proc private",
            "mount --make-private /mnt/targetos/proc || true"
        ).await;
        let _ = self.log_and_execute(
            "Bind /sys (rbind)",
            "[ -d /mnt/targetos/sys ] || mkdir -p /mnt/targetos/sys; mountpoint -q /mnt/targetos/sys || mount --rbind /sys /mnt/targetos/sys"
        ).await;
        let _ = self.log_and_execute(
            "Make /sys private",
            "mount --make-private /mnt/targetos/sys || true"
        ).await;
        let _ = self.log_and_execute(
            "Bind /run (rbind)",
            "[ -d /mnt/targetos/run ] || mkdir -p /mnt/targetos/run; mountpoint -q /mnt/targetos/run || mount --rbind /run /mnt/targetos/run"
        ).await;
        let _ = self.log_and_execute(
            "Make /run private",
            "mount --make-private /mnt/targetos/run || true"
        ).await;

        // Fix DNS inside chroot: resolv.conf is often a broken symlink in a chroot
        // Remove it and write a simple resolv.conf with public DNS to ensure apt can resolve
        let _ = self.log_and_execute(
            "Reset chroot resolv.conf",
            "[ -e /mnt/targetos/etc/resolv.conf ] && rm -f /mnt/targetos/etc/resolv.conf; echo 'nameserver 1.1.1.1' > /mnt/targetos/etc/resolv.conf"
        ).await;

        // Ensure ESP is mounted before installing EFI-related packages so postinst scripts can run correctly
        let _ = self.log_and_execute(
            "Ensure ESP mountpoint",
            "[ -d /mnt/targetos/boot/efi ] || mkdir -p /mnt/targetos/boot/efi"
        ).await;
        let esp_part = self.detect_esp_partition_path(&config.disk_device).await?;
        let _ = self.log_and_execute(
            "Mount ESP if not mounted",
            &format!("mountpoint -q /mnt/targetos/boot/efi || mount {} /mnt/targetos/boot/efi || true", esp_part)
        ).await;

        // Ensure /etc/fstab has a persistent entry for the ESP (UUID based)
        let esp_part = self.detect_esp_partition_path(&config.disk_device).await?;
        let esp_uuid_out = self.ssh.execute_with_output(&format!("blkid -s UUID -o value {} 2>/dev/null || true", esp_part)).await?;
        let esp_uuid = esp_uuid_out.trim();
        if !esp_uuid.is_empty() {
            let fstab_line = format!("UUID={} /boot/efi vfat umask=0077 0 1", esp_uuid);
            // Single-quote the bash -lc argument; use double quotes inside for grep pattern and echo payload
            let cmd = format!(
                r#"bash -lc 'grep -q "^UUID=.* /boot/efi " /mnt/targetos/etc/fstab 2>/dev/null || echo "{0}" >> /mnt/targetos/etc/fstab'"#,
                fstab_line
            );
            let _ = self.ssh.execute(&cmd).await;
        }

        // Ensure efivarfs is available in chroot prior to EFI package installation (some postinst may touch NVRAM)
        let _ = self.log_and_execute(
            "Ensure efivarfs in chroot",
            "chroot /mnt/targetos bash -lc '[ -d /sys/firmware/efi/efivars ] || mkdir -p /sys/firmware/efi/efivars; mountpoint -q /sys/firmware/efi/efivars || mount -t efivarfs efivarfs /sys/firmware/efi/efivars || true'"
        ).await;

        // Install essential packages
        let chroot_commands = vec![
            "apt update",
            // Core UEFI + ZFS packages
            "DEBIAN_FRONTEND=noninteractive apt install -y grub-efi-amd64 grub-efi-amd64-signed linux-image-generic shim-signed zfs-initramfs zfsutils-linux zsys efibootmgr cryptsetup cryptsetup-initramfs",
            // Helpful tooling
            "DEBIAN_FRONTEND=noninteractive apt install -y linux-headers-generic",
            "DEBIAN_FRONTEND=noninteractive apt install -y openssh-server vim htop curl",
        ];

        for cmd in chroot_commands {
            let _ = self.log_and_execute(&format!("Chroot: {}", cmd), &format!("chroot /mnt/targetos bash -lc '{}'", cmd)).await;
        }

        // Generate /etc/hostid to aid ZFS import on boot (prefer zgenhostid, fallback to hostid)
        let _ = self.log_and_execute(
            "Generate /etc/hostid",
            "chroot /mnt/targetos bash -lc 'command -v zgenhostid >/dev/null 2>&1 && zgenhostid -f /etc/hostid || (command -v hostid >/dev/null 2>&1 && hostid > /etc/hostid) || true'"
        ).await;

        // Set root password
        let _ = self.log_and_execute(
            "Setting root password",
            &format!("chroot /mnt/targetos bash -lc \"echo 'root:{}' | chpasswd\"", config.root_password)
        ).await;

    // Enable SSH (ignore failure if systemd not fully present yet)
    let _ = self.log_and_execute("Enabling SSH", "chroot /mnt/targetos bash -lc 'systemctl enable ssh'").await;

        Ok(())
    }

    /// Configure ZFS in chroot
    pub async fn configure_zfs_in_chroot(&mut self) -> Result<()> {
        info!("Configuring ZFS in chroot");

        // Enable ZFS services
        let zfs_commands = vec![
            "systemctl enable zfs-import-cache",
            "systemctl enable zfs-mount",
            "systemctl enable zfs-import.target",
            "update-initramfs -u -k all",
        ];

        for cmd in zfs_commands {
            // Best-effort: some services may not exist until packages are installed
            let _ = self.log_and_execute(&format!("ZFS: {}", cmd), &format!("chroot /mnt/targetos bash -lc '{}'", cmd)).await;
        }

        Ok(())
    }

    /// Configure GRUB in chroot
    pub async fn configure_grub_in_chroot(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Configuring GRUB in chroot");

        // Ensure ESP is mounted inside the target (some environments unmount it between phases)
        let _ = self.log_and_execute(
            "Ensure ESP mountpoint",
            "[ -d /mnt/targetos/boot/efi ] || mkdir -p /mnt/targetos/boot/efi"
        ).await;
        let esp_part = self.detect_esp_partition_path(&config.disk_device).await?;
        let _ = self.log_and_execute(
            "Mount ESP if not mounted",
            &format!("mountpoint -q /mnt/targetos/boot/efi || mount {} /mnt/targetos/boot/efi || true", esp_part)
        ).await;

        // Ensure efivarfs is mounted inside chroot (some environments need this for NVRAM writes)
        let _ = self.log_and_execute(
            "Ensure efivarfs",
            "chroot /mnt/targetos bash -lc '[ -d /sys/firmware/efi/efivars ] || mkdir -p /sys/firmware/efi/efivars; mountpoint -q /sys/firmware/efi/efivars || mount -t efivarfs efivarfs /sys/firmware/efi/efivars || true'"
        ).await;

        // Update GRUB configuration - try normal path first, then --no-nvram, then --removable as last resort
        if let Err(_e) = self.log_and_execute(
            "Installing GRUB to ESP",
            "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck'"
        ).await {
            // Fallback for systems that cannot write NVRAM (headless, buggy firmware, or efivars access issues)
            if let Err(_e2) = self.log_and_execute(
                "Installing GRUB to ESP (no-nvram fallback)",
                "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck --no-nvram'"
            ).await {
                // Final fallback: install as removable media bootloader; many UEFI firmwares will pick this up
                self.log_and_execute(
                    "Installing GRUB to ESP (removable fallback)",
                    "chroot /mnt/targetos bash -lc 'grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck --removable'"
                ).await?;
            }
        }

        self.log_and_execute("Updating GRUB config", "chroot /mnt/targetos bash -lc 'update-grub'").await?;

        Ok(())
    }

    /// Configure LUKS key handling in chroot
    pub async fn setup_luks_key_in_chroot(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up LUKS key in chroot");

        // Create keyfile in target system
        self.log_and_execute("Creating LUKS keyfile",
            &format!("echo '{}' > /mnt/targetos/etc/luks.key", config.luks_key)).await?;
        self.log_and_execute("Setting keyfile permissions", "chmod 600 /mnt/targetos/etc/luks.key").await?;

        // Discover partition UUID and write crypttab using UUID with recommended options
        let part = format!("{}p4", config.disk_device);
        let uuid_out = self.ssh.execute_with_output(&format!("blkid -s UUID -o value {} 2>/dev/null || true", part)).await?;
        let uuid = uuid_out.trim();
        let crypt_device = if uuid.is_empty() { part.clone() } else { format!("UUID={}", uuid) };
        let crypttab_entry = format!("luks {} /etc/luks.key luks,discard", crypt_device);
        let _ = self.ssh.execute(&format!("[ -d /mnt/targetos/etc ] || mkdir -p /mnt/targetos/etc; echo '{}' > /mnt/targetos/etc/crypttab", crypttab_entry)).await;

        // Update initramfs to include cryptroot and keyfile (post-crypttab)
        let _ = self.log_and_execute("Updating initramfs (post-crypttab)", "chroot /mnt/targetos bash -lc 'update-initramfs -u -k all'").await;

        Ok(())
    }

    /// Final cleanup and unmounting
    pub async fn final_cleanup(&mut self, _config: &InstallationConfig) -> Result<()> {
        info!("Performing final cleanup");

        // Unmount chroot bindings (recursive for rbind mounts)
        // Make unmounts idempotent
        self.log_and_execute("Unmounting /sys (recursive)", "umount -R /mnt/targetos/sys || true").await?;
        self.log_and_execute("Unmounting /proc (recursive)", "umount -R /mnt/targetos/proc || true").await?;
        self.log_and_execute("Unmounting /dev (recursive)", "umount -R /mnt/targetos/dev || true").await?;
        self.log_and_execute("Unmounting /run (recursive)", "umount -R /mnt/targetos/run || true").await?;

        // Unmount filesystems
    self.log_and_execute("Unmounting ESP", "umount /mnt/targetos/boot/efi || true").await?;

        // Export ZFS pools
    self.log_and_execute("Exporting bpool", "zpool export bpool || true").await?;
    self.log_and_execute("Exporting rpool", "zpool export rpool || true").await?;

        // Unmount and close LUKS if present
        let _ = self.log_and_execute("Unmounting /mnt/luks if mounted", "mountpoint -q /mnt/luks && umount -lf /mnt/luks || true").await;
        let _ = self.log_and_execute("Closing LUKS mapper if open", "cryptsetup status luks >/dev/null 2>&1 && cryptsetup close luks || true").await;

        info!("Final cleanup completed");
        Ok(())
    }

    /// Helper method to log and execute commands
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);
        self.ssh.execute(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_esp_detection_command_contains_expected_parts() {
        let guid = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
        let cmd = SystemConfigurator::build_esp_detection_command(guid);
        // Basic sanity of structure
        assert!(cmd.starts_with("bash -lc \""), "command should start with bash -lc");
        assert!(cmd.contains("lsblk -rP -o PATH,PARTTYPE"));
        assert!(cmd.contains("grep -i 'PARTTYPE=\""));
        assert!(cmd.contains(guid));
        assert!(cmd.contains("sed -n 's/.*PATH=\""));
        assert!(cmd.ends_with("\""), "command should end with closing quote");
    }

    #[test]
    fn test_choose_esp_partition_uses_detected_when_present() {
        let detected = "/dev/nvme0n1p1\n"; // with trailing newline
        let chosen = SystemConfigurator::choose_esp_partition(detected, "/dev/nvme0n1");
        assert_eq!(chosen, "/dev/nvme0n1p1");
    }

    #[test]
    fn test_choose_esp_partition_falls_back_when_empty() {
        let detected = "  \n\t"; // whitespace only
        let chosen = SystemConfigurator::choose_esp_partition(detected, "/dev/sda");
        assert_eq!(chosen, "/dev/sdap1");
    }
}
