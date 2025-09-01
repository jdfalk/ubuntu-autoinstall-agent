# Ubuntu AutoInstall Agent - Implementation Guide

<!-- file: IMPLEMENTATION.md -->
<!-- version: 1.0.0 -->
<!-- guid: b2c3d4e5-f6g7-8901-2345-678901bcdefg -->

## Coding Agent Implementation Instructions

This document provides step-by-step implementation instructions for the Ubuntu AutoInstall Agent. Follow this guide to build a complete, working system.

## Overview

**Primary Goal**: Replace complex shell-based Ubuntu autoinstall scripts with a reliable Rust application that uses golden image deployment.

**Key Requirements**:
- Zero manual intervention
- Support both amd64 and arm64 architectures
- Full disk encryption (LUKS) by default
- Golden image approach (build once, deploy many times)
- Netboot/PXE ready
- Remove all unnecessary features from copilot-agent-util base

## Step 1: Project Structure Setup

Create the following directory structure:

```
src/
├── main.rs                 # CLI entry point
├── lib.rs                  # Library root
├── cli/
│   ├── mod.rs             # CLI module root
│   ├── commands.rs        # Command definitions
│   └── args.rs            # Argument parsing
├── config/
│   ├── mod.rs             # Configuration module root
│   ├── target.rs          # Target machine config
│   ├── image.rs           # Image specification config
│   └── loader.rs          # Configuration file loading
├── image/
│   ├── mod.rs             # Image module root
│   ├── builder.rs         # Golden image creation
│   ├── deployer.rs        # Image deployment
│   ├── customizer.rs      # Target-specific customization
│   └── manager.rs         # Image lifecycle management
├── security/
│   ├── mod.rs             # Security module root
│   ├── luks.rs            # LUKS operations
│   └── validation.rs     # Input validation
├── network/
│   ├── mod.rs             # Network module root
│   ├── download.rs        # File download operations
│   └── ssh.rs             # SSH deployment
├── utils/
│   ├── mod.rs             # Utilities module root
│   ├── disk.rs            # Disk operations
│   ├── system.rs          # System utilities
│   └── vm.rs              # VM management
├── logging/
│   ├── mod.rs             # Logging module root
│   └── logger.rs          # Logger implementation
└── error/
    ├── mod.rs             # Error module root
    └── types.rs           # Error type definitions
```

## Step 2: Cargo.toml Configuration

Create this exact Cargo.toml:

```toml
[package]
name = "ubuntu-autoinstall-agent"
version = "0.1.0"
edition = "2021"
description = "Automated Ubuntu server deployment with golden images and LUKS encryption"
license = "MIT"
repository = "https://github.com/jdfalk/ubuntu-autoinstall-agent"

[dependencies]
# CLI Framework
clap = { version = "4.4", features = ["derive"] }

# Async Runtime
tokio = { version = "1.0", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Network Operations
reqwest = { version = "0.11", features = ["stream", "json"] }
ssh2 = "0.9"

# Cryptography
ring = "0.17"
sha2 = "0.10"

# File Operations
tempfile = "3.8"
walkdir = "2.4"

# System Operations
nix = "0.27"

# Progress Indicators
indicatif = "0.17"

# UUID Generation
uuid = { version = "1.6", features = ["v4"] }

# Futures and async utilities
futures = "0.3"

# Command execution
process = "0.4"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3.8"
```

## Step 3: Core Type Definitions

### Architecture Enum (`src/config/mod.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    #[serde(rename = "amd64")]
    Amd64,
    #[serde(rename = "arm64")]
    Arm64,
}

impl Architecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        }
    }

    pub fn qemu_arch(&self) -> &'static str {
        match self {
            Architecture::Amd64 => "x86_64",
            Architecture::Arm64 => "aarch64",
        }
    }
}
```

## Step 4: Error Handling System

Create comprehensive error types (`src/error/types.rs`):

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutoInstallError {
    #[error("VM operation failed: {0}")]
    VmError(String),

    #[error("Disk operation failed: {0}")]
    DiskError(String),

    #[error("Network operation failed: {0}")]
    NetworkError(String),

    #[error("LUKS operation failed: {0}")]
    LuksError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Image operation failed: {0}")]
    ImageError(String),

    #[error("SSH operation failed: {0}")]
    SshError(String),

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_yaml::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, AutoInstallError>;
```

## Step 5: Configuration Structures

### Target Configuration (`src/config/target.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::Architecture;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub hostname: String,
    pub architecture: Architecture,
    pub disk_device: String,
    pub timezone: String,
    pub network: NetworkConfig,
    pub users: Vec<UserConfig>,
    pub luks_config: LuksConfig,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interface: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub dns_servers: Vec<String>,
    pub dhcp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub name: String,
    pub sudo: bool,
    pub ssh_keys: Vec<String>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuksConfig {
    pub passphrase: String,
    pub cipher: String,
    pub key_size: u32,
    pub hash: String,
}
```

### Image Specification (`src/config/image.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::Architecture;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSpec {
    pub ubuntu_version: String,
    pub architecture: Architecture,
    pub base_packages: Vec<String>,
    pub custom_scripts: Vec<PathBuf>,
    pub vm_config: VmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub memory_mb: u32,
    pub disk_size_gb: u32,
    pub cpu_cores: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub ubuntu_version: String,
    pub architecture: Architecture,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
    pub checksum: String,
    pub path: PathBuf,
}
```

## Step 6: CLI Implementation

### Main CLI (`src/main.rs`)

```rust
use clap::{Parser, Subcommand};
use ubuntu_autoinstall_agent::{
    cli::commands::*,
    config::Architecture,
    error::Result,
    logging::logger,
};

