# Ubuntu AutoInstall Agent - Complete Design Document

<!-- file: DESIGN.md -->
<!-- version: 1.0.0 -->
<!-- guid: a1b2c3d4-e5f6-7890-1234-567890abcdef -->

## Project Overview

The Ubuntu AutoInstall Agent is a Rust-based utility that automates the deployment of Ubuntu servers using a golden image approach. This system replaces complex shell-based debootstrap installations with a reliable, fast, and consistent image-based deployment system.

### Background and Motivation

**Current Pain Points with Shell-Based Approach:**
- Complex 500+ line shell scripts (`jinstall.sh`) prone to failures
- Manual `debootstrap` operations with inconsistent results  
- Embedded installation logic in YAML user-data files
- No proper error recovery or rollback mechanisms
- Difficult to test and validate before production deployment
- Architecture-specific scripts that don't scale
- Manual intervention required for troubleshooting failures

**Problems with Current Implementation:**
1. **Inline Script Complexity**: Critical installation logic embedded in cloud-init user-data YAML files
2. **Debootstrap Reliability**: Manual filesystem bootstrap operations that fail unpredictably
3. **No Status Reporting**: Installations fail silently with no feedback mechanism
4. **Manual Recovery**: Failed installations require manual intervention and restart
5. **Testing Difficulty**: Cannot validate installations without deploying to actual hardware
6. **Legacy Code**: Inherited copilot-agent-util features not needed for autoinstall

**Enterprise Requirements:**
- Deploy latest Ubuntu (24.04+) on both amd64 and arm64 architectures
- Full disk encryption (LUKS) mandatory for security compliance
- Netboot/PXE deployment for bare metal servers
- Zero-touch installation process
- Automated status reporting and monitoring
- Rapid deployment (< 10 minutes vs current 30+ minutes)
- Consistent, reproducible results across all deployments

## Core Objectives

1. **Fully Automated**: Zero manual intervention required for deployments
2. **Cross-Architecture Support**: Build and deploy for both amd64 and arm64
3. **Golden Image Approach**: Create standard images once, deploy many times
4. **Security-First**: Full disk encryption (LUKS) by default
5. **Network Boot Ready**: Designed for PXE/netboot environments
6. **Modular Design**: Easy to extend and customize
7. **Robust Error Handling**: Graceful failure recovery and detailed logging
8. **Standard Utilities**: Include reliable, secure utility packages

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Ubuntu AutoInstall Agent                    │
├─────────────────────────────────────────────────────────────────┤
│  CLI Interface (main.rs)                                       │
├─────────────────────────────────────────────────────────────────┤
│  Core Modules:                                                 │
│  ├── Image Management (golden image creation & deployment)     │
│  ├── Configuration Management (target-specific configs)        │
│  ├── Security Layer (LUKS, encryption, validation)            │
│  ├── Network Operations (downloads, SSH deployment)            │
│  ├── Utilities (disk, crypto, system operations)              │
│  └── Logging & Error Handling (comprehensive tracking)        │
└─────────────────────────────────────────────────────────────────┘
```

## High-Level Workflow

### Phase 1: Golden Image Creation
```
VM Creation → Ubuntu Installation → Standard Configuration → 
Generalization (OEM Mode) → Image Capture → Compression → Storage
```

### Phase 2: Target Deployment
```
Netboot → LUKS Disk Setup → Image Download → Target Customization → 
Image Deployment → Bootloader Configuration → Validation
```

## Module Specifications

### 1. CLI Interface (`src/main.rs`)

**Purpose**: Main entry point with command-line interface

**Commands**:
- `create-image [--arch amd64|arm64] [--version 24.04]` - Create golden image
- `deploy [--target hostname] [--config path]` - Deploy to target machine
- `validate [--image path]` - Validate image integrity
- `list-images` - List available images
- `cleanup [--older-than days]` - Clean up old images

**Example Usage**:
```bash
# Create golden images
ubuntu-autoinstall-agent create-image --arch amd64 --version 24.04
ubuntu-autoinstall-agent create-image --arch arm64 --version 24.04

# Deploy to target
ubuntu-autoinstall-agent deploy --target len-serv-003 --config configs/len-serv-003.yaml

