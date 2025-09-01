#!/usr/bin/env rust
// file: src/main.rs
// version: 1.0.0
// guid: b2c3d4e5-f6a7-8901-bcde-f23456789012

use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;
use tracing::{info, error};

mod config;
mod installer;
mod recovery;
mod reporter;
mod steps;
mod utils;

use config::InstallConfig;
use installer::InstallationAgent;

#[derive(Parser)]
#[command(name = "ubuntu-autoinstall-agent")]
#[command(about = "Comprehensive Ubuntu Server auto-installer with ZFS encryption and error recovery")]
#[command(version = "1.0.0")]
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
    /// Run full automated installation
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

    /// Run specific installation step
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

    /// Attempt to recover from failed installation
    Recover {
        /// Step that failed
        step: String,
    },

    /// Cleanup failed installation artifacts
    Cleanup,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    utils::logging::init_logging(cli.verbose).await?;

    info!("Ubuntu AutoInstall Agent v1.0.0 starting");

    match cli.command {
        Commands::Install { hostname, disk, yes } => {
            let config = load_config(&cli.config).await?;
            let mut agent = InstallationAgent::new(config, cli.webhook_url).await?;

            if !yes {
                utils::confirm_installation(&hostname, &disk).await?;
            }

            agent.run_full_installation(hostname, disk).await
        }

        Commands::Step { step, continue_from } => {
            let config = load_config(&cli.config).await?;
            let mut agent = InstallationAgent::new(config, cli.webhook_url).await?;

            if continue_from {
                agent.run_from_step(&step).await
            } else {
                agent.run_single_step(&step).await
            }
        }

        Commands::Validate { config } => {
            let config_path = config.or(cli.config);
            validate_config(config_path).await
        }

        Commands::GenerateConfig { output, template } => {
            config::generator::generate_config(&template, output).await
        }

        Commands::Recovery { action } => {
            match action {
                RecoveryAction::Analyze => recovery::analyze_system().await,
                RecoveryAction::Recover { step } => recovery::recover_from_step(&step).await,
                RecoveryAction::Cleanup => recovery::cleanup_failed_installation().await,
            }
        }

        Commands::Info => {
            utils::system_info::display_system_info().await
        }
    }
}

async fn load_config(config_path: &Option<String>) -> Result<InstallConfig> {
    match config_path {
        Some(path) => {
            if path.starts_with("http://") || path.starts_with("https://") {
                // Download from URL
                config::loader::load_from_url(path).await
            } else {
                // Load from file
                config::loader::load_from_file(path).await
            }
        }
        None => {
            // Use default embedded config
            config::loader::load_default().await
        }
    }
}

async fn validate_config(config_path: Option<String>) -> Result<()> {
    let config = load_config(&config_path).await?;

    match config::validator::validate(&config).await {
        Ok(_) => {
            info!("✅ Configuration is valid");
            Ok(())
        }
        Err(errors) => {
            error!("❌ Configuration validation failed:");
            for error in errors {
                error!("  - {}", error);
            }
            std::process::exit(1);
        }
    }
}
