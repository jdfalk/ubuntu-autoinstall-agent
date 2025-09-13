// file: src/network/ssh_installer/system_setup.rs
// version: 1.7.0
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

    /// Install base system using debootstrap
    pub async fn install_base_system(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Installing base system");

        // Mount ESP partition
        self.log_and_execute("Creating ESP mount point", "mkdir -p /mnt/targetos/boot/efi").await?;
        self.log_and_execute("Mounting ESP", &format!("mount {}p1 /mnt/targetos/boot/efi", config.disk_device)).await?;

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
            "Binding /dev (rbind)",
            "[ -d /mnt/targetos/dev ] || mkdir -p /mnt/targetos/dev; mountpoint -q /mnt/targetos/dev || mount --rbind /dev /mnt/targetos/dev; mount --make-rslave /mnt/targetos/dev"
        ).await;
        // Ensure devpts exists (rbind should cover it, but this is a safe fallback)
        let _ = self.log_and_execute(
            "Ensuring /dev/pts",
            "[ -d /mnt/targetos/dev/pts ] || mkdir -p /mnt/targetos/dev/pts; mountpoint -q /mnt/targetos/dev/pts || mount -t devpts devpts /mnt/targetos/dev/pts || true"
        ).await;
        let _ = self.log_and_execute(
            "Binding /proc (rbind)",
            "[ -d /mnt/targetos/proc ] || mkdir -p /mnt/targetos/proc; mountpoint -q /mnt/targetos/proc || mount --rbind /proc /mnt/targetos/proc; mount --make-rslave /mnt/targetos/proc"
        ).await;
        let _ = self.log_and_execute(
            "Binding /sys (rbind)",
            "[ -d /mnt/targetos/sys ] || mkdir -p /mnt/targetos/sys; mountpoint -q /mnt/targetos/sys || mount --rbind /sys /mnt/targetos/sys; mount --make-rslave /mnt/targetos/sys"
        ).await;
        let _ = self.log_and_execute(
            "Binding /run (rbind)",
            "[ -d /mnt/targetos/run ] || mkdir -p /mnt/targetos/run; mountpoint -q /mnt/targetos/run || mount --rbind /run /mnt/targetos/run; mount --make-rslave /mnt/targetos/run"
        ).await;

        // Fix DNS inside chroot: resolv.conf is often a broken symlink in a chroot
        // Remove it and write a simple resolv.conf with public DNS to ensure apt can resolve
        let _ = self.log_and_execute(
            "Reset chroot resolv.conf",
            "[ -e /mnt/targetos/etc/resolv.conf ] && rm -f /mnt/targetos/etc/resolv.conf; echo 'nameserver 1.1.1.1' > /mnt/targetos/etc/resolv.conf"
        ).await;

        // Install essential packages
        let chroot_commands = vec![
            "apt update",
            // Core UEFI + ZFS packages
            "DEBIAN_FRONTEND=noninteractive apt install -y grub-efi-amd64 grub-efi-amd64-signed linux-image-generic shim-signed zfs-initramfs zfsutils-linux zsys efibootmgr",
            // Helpful tooling
            "DEBIAN_FRONTEND=noninteractive apt install -y linux-headers-generic",
            "DEBIAN_FRONTEND=noninteractive apt install -y openssh-server vim htop curl",
        ];

        for cmd in chroot_commands {
            let _ = self.log_and_execute(&format!("Chroot: {}", cmd), &format!("chroot /mnt/targetos bash -lc '{}'", cmd)).await;
        }

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
        let _ = self.log_and_execute(
            "Mount ESP if not mounted",
            &format!("mountpoint -q /mnt/targetos/boot/efi || mount {}p1 /mnt/targetos/boot/efi || true", config.disk_device)
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

        // Update crypttab
    let crypttab_entry = format!("luks {}p4 /etc/luks.key luks", config.disk_device);
    let _ = self.ssh.execute(&format!("[ -d /mnt/targetos/etc ] || mkdir -p /mnt/targetos/etc; echo '{}' > /mnt/targetos/etc/crypttab", crypttab_entry)).await;

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
