#!/usr/bin/env rust
// file: src/main.rs
// version: 2.0.0
// guid: b2c3d4e5-f6a7-8901-bcde-f23456789012

use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{info, error};

mod config;
mod installer;
mod recovery;
mod reporter;
mod steps;
mod utils;
mod image;

use config::InstallConfig;
use installer::InstallationAgent;
use image::{Architecture, ImageManager, ImageManagerConfig, TargetMachine, NetworkConfig, LuksConfig};

#[derive(Parser)]
#[command(name = "ubuntu-autoinstall-agent")]
#[command(about = "Revolutionary VM-based Ubuntu autoinstall system with ZFS encryption and error recovery")]
#[command(version = "2.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path or URL
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Webhook URL for status reporting
    #[arg(long, global = true)]
    webhook_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// NEW: Create a golden image using QEMU/KVM virtualization
    CreateImage {
        /// Ubuntu version (e.g., "22.04", "24.04")
        #[arg(short, long)]
        ubuntu_version: String,

        /// Target architecture
        #[arg(short, long, default_value = "amd64")]
        architecture: String,

        /// Custom image name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// NEW: Deploy a golden image to a target machine
    DeployImage {
        /// Image name to deploy
        #[arg(short, long)]
        image: String,

        /// Target hostname
        #[arg(short = 'H', long)]
        hostname: String,

        /// Target disk device (e.g., /dev/sda)
        #[arg(short, long)]
        disk: String,

        /// Network interface
        #[arg(long, default_value = "eth0")]
        interface: String,

        /// IP address with CIDR (e.g., 192.168.1.100/24)
        #[arg(long)]
        address: String,

        /// Gateway IP address
        #[arg(long)]
        gateway: String,

        /// DNS servers (comma-separated)
        #[arg(long, default_value = "8.8.8.8,8.8.4.4")]
        dns: String,

        /// Timezone
        #[arg(long, default_value = "UTC")]
        timezone: String,

        /// SSH public keys file
        #[arg(long)]
        ssh_keys: Option<PathBuf>,

        /// Customization template name
        #[arg(long)]
        template: Option<String>,
    },

    /// NEW: List available golden images
    ListImages,

    /// NEW: Remove an image from cache
    RemoveImage {
        /// Image name to remove
        name: String,
    },

    /// NEW: Clean up old images (keep only N most recent)
    CleanupImages {
        /// Number of images to keep
        #[arg(short, long, default_value = "3")]
        keep: usize,
    },

    /// NEW: Initialize the image manager
    InitImageManager,

    /// LEGACY: Run full automated installation (use DeployImage for new deployments)
    Install {
        /// Server hostname
        #[arg(short, long)]
        hostname: String,

        /// Disk device (e.g., /dev/nvme0n1)
        #[arg(short, long)]
        disk: Option<String>,

        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// LEGACY: Run specific installation step
    Step {
        /// Step name to execute
        step: String,

        /// Continue from this step onwards
        #[arg(short, long)]
        continue_from: bool,
    },

    /// Validate configuration file
    Validate {
        /// Configuration file to validate
        config: Option<String>,
    },

    /// Generate default configuration
    GenerateConfig {
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Configuration template type
        #[arg(short, long, default_value = "standard")]
        template: String,
    },

    /// Recovery operations
    Recovery {
        /// Recovery action to perform
        #[command(subcommand)]
        action: RecoveryAction,
    },

    /// System information and readiness check
    Info,
}

#[derive(Subcommand)]
enum RecoveryAction {
    /// Analyze system for recovery options
    Analyze,

    /// Repair bootloader configuration
    RepairBootloader,

    /// Reset LUKS encryption
    ResetLuks,

    /// Restore from backup
    RestoreBackup {
        /// Backup location
        backup_path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
        .init();

    info!("Ubuntu Autoinstall Agent v2.0.0 starting");

    match cli.command {
        // NEW IMAGE MANAGEMENT COMMANDS
        Commands::CreateImage { ubuntu_version, architecture, name } => {
            let arch = parse_architecture(&architecture)?;
            create_golden_image(&ubuntu_version, arch, name).await
        }

        Commands::DeployImage {
            image,
            hostname,
            disk,
            interface,
            address,
            gateway,
            dns,
            timezone,
            ssh_keys,
            template,
        } => {
            deploy_image(
                &image,
                &hostname,
                &disk,
                &interface,
                &address,
                &gateway,
                &dns,
                &timezone,
                ssh_keys.as_ref(),
                template.as_deref(),
            ).await
        }

        Commands::ListImages => list_images().await,

        Commands::RemoveImage { name } => remove_image(&name).await,

        Commands::CleanupImages { keep } => cleanup_images(keep).await,

        Commands::InitImageManager => init_image_manager().await,

        // LEGACY COMMANDS
        Commands::Install { hostname, disk, yes } => {
            info!("Legacy install mode - consider using DeployImage for golden image deployment");
            let config = load_config(cli.config).await?;
            let agent = InstallationAgent::new(config, cli.webhook_url);
            agent.install(&hostname, disk.as_deref(), yes).await
        }

        Commands::Step { step, continue_from } => {
            info!("Legacy step mode - consider using DeployImage for golden image deployment");
            let config = load_config(cli.config).await?;
            let agent = InstallationAgent::new(config, cli.webhook_url);
            agent.run_step(&step, continue_from).await
        }

        Commands::Validate { config } => {
            let config_path = config.or(cli.config);
            let config = load_config(config_path).await?;
            let agent = InstallationAgent::new(config, cli.webhook_url);
            agent.validate().await
        }

        Commands::GenerateConfig { output, template } => {
            let config = InstallConfig::generate_template(&template)?;
            let output_path = output.unwrap_or_else(|| PathBuf::from("autoinstall-config.yaml"));

            tokio::fs::write(&output_path, serde_yaml::to_string(&config)?)
                .await
                .context("Failed to write configuration file")?;

            info!("Configuration template written to: {:?}", output_path);
            Ok(())
        }

        Commands::Recovery { action } => {
            let config = load_config(cli.config).await?;
            let agent = InstallationAgent::new(config, cli.webhook_url);

            match action {
                RecoveryAction::Analyze => agent.analyze_recovery().await,
                RecoveryAction::RepairBootloader => agent.repair_bootloader().await,
                RecoveryAction::ResetLuks => agent.reset_luks().await,
                RecoveryAction::RestoreBackup { backup_path } => {
                    agent.restore_backup(&backup_path).await
                }
            }
        }

        Commands::Info => {
            let config = load_config(cli.config).await?;
            let agent = InstallationAgent::new(config, cli.webhook_url);
            agent.system_info().await
        }
    }
}

// NEW IMAGE MANAGEMENT FUNCTIONS

async fn create_golden_image(
    ubuntu_version: &str,
    architecture: Architecture,
    name: Option<String>,
) -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    manager.initialize().await?;

    let image_info = manager.create_golden_image(ubuntu_version, architecture, name).await?;

    info!("Golden image created successfully:");
    info!("  Name: {}", image_info.name);
    info!("  Version: {}", image_info.version);
    info!("  Architecture: {}", image_info.architecture.as_str());
    info!("  Size: {} MB", image_info.size_bytes / 1024 / 1024);
    info!("  Path: {:?}", image_info.path);

    Ok(())
}

async fn deploy_image(
    image_name: &str,
    hostname: &str,
    disk: &str,
    interface: &str,
    address: &str,
    gateway: &str,
    dns: &str,
    timezone: &str,
    ssh_keys_file: Option<&PathBuf>,
    template: Option<&str>,
) -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    // Find the specified image
    let images = manager.list_images().await?;
    let image_info = images
        .iter()
        .find(|img| img.name == image_name)
        .ok_or_else(|| anyhow::anyhow!("Image '{}' not found", image_name))?;

    // Load SSH keys if provided
    let ssh_keys = if let Some(keys_file) = ssh_keys_file {
        let content = tokio::fs::read_to_string(keys_file).await
            .context("Failed to read SSH keys file")?;
        content.lines().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };

    // Parse DNS servers
    let dns_servers: Vec<String> = dns.split(',').map(|s| s.trim().to_string()).collect();

    // Create target machine configuration
    let target = TargetMachine {
        hostname: hostname.to_string(),
        architecture: image_info.architecture,
        disk_device: disk.to_string(),
        network_config: NetworkConfig {
            interface: interface.to_string(),
            address: address.to_string(),
            gateway: gateway.to_string(),
            dns_servers,
        },
        luks_config: LuksConfig {
            cipher: "aes-xts-plain64".to_string(),
            key_size: 512,
            hash: "sha512".to_string(),
        },
        ssh_keys,
        timezone: timezone.to_string(),
    };

    manager.deploy_image(image_info, &target, template).await?;

    info!("Image deployed successfully to {}", hostname);
    Ok(())
}