# Validate image
ubuntu-autoinstall-agent validate --image images/ubuntu-24.04-amd64.img
```

### 2. Image Management Module (`src/image/`)

#### 2.1 Builder (`src/image/builder.rs`)

**Purpose**: Create golden Ubuntu images using VMs

**Key Functions**:
- `create_vm(arch: Architecture) -> Result<VirtualMachine>`
- `install_ubuntu(vm: &VirtualMachine, version: &str) -> Result<()>`
- `apply_standard_config(vm: &VirtualMachine) -> Result<()>`
- `generalize_system(vm: &VirtualMachine) -> Result<()>`
- `create_image(vm: &VirtualMachine) -> Result<PathBuf>`

**Implementation Notes**:
- Use QEMU/KVM for VM creation
- Support both amd64 and arm64 architectures
- Apply standard package installation (openssh-server, utilities, etc.)
- Run sysprep/generalization to remove machine-specific data
- Create compressed images (qcow2 or raw with compression)

#### 2.2 Deployer (`src/image/deployer.rs`)

**Purpose**: Deploy images to target machines

**Key Functions**:
- `setup_luks_disk(target: &TargetConfig) -> Result<LuksDevice>`
- `download_image(arch: Architecture, version: &str) -> Result<PathBuf>`
- `verify_image_integrity(image_path: &Path) -> Result<()>`
- `deploy_to_luks(image: &Path, luks_device: &LuksDevice) -> Result<()>`
- `configure_bootloader(target: &TargetConfig) -> Result<()>`

#### 2.3 Customizer (`src/image/customizer.rs`)

**Purpose**: Apply target-specific customizations to images

**Key Functions**:
- `customize_hostname(image: &Path, hostname: &str) -> Result<()>`
- `setup_networking(image: &Path, network_config: &NetworkConfig) -> Result<()>`
- `install_ssh_keys(image: &Path, keys: &[SshKey]) -> Result<()>`
- `apply_user_config(image: &Path, users: &[UserConfig]) -> Result<()>`
- `set_timezone(image: &Path, timezone: &str) -> Result<()>`

#### 2.4 Manager (`src/image/manager.rs`)

**Purpose**: Orchestrate the entire image lifecycle

**Key Functions**:
- `build_golden_image(spec: &ImageSpec) -> Result<ImageId>`
- `deploy_image(image_id: &ImageId, target: &TargetConfig) -> Result<DeploymentId>`
- `list_available_images() -> Result<Vec<ImageInfo>>`
- `cleanup_old_images(retention_days: u32) -> Result<CleanupReport>`

### 3. Configuration Module (`src/config/`)

#### 3.1 Target Configuration (`src/config/target.rs`)

**Purpose**: Define target machine specifications

**Data Structures**:
```rust
#[derive(Debug, Deserialize)]
pub struct TargetConfig {
    pub hostname: String,
    pub architecture: Architecture,
    pub disk_device: String,
    pub network: NetworkConfig,
    pub users: Vec<UserConfig>,
    pub ssh_keys: Vec<SshKey>,
    pub timezone: String,
    pub packages: Vec<String>,
    pub luks_config: LuksConfig,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {
    pub interface: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub dns_servers: Vec<String>,
    pub dhcp: bool,
}

#[derive(Debug, Deserialize)]
pub struct LuksConfig {
    pub passphrase: String,
    pub cipher: String,
    pub key_size: u32,
    pub hash: String,
}
```

#### 3.2 Image Specification (`src/config/image.rs`)

**Purpose**: Define golden image specifications

**Data Structures**:
```rust
#[derive(Debug, Deserialize)]
pub struct ImageSpec {
    pub ubuntu_version: String,
    pub architecture: Architecture,
    pub base_packages: Vec<String>,
    pub custom_scripts: Vec<PathBuf>,
    pub vm_config: VmConfig,
}

#[derive(Debug, Deserialize)]
pub struct VmConfig {
    pub memory_mb: u32,
    pub disk_size_gb: u32,
    pub cpu_cores: u32,
}
```

### 4. Security Module (`src/security/`)

#### 4.1 LUKS Operations (`src/security/luks.rs`)

**Purpose**: Handle full disk encryption setup

**Key Functions**:
- `create_luks_device(device: &str, passphrase: &str) -> Result<LuksDevice>`
- `open_luks_device(device: &str, passphrase: &str) -> Result<LuksDevice>`
- `format_luks_device(device: &LuksDevice) -> Result<()>`
- `close_luks_device(device: &LuksDevice) -> Result<()>`

#### 4.2 Validation (`src/security/validation.rs`)

**Purpose**: Validate configurations and operations

**Key Functions**:
- `validate_target_config(config: &TargetConfig) -> Result<()>`
- `validate_disk_device(device: &str) -> Result<()>`
- `validate_image_checksum(image: &Path, expected: &str) -> Result<()>`
- `validate_ssh_keys(keys: &[SshKey]) -> Result<()>`

### 5. Network Module (`src/network/`)

#### 5.1 Download Manager (`src/network/download.rs`)

**Purpose**: Handle image downloads and transfers

**Key Functions**:
- `download_image(url: &str, dest: &Path) -> Result<()>`
- `download_with_progress(url: &str, dest: &Path) -> Result<()>`
- `verify_download_integrity(file: &Path, checksum: &str) -> Result<()>`

#### 5.2 SSH Deployment (`src/network/ssh.rs`)

**Purpose**: Deploy via SSH to remote machines

**Key Functions**:
- `connect_ssh(host: &str, credentials: &SshCredentials) -> Result<SshSession>`
- `upload_file(session: &SshSession, local: &Path, remote: &Path) -> Result<()>`
- `execute_command(session: &SshSession, command: &str) -> Result<CommandOutput>`
- `deploy_via_ssh(target: &TargetConfig, image: &Path) -> Result<()>`

### 6. Utilities Module (`src/utils/`)

#### 6.1 Disk Operations (`src/utils/disk.rs`)

**Purpose**: Low-level disk operations

**Key Functions**:
- `partition_disk(device: &str, layout: &PartitionLayout) -> Result<()>`
- `format_partition(partition: &str, filesystem: &str) -> Result<()>`
- `mount_partition(partition: &str, mountpoint: &Path) -> Result<()>`
- `unmount_partition(mountpoint: &Path) -> Result<()>`

#### 6.2 System Operations (`src/utils/system.rs`)

**Purpose**: System-level utilities

**Key Functions**:
- `run_command(command: &str, args: &[&str]) -> Result<CommandOutput>`
- `run_command_with_timeout(command: &str, args: &[&str], timeout: Duration) -> Result<CommandOutput>`
- `check_command_exists(command: &str) -> bool`
- `get_system_architecture() -> Architecture`

#### 6.3 VM Management (`src/utils/vm.rs`)

**Purpose**: Virtual machine operations

**Key Functions**:
- `create_qemu_vm(config: &VmConfig) -> Result<VirtualMachine>`
- `start_vm(vm: &VirtualMachine) -> Result<()>`
- `stop_vm(vm: &VirtualMachine) -> Result<()>`
- `create_disk_image(path: &Path, size_gb: u32) -> Result<()>`

### 7. Logging Module (`src/logging/`)

#### 7.1 Logger (`src/logging/logger.rs`)

**Purpose**: Comprehensive logging system

**Key Functions**:
- `init_logger(level: LogLevel, output: LogOutput) -> Result<()>`
- `log_operation_start(operation: &str) -> Result<OperationId>`
- `log_operation_success(op_id: OperationId) -> Result<()>`
- `log_operation_failure(op_id: OperationId, error: &Error) -> Result<()>`

### 8. Error Handling (`src/error/`)

#### 8.1 Error Types (`src/error/mod.rs`)

**Purpose**: Comprehensive error handling

**Error Types**:
```rust
#[derive(Debug, thiserror::Error)]
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
}
```

## Configuration Files

### Target Configuration Example (`configs/len-serv-003.yaml`)
```yaml
hostname: len-serv-003
architecture: amd64
disk_device: /dev/nvme0n1
timezone: America/New_York

