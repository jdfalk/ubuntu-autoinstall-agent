// file: src/network/ssh_installer.rs
// version: 1.0.0
// guid: s1h2i3j4-k5l6-7890-1234-567890shijkl

//! SSH-based Ubuntu installation with ZFS and LUKS

use std::collections::HashMap;
use tracing::{info, warn, error};
use crate::network::SshClient;
use crate::Result;

/// SSH-based installer for Ubuntu with ZFS and LUKS
pub struct SshInstaller {
    ssh: SshClient,
    connected: bool,
    variables: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct InstallationConfig {
    pub hostname: String,
    pub disk_device: String,
    pub timezone: String,
    pub luks_key: String,
    pub root_password: String,
    pub network_interface: String,
    pub network_address: String,
    pub network_gateway: String,
    pub network_search: String,
    pub network_nameservers: Vec<String>,
}

impl InstallationConfig {
    /// Create configuration for len-serv-003 based on the variables.sh file
    pub fn for_len_serv_003() -> Self {
        Self {
            hostname: "len-serv-003".to_string(),
            disk_device: "/dev/nvme0n1".to_string(),
            timezone: "America/New_York".to_string(),
            luks_key: "defaultLUKSkey123".to_string(),
            root_password: "defaultPassword123".to_string(),
            network_interface: "enp1s0f0".to_string(),
            network_address: "172.16.3.96/23".to_string(),
            network_gateway: "172.16.2.1".to_string(),
            network_search: "jf.local".to_string(),
            network_nameservers: vec![
                "172.16.2.1".to_string(),
                "1.1.1.1".to_string(),
                "8.8.8.8".to_string(),
            ],
        }
    }
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

    /// Connect to the target machine via SSH
    pub async fn connect(&mut self, host: &str, username: &str) -> Result<()> {
        info!("Attempting to connect to {}@{}", username, host);

        // Try connecting with SSH keys first
        match self.ssh.connect(host, username).await {
            Ok(_) => {
                self.connected = true;
                info!("Successfully connected to {} as {}", host, username);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to {} as {}: {}", host, username, e);
                Err(e)
            }
        }
    }