async fn list_images() -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    let images = manager.list_images().await?;

    if images.is_empty() {
        println!("No images found in cache");
        return Ok(());
    }

    println!("Available Images:");
    println!("{:<20} {:<10} {:<8} {:<10} {:<20}", "Name", "Version", "Arch", "Size (MB)", "Created");
    println!("{:-<80}", "");

    for image in images {
        println!(
            "{:<20} {:<10} {:<8} {:<10} {:<20}",
            image.name,
            image.version,
            image.architecture.as_str(),
            image.size_bytes / 1024 / 1024,
            image.created_at.format("%Y-%m-%d %H:%M")
        );
    }

    Ok(())
}

async fn remove_image(name: &str) -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    manager.remove_image(name).await?;
    info!("Image '{}' removed successfully", name);
    Ok(())
}

async fn cleanup_images(keep: usize) -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    manager.cleanup_images(keep).await?;
    Ok(())
}

async fn init_image_manager() -> Result<()> {
    let config = ImageManagerConfig::default();
    let manager = ImageManager::new(config);

    manager.initialize().await?;
    info!("Image manager initialized successfully");
    Ok(())
}

fn parse_architecture(arch: &str) -> Result<Architecture> {
    match arch.to_lowercase().as_str() {
        "amd64" | "x86_64" | "x64" => Ok(Architecture::Amd64),
        "arm64" | "aarch64" => Ok(Architecture::Arm64),
        _ => Err(anyhow::anyhow!("Unsupported architecture: {}", arch)),
    }
}

// LEGACY HELPER FUNCTIONS

async fn load_config(config_path: Option<String>) -> Result<InstallConfig> {
    let path = config_path.unwrap_or_else(|| "autoinstall-config.yaml".to_string());
    InstallConfig::load(&path).await
}
