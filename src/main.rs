// file: src/main.rs
// version: 1.0.0
// guid: h8i9j0k1-l2m3-4567-8901-234567hijklm

//! Ubuntu AutoInstall Agent - Main entry point

use clap::Parser;
use ubuntu_autoinstall_agent::{
    cli::{args::Cli, commands::*},
    logging::logger,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    logger::init_logger(cli.verbose, cli.quiet)?;

    // Execute command
    match cli.command {
        ubuntu_autoinstall_agent::cli::args::Commands::CreateImage { 
            arch, version, output, spec 
        } => {
            create_image_command(arch.into(), &version, output, spec).await
        }
        ubuntu_autoinstall_agent::cli::args::Commands::Deploy { 
            target, config, via_ssh, dry_run 
        } => {
            deploy_command(&target, &config, via_ssh, dry_run).await
        }
        ubuntu_autoinstall_agent::cli::args::Commands::Validate { image } => {
            validate_command(&image).await
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
    }
}