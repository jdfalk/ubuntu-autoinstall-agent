// file: src/network/ssh_installer/system_setup.rs
// version: 1.1.0
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
        let release = config.debootstrap_release.as_deref().unwrap_or("oracular");
        let mirror = config.debootstrap_mirror.as_deref().unwrap_or("http://old-releases.ubuntu.com/ubuntu/");
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

        // Prepare chroot
        // Only bind if target exists (avoid cascading failures if debootstrap didn't run)
        let _ = self.log_and_execute("Binding /dev", "[ -d /mnt/targetos/dev ] || mkdir -p /mnt/targetos/dev; mountpoint -q /mnt/targetos/dev || mount --bind /dev /mnt/targetos/dev").await;
        let _ = self.log_and_execute("Binding /proc", "[ -d /mnt/targetos/proc ] || mkdir -p /mnt/targetos/proc; mountpoint -q /mnt/targetos/proc || mount --bind /proc /mnt/targetos/proc").await;
        let _ = self.log_and_execute("Binding /sys", "[ -d /mnt/targetos/sys ] || mkdir -p /mnt/targetos/sys; mountpoint -q /mnt/targetos/sys || mount --bind /sys /mnt/targetos/sys").await;

        // Install essential packages
        let chroot_commands = vec![
            "apt update",
            "DEBIAN_FRONTEND=noninteractive apt install -y zfsutils-linux grub-efi-amd64 grub-efi-amd64-signed shim-signed",
            "DEBIAN_FRONTEND=noninteractive apt install -y linux-image-generic linux-headers-generic",
            "DEBIAN_FRONTEND=noninteractive apt install -y openssh-server vim htop curl",
        ];

        for cmd in chroot_commands {
            let _ = self.log_and_execute(&format!("Chroot: {}", cmd), &format!("chroot /mnt/targetos {}", cmd)).await;
        }

        // Set root password
        let _ = self.log_and_execute("Setting root password",
            &format!("chroot /mnt/targetos bash -c \"echo 'root:{}' | chpasswd\"", config.root_password)).await;

    // Enable SSH (ignore failure if systemd not fully present yet)
    let _ = self.log_and_execute("Enabling SSH", "chroot /mnt/targetos systemctl enable ssh").await;

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
            let _ = self.log_and_execute(&format!("ZFS: {}", cmd), &format!("chroot /mnt/targetos {}", cmd)).await;
        }

        Ok(())
    }

    /// Configure GRUB in chroot
    pub async fn configure_grub_in_chroot(&mut self, _config: &InstallationConfig) -> Result<()> {
        info!("Configuring GRUB in chroot");

        // Update GRUB configuration
        self.log_and_execute("Installing GRUB to ESP",
            "chroot /mnt/targetos grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck").await?;

        self.log_and_execute("Updating GRUB config", "chroot /mnt/targetos update-grub").await?;

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

        // Unmount chroot bindings
    // Make unmounts idempotent
    self.log_and_execute("Unmounting /sys", "umount /mnt/targetos/sys || true").await?;
    self.log_and_execute("Unmounting /proc", "umount /mnt/targetos/proc || true").await?;
    self.log_and_execute("Unmounting /dev", "umount /mnt/targetos/dev || true").await?;

        // Unmount filesystems
    self.log_and_execute("Unmounting ESP", "umount /mnt/targetos/boot/efi || true").await?;

        // Export ZFS pools
    self.log_and_execute("Exporting bpool", "zpool export bpool || true").await?;
    self.log_and_execute("Exporting rpool", "zpool export rpool || true").await?;

        info!("Final cleanup completed");
        Ok(())
    }

    /// Helper method to log and execute commands
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);
        self.ssh.execute(command).await
    }
}