network:
  interface: enp0s3
  dhcp: true
  dns_servers:
    - 8.8.8.8
    - 8.8.4.4

users:
  - name: jfalk
    sudo: true
    ssh_keys:
      - "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5..."

luks_config:
  passphrase: "${LUKS_PASSPHRASE}"
  cipher: aes-xts-plain64
  key_size: 512
  hash: sha256

packages:
  - openssh-server
  - htop
  - curl
  - wget
```

### Image Specification Example (`specs/ubuntu-24.04-server.yaml`)
```yaml
ubuntu_version: "24.04"
architecture: amd64

base_packages:
  - openssh-server
  - curl
  - wget
  - htop
  - vim
  - git
  - build-essential

vm_config:
  memory_mb: 2048
  disk_size_gb: 20
  cpu_cores: 2

custom_scripts:
  - scripts/security-hardening.sh
  - scripts/performance-tuning.sh
```

## Dependencies (`Cargo.toml`)

```toml
[package]
name = "ubuntu-autoinstall-agent"
version = "0.1.0"
edition = "2021"

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
reqwest = { version = "0.11", features = ["stream"] }
ssh2 = "0.9"

# Cryptography
ring = "0.17"
sha2 = "0.10"

# File Operations
tempfile = "3.8"
walkdir = "2.4"

