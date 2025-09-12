// file: src/image/builder/cloudinit.rs
// version: 1.0.0
// guid: c1c2c3c4-d5d6-7890-1234-567890cdefgh

//! Cloud-init configuration generation

use std::path::PathBuf;
use tokio::fs;
use tracing::debug;
use crate::config::ImageSpec;
use crate::Result;

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
        let packages = spec.base_packages.join("\n      - ");

        let config = format!(r#"#cloud-config
autoinstall:
  version: 1
  locale: en_US.UTF-8
  keyboard:
    layout: us
    variant: ''
  network:
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
    swap:
      size: 0
  packages:
      - {}
  ssh:
    install-server: true
    allow-pw: false
  user-data:
    disable_root: true
    users:
      - name: ubuntu
        sudo: ALL=(ALL) NOPASSWD:ALL
        shell: /bin/bash
        lock_passwd: true
        ssh_authorized_keys:
          - ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQDHQGvTZ8nZ8/temp-key-for-image-creation
    timezone: UTC
  kernel:
    package: linux-generic
    cmdline: "console=ttyS0,115200n8 console=tty0"
  grub:
    terminal: "console serial"
    terminal_input: "console serial"
    terminal_output: "console serial"
    serial_command: "serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1"
    cmdline_linux_default: "console=ttyS0,115200n8 console=tty0"
  late-commands:
    # Configure GRUB for serial console
    - echo 'GRUB_TERMINAL="console serial"' >> /target/etc/default/grub
    - echo 'GRUB_SERIAL_COMMAND="serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1"' >> /target/etc/default/grub
    - echo 'GRUB_CMDLINE_LINUX_DEFAULT="console=ttyS0,115200n8 console=tty0"' >> /target/etc/default/grub
    # Remove temporary SSH key and prepare image for generalization
    - rm -f /target/home/ubuntu/.ssh/authorized_keys
    - echo "Image creation completed at $(date)" > /target/var/log/autoinstall.log
    # Ensure cloud-init will run on first boot
    - touch /target/etc/cloud/cloud-init.disabled && rm /target/etc/cloud/cloud-init.disabled
    # Clean up any installer logs that might contain sensitive data
    - rm -f /target/var/log/installer/autoinstall-user-data
    # Update GRUB configuration
    - chroot /target update-grub
  error-commands:
    - echo "Installation failed at $(date)" > /target/var/log/autoinstall-error.log
"#, packages);

        Ok(config)
    }
}