    /// Perform comprehensive system investigation
    pub async fn investigate_system(&mut self) -> Result<SystemInfo> {
        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string()
            ));
        }

        info!("Starting comprehensive system investigation");

        let mut system_info = SystemInfo::default();

        // Get basic system information
        system_info.hostname = self.get_command_output("hostname").await?;
        system_info.kernel_version = self.get_command_output("uname -r").await?;
        system_info.os_release = self.get_command_output("cat /etc/os-release").await?;

        // Investigate disk layout
        system_info.disk_info = self.investigate_disks().await?;

        // Check network configuration
        system_info.network_info = self.investigate_network().await?;

        // Check available tools
        system_info.available_tools = self.check_available_tools().await?;

        // Check memory and CPU
        system_info.memory_info = self.get_command_output("free -h").await?;
        system_info.cpu_info = self.get_command_output("lscpu").await?;

        info!("System investigation completed");
        Ok(system_info)
    }

    /// Investigate disk configuration
    async fn investigate_disks(&mut self) -> Result<String> {
        info!("Investigating disk configuration");

        let mut disk_info = String::new();

        // List all block devices
        disk_info.push_str("=== Block Devices ===\n");
        disk_info.push_str(&self.get_command_output("lsblk -a").await?);
        disk_info.push_str("\n\n");

        // Show disk details
        disk_info.push_str("=== Disk Details ===\n");
        disk_info.push_str(&self.get_command_output("fdisk -l").await?);
        disk_info.push_str("\n\n");

        // Check for existing ZFS pools
        disk_info.push_str("=== ZFS Pools ===\n");
        let zfs_pools = self.get_command_output("zpool list 2>/dev/null || echo 'No ZFS pools found'").await?;
        disk_info.push_str(&zfs_pools);
        disk_info.push_str("\n\n");

        // Check for LUKS devices
        disk_info.push_str("=== LUKS Devices ===\n");
        let luks_devices = self.get_command_output("cryptsetup status luks 2>/dev/null || echo 'No LUKS devices found'").await?;
        disk_info.push_str(&luks_devices);
        disk_info.push_str("\n\n");

        // Check mounted filesystems
        disk_info.push_str("=== Mounted Filesystems ===\n");
        disk_info.push_str(&self.get_command_output("mount | grep '^/dev'").await?);

        Ok(disk_info)
    }

    /// Investigate network configuration
    async fn investigate_network(&mut self) -> Result<String> {
        info!("Investigating network configuration");

        let mut network_info = String::new();

        // Network interfaces
        network_info.push_str("=== Network Interfaces ===\n");
        network_info.push_str(&self.get_command_output("ip addr show").await?);
        network_info.push_str("\n\n");

        // Routing table
        network_info.push_str("=== Routing Table ===\n");
        network_info.push_str(&self.get_command_output("ip route show").await?);
        network_info.push_str("\n\n");

        // DNS configuration
        network_info.push_str("=== DNS Configuration ===\n");
        network_info.push_str(&self.get_command_output("cat /etc/resolv.conf").await?);

        Ok(network_info)
    }

    /// Check available installation tools
    async fn check_available_tools(&mut self) -> Result<Vec<String>> {
        info!("Checking available installation tools");

        let required_tools = vec![
            "zfsutils-linux", "cryptsetup", "parted", "gdisk", "debootstrap",
            "mkfs.fat", "mkfs.xfs", "wipefs", "sgdisk", "blkdiscard"
        ];

        let mut available = Vec::new();
        let mut missing = Vec::new();

        for tool in &required_tools {
            match self.ssh.execute(&format!("command -v {} >/dev/null 2>&1", tool)).await {
                Ok(_) => available.push(tool.to_string()),
                Err(_) => missing.push(tool.to_string()),
            }
        }

        if !missing.is_empty() {
            warn!("Missing required tools: {:?}", missing);
            info!("Attempting to install missing packages...");
            self.install_required_packages().await?;
        }

        Ok(available)
    }

    /// Install required packages for installation
    async fn install_required_packages(&mut self) -> Result<()> {
        info!("Installing required packages");

        // Update package lists
        self.ssh.execute("apt update").await?;

        // Install required packages
        let packages = vec![
            "zfsutils-linux", "cryptsetup", "parted", "gdisk", "debootstrap",
            "dosfstools", "xfsprogs", "util-linux"
        ];

        let install_cmd = format!("DEBIAN_FRONTEND=noninteractive apt install -y {}", packages.join(" "));
        self.ssh.execute(&install_cmd).await?;

        info!("Required packages installed successfully");
        Ok(())
    }

    /// Perform full ZFS + LUKS installation
    pub async fn perform_installation(&mut self, config: &InstallationConfig) -> Result<()> {
        if !self.connected {
            return Err(crate::error::AutoInstallError::SshError(
                "Not connected to target system".to_string()
            ));
        }

        info!("Starting full ZFS + LUKS installation for {}", config.hostname);

        // Setup installation variables
        self.setup_installation_variables(config).await?;

        // Phase 1: Disk preparation
        self.phase_1_disk_preparation(config).await?;

        // Phase 2: ZFS pool creation
        self.phase_2_zfs_creation(config).await?;

        // Phase 3: Base system installation
        self.phase_3_base_system(config).await?;

        // Phase 4: System configuration
        self.phase_4_system_configuration(config).await?;

        // Phase 5: Final setup
        self.phase_5_final_setup(config).await?;

        info!("Installation completed successfully for {}", config.hostname);
        Ok(())
    }

    /// Setup installation variables
    async fn setup_installation_variables(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Setting up installation variables");

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

    /// Phase 1: Disk preparation and partitioning
    async fn phase_1_disk_preparation(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 1: Disk preparation and partitioning");

        // Stop unnecessary services
        self.log_and_execute("Stopping zed service", "systemctl stop zed || true").await?;

        // Configure timezone
        self.log_and_execute(
            "Setting timezone",
            &format!("timedatectl set-timezone {}", config.timezone)
        ).await?;

        self.log_and_execute("Enabling NTP", "timedatectl set-ntp on").await?;

        // Destroy existing ZFS pools
        let existing_pools = self.get_command_output("zpool list -H -o name 2>/dev/null || true").await?;
        if !existing_pools.trim().is_empty() {
            for pool in existing_pools.lines() {
                if !pool.trim().is_empty() {
                    self.log_and_execute(
                        &format!("Destroying ZFS pool: {}", pool.trim()),
                        &format!("zpool destroy {}", pool.trim())
                    ).await?;
                }
            }
        }

        // Wipe and partition disk
        self.log_and_execute("Wiping disk", &format!("wipefs -a {}", config.disk_device)).await?;
        self.log_and_execute("Discarding blocks", &format!("blkdiscard -f {}", config.disk_device)).await?;
        self.log_and_execute("Zapping GPT", &format!("sgdisk --zap-all {}", config.disk_device)).await?;

        // Create partitions
        self.log_and_execute("Creating GPT table", &format!("parted -s {} mklabel gpt", config.disk_device)).await?;

        // ESP partition (1MiB to 513MiB)
        self.log_and_execute("Creating ESP partition",
            &format!("parted -s {} mkpart ESP fat32 1MiB 513MiB", config.disk_device)).await?;
        self.log_and_execute("Setting ESP boot flag",
            &format!("parted -s {} set 1 boot on", config.disk_device)).await?;
        self.log_and_execute("Setting ESP esp flag",
            &format!("parted -s {} set 1 esp on", config.disk_device)).await?;

        // RESET partition (513MiB to 4609MiB)
        self.log_and_execute("Creating RESET partition",
            &format!("parted -s {} mkpart RESET fat32 513MiB 4609MiB", config.disk_device)).await?;

        // BPOOL partition (4609MiB to 6657MiB)
        self.log_and_execute("Creating BPOOL partition",
            &format!("parted -s {} mkpart BPOOL 4609MiB 6657MiB", config.disk_device)).await?;

        // LUKS partition (6657MiB to 7681MiB)
        self.log_and_execute("Creating LUKS partition",
            &format!("parted -s {} mkpart LUKS 6657MiB 7681MiB", config.disk_device)).await?;

        // RPOOL partition (7681MiB to 100%)
        self.log_and_execute("Creating RPOOL partition",
            &format!("parted -s {} mkpart RPOOL 7681MiB 100%", config.disk_device)).await?;

        // Format ESP and RESET partitions
        self.log_and_execute("Formatting ESP", &format!("mkfs.fat -F32 {}p1", config.disk_device)).await?;
        self.log_and_execute("Formatting RESET", &format!("mkfs.fat -F32 {}p2", config.disk_device)).await?;

        // Setup LUKS encryption
        self.log_and_execute("Setting up LUKS encryption",
            &format!("echo '{}' | cryptsetup luksFormat --batch-mode {}p4", config.luks_key, config.disk_device)).await?;
        self.log_and_execute("Opening LUKS device",
            &format!("echo '{}' | cryptsetup open {}p4 luks", config.luks_key, config.disk_device)).await?;
        self.log_and_execute("Creating XFS on LUKS", "mkfs.xfs -f -b size=4096 /dev/mapper/luks").await?;

        info!("Phase 1 completed: Disk preparation and partitioning");
        Ok(())
    }

    /// Phase 2: ZFS pool and dataset creation
    async fn phase_2_zfs_creation(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 2: ZFS pool and dataset creation");

        // Prepare for ZFS
        self.log_and_execute("Creating /mnt/luks", "mkdir -p /mnt/luks").await?;
        self.log_and_execute("Mounting LUKS", "mount /dev/mapper/luks /mnt/luks").await?;
        self.log_and_execute("Generating ZFS key", "dd if=/dev/random of=/mnt/luks/zfs.key bs=32 count=1").await?;
        self.log_and_execute("Setting ZFS key permissions", "chmod 600 /mnt/luks/zfs.key").await?;
        self.log_and_execute("Unmounting LUKS", "umount /mnt/luks").await?;
        self.log_and_execute("Closing LUKS", "cryptsetup close luks").await?;
        self.log_and_execute("Creating target directory", "mkdir -p /mnt/targetos").await?;

        // Generate UUID for dataset naming
        let uuid = self.generate_installation_uuid().await?;
        self.variables.insert("UUID".to_string(), uuid.clone());

        // Create bpool
        let bpool_cmd = format!(
            "zpool create -o ashift=12 -o autotrim=on -o cachefile=/etc/zfs/zpool.cache \
             -o compatibility=grub2 -o feature@livelist=enabled -o feature@zpool_checkpoint=enabled \
             -O devices=off -O acltype=posixacl -O xattr=sa -O compression=lz4 \
             -O normalization=formD -O relatime=on -O canmount=off -O mountpoint=/boot \
             -R /mnt/targetos bpool {}p3", config.disk_device
        );
        self.log_and_execute("Creating bpool", &bpool_cmd).await?;

        // Create rpool with encryption
        let rpool_cmd = format!(
            "zpool create -o ashift=12 -o autotrim=on \
             -O encryption=on -O keylocation=file:///mnt/luks/zfs.key -O keyformat=raw \
             -O acltype=posixacl -O xattr=sa -O dnodesize=auto -O compression=lz4 \
             -O normalization=formD -O relatime=on -O canmount=off -O mountpoint=/ \
             -R /mnt/targetos rpool {}p5", config.disk_device
        );
        self.log_and_execute("Creating rpool", &rpool_cmd).await?;

        // Create bpool datasets
        self.log_and_execute("Creating bpool/BOOT", "zfs create -o canmount=off -o mountpoint=none bpool/BOOT").await?;
        self.log_and_execute("Creating bpool boot dataset",
            &format!("zfs create -o mountpoint=/boot bpool/BOOT/ubuntu_{}", uuid)).await?;

        // Create rpool datasets
        self.create_rpool_datasets(&uuid).await?;

        info!("Phase 2 completed: ZFS pools and datasets created");
        Ok(())
    }

    /// Create comprehensive rpool dataset structure
    async fn create_rpool_datasets(&mut self, uuid: &str) -> Result<()> {
        info!("Creating rpool dataset structure");

        // Root dataset structure
        self.log_and_execute("Creating rpool/ROOT", "zfs create -o canmount=off -o mountpoint=none rpool/ROOT").await?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_and_execute("Creating root filesystem",
            &format!("zfs create -o mountpoint=/ -o com.ubuntu.zsys:bootfs=yes -o com.ubuntu.zsys:last-used={} rpool/ROOT/ubuntu_{}", current_time, uuid)).await?;

        // System directories
        let datasets = vec![
            ("usr", "rpool/ROOT/ubuntu_{}/usr", "-o com.ubuntu.zsys:bootfs=no -o canmount=off"),
            ("var", "rpool/ROOT/ubuntu_{}/var", "-o com.ubuntu.zsys:bootfs=no -o canmount=off"),
            ("var/lib", "rpool/ROOT/ubuntu_{}/var/lib", ""),
            ("var/log", "rpool/ROOT/ubuntu_{}/var/log", ""),
            ("var/spool", "rpool/ROOT/ubuntu_{}/var/spool", ""),
            ("var/cache", "rpool/ROOT/ubuntu_{}/var/cache", ""),
            ("var/lib/nfs", "rpool/ROOT/ubuntu_{}/var/lib/nfs", ""),
            ("var/tmp", "rpool/ROOT/ubuntu_{}/var/tmp", ""),
            ("var/lib/apt", "rpool/ROOT/ubuntu_{}/var/lib/apt", ""),
            ("var/lib/dpkg", "rpool/ROOT/ubuntu_{}/var/lib/dpkg", ""),
            ("srv", "rpool/ROOT/ubuntu_{}/srv", "-o com.ubuntu.zsys:bootfs=no"),
            ("usr/local", "rpool/ROOT/ubuntu_{}/usr/local", ""),
            ("var/games", "rpool/ROOT/ubuntu_{}/var/games", ""),
            ("var/lib/AccountsService", "rpool/ROOT/ubuntu_{}/var/lib/AccountsService", ""),
        ];

        for (name, dataset, opts) in datasets {
            let dataset_name = dataset.replace("{}", uuid);
            self.log_and_execute(
                &format!("Creating {}", name),
                &format!("zfs create {} {}", opts, dataset_name)
            ).await?;
        }

        // Set special permissions
        self.log_and_execute("Setting /root permissions", "chmod 700 /mnt/targetos/root").await?;
        self.log_and_execute("Setting /var/tmp permissions", "chmod 1777 /mnt/targetos/var/tmp").await?;

        // Create USERDATA structure
        self.log_and_execute("Creating USERDATA", "zfs create -o canmount=off -o mountpoint=/ rpool/USERDATA").await?;
        self.log_and_execute("Creating root user data",
            &format!("zfs create -o com.ubuntu.zsys:bootfs-datasets=rpool/ROOT/ubuntu_{} -o canmount=on -o mountpoint=/root rpool/USERDATA/root_{}", uuid, uuid)).await?;

        Ok(())
    }

    /// Generate unique UUID for this installation
    async fn generate_installation_uuid(&mut self) -> Result<String> {
        let uuid_output = self.get_command_output("dd if=/dev/urandom bs=1 count=100 2>/dev/null | tr -dc 'a-z0-9' | cut -c-6").await?;
        let uuid = uuid_output.trim().to_string();

        // Write UUID to target
        self.ssh.execute(&format!("echo 'UUID={}' > /mnt/targetos/uuid", uuid)).await?;
        self.ssh.execute(&format!("echo 'DISK={}' >> /mnt/targetos/uuid", self.variables.get("DISK").unwrap_or(&"unknown".to_string()))).await?;

        info!("Generated installation UUID: {}", uuid);
        Ok(uuid)
    }

    /// Phase 3: Base system installation
    async fn phase_3_base_system(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 3: Base system installation");

        // Mount ESP partition
        self.log_and_execute("Creating ESP mount point", "mkdir -p /mnt/targetos/boot/efi").await?;
        self.log_and_execute("Mounting ESP", &format!("mount {}p1 /mnt/targetos/boot/efi", config.disk_device)).await?;

        // Install base system using debootstrap
        self.log_and_execute("Running debootstrap",
            "debootstrap oracular /mnt/targetos http://archive.ubuntu.com/ubuntu/").await?;

        // Setup basic system files
        self.setup_basic_system_files(config).await?;

        // Chroot and configure system
        self.configure_system_in_chroot(config).await?;

        info!("Phase 3 completed: Base system installed");
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
        self.log_and_execute("Binding /dev", "mount --bind /dev /mnt/targetos/dev").await?;
        self.log_and_execute("Binding /proc", "mount --bind /proc /mnt/targetos/proc").await?;
        self.log_and_execute("Binding /sys", "mount --bind /sys /mnt/targetos/sys").await?;

        // Install essential packages
        let chroot_commands = vec![
            "apt update",
            "DEBIAN_FRONTEND=noninteractive apt install -y zfsutils-linux grub-efi-amd64 grub-efi-amd64-signed shim-signed",
            "DEBIAN_FRONTEND=noninteractive apt install -y linux-image-generic linux-headers-generic",
            "DEBIAN_FRONTEND=noninteractive apt install -y openssh-server vim htop curl",
        ];

        for cmd in chroot_commands {
            self.log_and_execute(&format!("Chroot: {}", cmd), &format!("chroot /mnt/targetos {}", cmd)).await?;
        }

        // Set root password
        self.log_and_execute("Setting root password",
            &format!("chroot /mnt/targetos bash -c \"echo 'root:{}' | chpasswd\"", config.root_password)).await?;

        // Enable SSH
        self.log_and_execute("Enabling SSH", "chroot /mnt/targetos systemctl enable ssh").await?;

        Ok(())
    }

    /// Phase 4: System configuration
    async fn phase_4_system_configuration(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 4: System configuration");

        // Configure ZFS
        self.configure_zfs_in_chroot().await?;

        // Configure GRUB
        self.configure_grub_in_chroot(config).await?;

        // Setup LUKS key
        self.setup_luks_key_in_chroot(config).await?;

        info!("Phase 4 completed: System configuration");
        Ok(())
    }

    /// Configure ZFS in chroot
    async fn configure_zfs_in_chroot(&mut self) -> Result<()> {
        info!("Configuring ZFS in chroot");

        // Enable ZFS services
        let zfs_commands = vec![
            "systemctl enable zfs-import-cache",
            "systemctl enable zfs-mount",
            "systemctl enable zfs-import.target",
            "update-initramfs -u -k all",
        ];

        for cmd in zfs_commands {
            self.log_and_execute(&format!("ZFS: {}", cmd), &format!("chroot /mnt/targetos {}", cmd)).await?;
        }

        Ok(())
    }

    /// Configure GRUB in chroot
    async fn configure_grub_in_chroot(&mut self, _config: &InstallationConfig) -> Result<()> {
        info!("Configuring GRUB in chroot");

        // Update GRUB configuration
        self.log_and_execute("Installing GRUB to ESP",
            &format!("chroot /mnt/targetos grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=ubuntu --recheck")).await?;

        self.log_and_execute("Updating GRUB config", "chroot /mnt/targetos update-grub").await?;

        Ok(())
    }

    /// Configure LUKS key handling in chroot
    async fn setup_luks_key_in_chroot(&mut self, _config: &InstallationConfig) -> Result<()> {
        info!("Setting up LUKS key in chroot");

        // Create keyfile in target system
        self.log_and_execute("Creating LUKS keyfile",
            &format!("echo '{}' > /mnt/targetos/etc/luks.key", _config.luks_key)).await?;
        self.log_and_execute("Setting keyfile permissions", "chmod 600 /mnt/targetos/etc/luks.key").await?;

        // Update crypttab
        let crypttab_entry = format!("luks {}p4 /etc/luks.key luks", _config.disk_device);
        self.ssh.execute(&format!("echo '{}' > /mnt/targetos/etc/crypttab", crypttab_entry)).await?;

        Ok(())
    }

    /// Phase 5: Final setup and cleanup
    async fn phase_5_final_setup(&mut self, config: &InstallationConfig) -> Result<()> {
        info!("Phase 5: Final setup and cleanup");

        // Unmount chroot bindings
        self.log_and_execute("Unmounting /sys", "umount /mnt/targetos/sys").await?;
        self.log_and_execute("Unmounting /proc", "umount /mnt/targetos/proc").await?;
        self.log_and_execute("Unmounting /dev", "umount /mnt/targetos/dev").await?;

        // Unmount filesystems
        self.log_and_execute("Unmounting ESP", "umount /mnt/targetos/boot/efi").await?;

        // Export ZFS pools
        self.log_and_execute("Exporting bpool", "zpool export bpool").await?;
        self.log_and_execute("Exporting rpool", "zpool export rpool").await?;

        info!("Phase 5 completed: Final setup and cleanup");
        info!("Installation of {} completed successfully!", config.hostname);
        Ok(())
    }

    /// Helper method to log and execute commands
    async fn log_and_execute(&mut self, description: &str, command: &str) -> Result<()> {
        info!("Executing: {} -> {}", description, command);
        self.ssh.execute(command).await
    }

    /// Helper method to get command output
    async fn get_command_output(&mut self, command: &str) -> Result<String> {
        self.ssh.execute_with_output(command).await
    }
}

#[derive(Debug, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub kernel_version: String,
    pub os_release: String,
    pub disk_info: String,
    pub network_info: String,
    pub available_tools: Vec<String>,
    pub memory_info: String,
    pub cpu_info: String,
}
