// file: src/main.rs
// version: 1.1.1
// guid: h8i9j0k1-l2m3-4567-8901-234567hijklm

//! Ubuntu AutoInstall Agent - Main entry point

use clap::Parser;
use ubuntu_autoinstall_agent::{
    cli::{args::Cli, commands::*},
    logging::logger,
    Result,
};
use tokio::signal;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    logger::init_logger(cli.verbose, cli.quiet)?;

    // Set up signal handling for graceful shutdown
    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        warn!("Received Ctrl+C, initiating graceful shutdown...");
        cleanup_on_exit().await;
    };

    // Execute command with signal handling
    let command_future = async {
        match cli.command {
            ubuntu_autoinstall_agent::cli::args::Commands::CreateImage {
                arch, version, output, spec, cache_dir
            } => {
                create_image_command(arch.into(), &version, output, spec, cache_dir).await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::Deploy {
                target, config, image, via_ssh, dry_run
            } => {
                deploy_command(&target, &config, &image, via_ssh, dry_run).await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::Validate { image } => {
                validate_command(&image).await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::CheckPrereqs => {
                check_prerequisites_command().await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::ListImages {
                filter_arch, json
            } => {
                list_images_command(filter_arch.map(Into::into), json).await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::Cleanup {
                older_than_days, dry_run
            } => {
                cleanup_command(older_than_days, dry_run).await
            }
            ubuntu_autoinstall_agent::cli::args::Commands::SshInstall {
                host, hostname, username, investigate_only, dry_run, hold_on_failure
            } => {
                ssh_install_command(&host, hostname, username, investigate_only, dry_run, hold_on_failure).await
            }
        }
    };

    // Run command with signal handling
    tokio::select! {
        result = command_future => result,
        _ = shutdown_signal => {
            warn!("Application interrupted by user");
            std::process::exit(130); // Standard exit code for Ctrl+C
        }
    }
}

/// Cleanup function called on exit
async fn cleanup_on_exit() {
    info!("Performing cleanup on exit...");

    // Kill any running QEMU processes
    let _ = tokio::process::Command::new("pkill")
        .args(["-f", "qemu-system"])
        .output()
        .await;

    // Cleanup temporary files
    let cleanup_files = [
        "/tmp/qemu-serial.log",
        "/tmp/qemu-uefi.log",
        "/tmp/qemu-monitor.sock",
        "/tmp/OVMF_VARS.fd",
    ];

    for file in &cleanup_files {
        let _ = tokio::fs::remove_file(file).await;
    }

    info!("Cleanup completed");
}