#[derive(Parser)]
#[command(name = "ubuntu-autoinstall-agent")]
#[command(about = "Automated Ubuntu server deployment with golden images")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a golden Ubuntu image
    CreateImage {
        #[arg(short, long, default_value = "amd64")]
        arch: Architecture,

        #[arg(short, long, default_value = "24.04")]
        version: String,

        #[arg(short, long)]
        output: Option<String>,
    },

    /// Deploy image to target machine
    Deploy {
        #[arg(short, long)]
        target: String,

        #[arg(short, long)]
        config: String,

        #[arg(long)]
        via_ssh: bool,
    },

    /// Validate image integrity
    Validate {
        #[arg(short, long)]
        image: String,
    },

    /// List available images
    ListImages,

    /// Cleanup old images
    Cleanup {
        #[arg(long, default_value = "30")]
        older_than_days: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    logger::init_logger(cli.verbose)?;

    // Execute command
    match cli.command {
        Commands::CreateImage { arch, version, output } => {
            create_image_command(arch, &version, output).await
        }
        Commands::Deploy { target, config, via_ssh } => {
            deploy_command(&target, &config, via_ssh).await
        }
        Commands::Validate { image } => {
            validate_command(&image).await
        }
        Commands::ListImages => {
            list_images_command().await
        }
        Commands::Cleanup { older_than_days } => {
            cleanup_command(older_than_days).await
        }
    }
}
```

## Step 7: Key Implementation Guidelines

### DO NOT Include These Features

**Remove all traces of copilot-agent-util functionality**:
- ❌ No `buf` command processing
- ❌ No protocol buffer operations
- ❌ No git operations
- ❌ No linting or formatting tools
- ❌ No generic command filtering
- ❌ No copilot utility features

**Focus ONLY on autoinstall functionality**:
- ✅ Image creation and management
- ✅ LUKS disk encryption
- ✅ Ubuntu deployment
- ✅ Network operations for deployment
- ✅ Configuration management

### Critical Implementation Notes

1. **VM Management**: Use QEMU/KVM for creating golden images
2. **Error Handling**: Every operation must have proper error handling with context
3. **Logging**: Comprehensive logging for all operations
4. **Security**: Never log sensitive data (passwords, keys)
5. **Validation**: Validate all user inputs and configurations
6. **Async Operations**: Use tokio for all I/O operations
7. **Progress Indicators**: Show progress for long-running operations

### Required Operations

1. **Image Builder**:
   - Create VM with specified architecture
   - Install Ubuntu from ISO
   - Apply standard configuration
   - Generalize system (remove machine-specific data)
   - Create compressed image file

2. **Image Deployer**:
   - Set up LUKS encrypted disk
   - Download and verify image
   - Apply target-specific customization
   - Deploy image to encrypted disk
   - Configure bootloader

3. **Configuration System**:
   - Load YAML configuration files
   - Validate configuration data
   - Support environment variable substitution
   - Handle sensitive data securely

4. **Network Operations**:
   - HTTP downloads with progress
   - SSH connections and file transfers
   - Network configuration setup

## Step 8: Testing Requirements

### Unit Tests
Each module must include tests for:
- Normal operation scenarios
- Error conditions
- Edge cases
- Input validation

### Integration Tests
- Complete image creation workflow
- Complete deployment workflow
- Configuration loading and validation
- Error recovery scenarios

## Step 9: Build Configuration

### Cross-compilation targets
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

### GitHub Actions workflow
Create automated builds for all target architectures with proper artifact generation.

## Step 10: Documentation

### Required Documentation
1. `README.md` - User guide and quick start
2. `DEPLOYMENT.md` - Deployment scenarios and examples
3. `CONFIGURATION.md` - Configuration file reference
4. `TROUBLESHOOTING.md` - Common issues and solutions
5. `API.md` - Library API documentation (if applicable)

### Configuration Examples
Create example configuration files for common scenarios:
- `examples/configs/basic-server.yaml`
- `examples/configs/development-machine.yaml`
- `examples/specs/ubuntu-24.04-minimal.yaml`

## Success Criteria

The implementation is complete when:

1. ✅ Can create golden Ubuntu images for both amd64 and arm64
2. ✅ Can deploy images to target machines with LUKS encryption
3. ✅ Supports both SSH and netboot deployment methods
4. ✅ Has comprehensive error handling and logging
5. ✅ All operations are fully automated (zero manual intervention)
6. ✅ Configuration system is flexible and well-documented
7. ✅ Includes comprehensive test suite
8. ✅ Binary builds successfully for all target architectures
9. ✅ Deployment is 10x faster than current shell-based approach
10. ✅ All copilot-agent-util legacy code has been removed

## Priority Order

Implement in this exact order:

1. **Week 1**: Error handling, logging, configuration system
2. **Week 2**: VM management and image builder
3. **Week 3**: LUKS operations and image deployer
4. **Week 4**: Network operations and SSH deployment
5. **Week 5**: CLI interface and command implementation
6. **Week 6**: Testing, documentation, and optimization

Follow this implementation guide precisely to create a robust, maintainable Ubuntu autoinstall system that meets all specified requirements.