# System Operations
nix = "0.27"
libc = "0.2"

# Progress Indicators
indicatif = "0.17"

# UUID Generation
uuid = { version = "1.6", features = ["v4"] }
```

## Build Configuration

### Cross-Compilation Support
- Support for amd64 and arm64 targets
- Use GitHub Actions for automated builds
- Static linking for portable binaries

### Target Architectures
- `x86_64-unknown-linux-gnu` (amd64)
- `aarch64-unknown-linux-gnu` (arm64)
- `x86_64-unknown-linux-musl` (amd64 static)
- `aarch64-unknown-linux-musl` (arm64 static)

## Testing Strategy

### Unit Tests
- Each module must have comprehensive unit tests
- Mock external dependencies (VMs, disk operations)
- Test error handling paths

### Integration Tests
- Test complete workflows in isolated environments
- Use temporary VMs for testing
- Validate image creation and deployment

### End-to-End Tests
- Test on real hardware when possible
- Validate complete netboot deployments
- Performance benchmarking

## Security Considerations

1. **Secrets Management**: Use environment variables for sensitive data
2. **Input Validation**: Validate all user inputs and configurations
3. **Privilege Escalation**: Minimal use of root privileges
4. **Image Integrity**: Checksums and signatures for all images
5. **Network Security**: HTTPS/TLS for all network operations
6. **LUKS Security**: Strong encryption by default

## Performance Requirements

1. **Image Creation**: < 30 minutes for standard Ubuntu image
2. **Image Deployment**: < 10 minutes for 20GB image
3. **Memory Usage**: < 100MB during normal operations
4. **Disk Space**: Configurable image retention policies

## Migration from Shell Scripts

### Current Shell Script Features to Preserve
1. ZFS pool creation and configuration
2. LUKS full disk encryption setup
3. Network configuration automation
4. User and SSH key management
5. Package installation and system configuration

### Features to Remove/Replace
1. Complex debootstrap operations → Golden image approach
2. Manual chroot configurations → Image customization
3. Hardcoded system-specific values → Configuration files
4. Buf build operations (leftover from copilot utility)
5. Manual error handling → Robust Rust error handling

## Deployment Modes

### Mode 1: Local VM Image Creation
```bash
ubuntu-autoinstall-agent create-image --arch amd64 --output ./images/
```

### Mode 2: Remote SSH Deployment
```bash
ubuntu-autoinstall-agent deploy --target 192.168.1.100 --config len-serv-003.yaml --via ssh
```

### Mode 3: Netboot PXE Deployment
```bash
ubuntu-autoinstall-agent deploy --target netboot --config len-serv-003.yaml --pxe-server 192.168.1.1
```

## Future Enhancements

1. **Web UI**: Browser-based management interface
2. **API Server**: REST API for remote management
3. **Clustering**: Multi-machine deployments
4. **Monitoring**: Integration with monitoring systems
5. **Backup/Restore**: Image and configuration backup
6. **Template System**: Reusable deployment templates
7. **Cloud Integration**: AWS, GCP, Azure support

## Success Criteria

1. **Reliability**: 99% successful deployment rate
2. **Speed**: 10x faster than current shell-based approach
3. **Maintainability**: Clear, documented, modular code
4. **Usability**: Simple CLI interface for all operations
5. **Security**: Full disk encryption by default
6. **Portability**: Works on multiple Linux distributions
7. **Scalability**: Handle dozens of concurrent deployments

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)
- Basic project structure
- CLI framework
- Configuration management
- Error handling and logging

### Phase 2: Image Management (Week 3-4)
- VM creation and management
- Golden image creation
- Image storage and retrieval

### Phase 3: Deployment Engine (Week 5-6)
- LUKS disk setup
- Image customization
- Deployment orchestration

### Phase 4: Network Operations (Week 7-8)
- SSH deployment
- Download management
- Netboot integration

### Phase 5: Testing and Validation (Week 9-10)
- Comprehensive test suite
- Performance optimization
- Documentation completion

This design document provides a complete roadmap for implementing a robust, automated Ubuntu deployment system that will replace the current shell-based approach with a reliable, maintainable Rust application.
