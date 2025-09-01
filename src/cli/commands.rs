// file: src/cli/commands.rs
// version: 1.0.0
// guid: g7h8i9j0-k1l2-3456-7890-123456ghijkl

//! Command implementations for the CLI

use crate::{
    config::{Architecture, loader::ConfigLoader, ImageSpec},
    image::{builder::ImageBuilder, manager::ImageManager},
    image::deployer::ImageDeployer,
    Result,
};
use tracing::{info, error};

/// Create a golden Ubuntu image
pub async fn create_image_command(
    arch: Architecture,
    version: &str,
    output: Option<String>,
    spec_path: Option<String>,
) -> Result<()> {
    info!("Creating Ubuntu {} image for {} architecture", version, arch.as_str());

    let spec = if let Some(spec_path) = spec_path {
        let loader = ConfigLoader::new();
        loader.load_image_spec(&spec_path)?
    } else {
        ImageSpec::minimal(version.to_string(), arch)
    };

    let builder = ImageBuilder::new();
    let image_path = builder.create_image(spec, output).await?;
    
    info!("Image created successfully: {}", image_path.display());
    Ok(())
}

/// Deploy image to target machine
pub async fn deploy_command(
    target: &str,
    config_path: &str,
    via_ssh: bool,
    dry_run: bool,
) -> Result<()> {
    info!("Deploying image to target: {}", target);

    let loader = ConfigLoader::new();
    let config = loader.load_target_config(config_path)?;

    if dry_run {
        info!("DRY RUN: Would deploy to {} via {}", 
              target, if via_ssh { "SSH" } else { "netboot" });
        info!("Target config: hostname={}, arch={}", 
              config.hostname, config.architecture.as_str());
        return Ok(());
    }

    let deployer = ImageDeployer::new();
    if via_ssh {
        deployer.deploy_via_ssh(target, &config).await?;
    } else {
        deployer.deploy_via_netboot(target, &config).await?;
    }

    info!("Deployment completed successfully");
    Ok(())
}

/// Validate image integrity
pub async fn validate_command(image_path: &str) -> Result<()> {
    info!("Validating image: {}", image_path);

    let manager = ImageManager::new();
    let is_valid = manager.validate_image(image_path).await?;

    if is_valid {
        info!("Image validation successful");
    } else {
        error!("Image validation failed");
        return Err(crate::error::AutoInstallError::ImageError(
            "Image validation failed".to_string()
        ));
    }

    Ok(())
}

/// List available images
pub async fn list_images_command(
    filter_arch: Option<Architecture>,
    json_output: bool,
) -> Result<()> {
    let manager = ImageManager::new();
    let images = manager.list_images(filter_arch).await?;

    if json_output {
        let json = serde_json::to_string_pretty(&images)?;
        println!("{}", json);
    } else {
        if images.is_empty() {
            info!("No images found");
            return Ok(());
        }

        println!("Available Images:");
        println!("{:<36} {:<12} {:<8} {:<12} {:<20}", "ID", "Version", "Arch", "Size", "Created");
        println!("{:-<88}", "");

        for image in &images {
            println!("{:<36} {:<12} {:<8} {:<12} {:<20}",
                image.id,
                image.ubuntu_version,
                image.architecture.as_str(),
                image.size_human(),
                image.created_at.format("%Y-%m-%d %H:%M")
            );
        }

        info!("Found {} images", images.len());
    }

    Ok(())
}

/// Cleanup old images
pub async fn cleanup_command(older_than_days: u32, dry_run: bool) -> Result<()> {
    info!("Cleaning up images older than {} days", older_than_days);

    let manager = ImageManager::new();
    let old_images = manager.find_old_images(older_than_days).await?;

    if old_images.is_empty() {
        info!("No old images found for cleanup");
        return Ok(());
    }

    if dry_run {
        info!("DRY RUN: Would delete {} old images:", old_images.len());
        for image in &old_images {
            info!("  {} - {} ({}) - {}", 
                  image.id, image.ubuntu_version, 
                  image.architecture.as_str(), image.size_human());
        }
        return Ok(());
    }

    let deleted_count = manager.cleanup_images(old_images).await?;
    info!("Successfully deleted {} old images", deleted_count);

    Ok(())
}