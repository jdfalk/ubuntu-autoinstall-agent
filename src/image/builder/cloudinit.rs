// file: src/image/builder/cloudinit.rs
// version: 1.0.0
// guid: c1c2c3c4-d5d6-7890-1234-567890cdefgh

//! Cloud-init configuration generation

use crate::config::ImageSpec;
use crate::Result;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// Cloud-init configuration manager
pub struct CloudInitManager {
    work_dir: PathBuf,
}

impl CloudInitManager {
    /// Create a new cloud-init manager
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Create cloud-init configuration for automated installation
    pub async fn create_cloud_init_config(&self, spec: &ImageSpec) -> Result<PathBuf> {
        let cloud_init_dir = self.work_dir.join("cloud-init");
        fs::create_dir_all(&cloud_init_dir).await?;

        // Create user-data
        let user_data = self.generate_user_data(spec)?;
        let user_data_path = cloud_init_dir.join("user-data");
        fs::write(&user_data_path, user_data).await?;

        // Create meta-data
        let meta_data = format!("instance-id: ubuntu-autoinstall-{}\n", uuid::Uuid::new_v4());
        let meta_data_path = cloud_init_dir.join("meta-data");
        fs::write(&meta_data_path, meta_data).await?;

        debug!("Created cloud-init config in: {}", cloud_init_dir.display());
        Ok(cloud_init_dir)
    }

    /// Generate cloud-init user-data for automated installation
    fn generate_user_data(&self, spec: &ImageSpec) -> Result<String> {
        let packages = spec.base_packages.join("\n    - ");

        // Generate a password hash for the ubuntu user (password: 'ubuntu')
        // In production, this should be configurable or use key-based auth only
        let password_hash = "$6$rounds=4096$saltsalt$Nn9XLY39PZBO1NMdM9M1BKoJFtIpEcj1zLEHdN6mU.FWrJKOvE4PN8.BGeLhq.KOdFBVZ3MmE7Bl/VLy5L78O1";

        let config = format!(
            r#"#cloud-config
autoinstall:
  version: 1
  locale: en_US.UTF-8
  keyboard:
    layout: us
    variant: ''
  network:
    version: 2
    ethernets:
      eth0:
        dhcp4: true
  storage:
    layout:
      name: direct
      match:
        size: largest
  packages:
    - {}
  ssh:
    install-server: true
    allow-pw: false
    authorized-keys:
      - ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQDHQGvTZ8nZ8/temp-key-for-image-creation
  identity:
    realname: Ubuntu User
    username: ubuntu
    hostname: ubuntu-autoinstall
    password: '{}'
  kernel:
    flavor: generic
  timezone: UTC
  updates: security
  shutdown: reboot
  late-commands:
    # Configure GRUB for serial console
    - echo 'GRUB_TERMINAL="console serial"' >> /target/etc/default/grub
    - echo 'GRUB_SERIAL_COMMAND="serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1"' >> /target/etc/default/grub
    - echo 'GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200n8 console=tty0"' >> /target/etc/default/grub
    # Configure sudoers for passwordless sudo
    - echo 'ubuntu ALL=(ALL) NOPASSWD:ALL' >> /target/etc/sudoers.d/ubuntu-nopasswd
    - chmod 440 /target/etc/sudoers.d/ubuntu-nopasswd
    # Remove temporary SSH key - it will be replaced during VM provisioning
    - rm -f /target/home/ubuntu/.ssh/authorized_keys
    - echo "Image creation completed at $(date)" > /target/var/log/autoinstall.log
    # Update GRUB configuration
    - chroot /target update-grub
  error-commands:
    - echo "Installation failed at $(date)" > /target/var/log/autoinstall-error.log
    - journalctl -b > /target/var/log/autoinstall-journal.log
"#,
            packages, password_hash
        );

        Ok(config)
    }
}
