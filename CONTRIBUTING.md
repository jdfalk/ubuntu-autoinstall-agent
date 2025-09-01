# Contributing to Ubuntu AutoInstall Agent

<!-- file: CONTRIBUTING.md -->
<!-- version: 1.0.0 -->
<!-- guid: f7g8h9i0-j1k2-3456-7890-123456fghijk -->

Thank you for your interest in contributing to the Ubuntu AutoInstall Agent! This document provides guidelines and information for contributors.

## Development Environment

### Prerequisites

- **Rust**: Version 1.70 or later
- **QEMU/KVM**: For image creation and testing
- **Docker**: For containerized testing
- **SSH Access**: To test target deployments

### Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jdfalk/ubuntu-autoinstall-agent.git
   cd ubuntu-autoinstall-agent
   ```

2. **Install Rust dependencies**:
   ```bash
   rustup update
   cargo build
   ```

3. **Install system dependencies**:
   ```bash
   # Ubuntu/Debian
   sudo apt install qemu-kvm libvirt-daemon-system libvirt-clients

   # Add user to libvirt group
   sudo usermod -a -G libvirt $USER
   ```

4. **Verify setup**:
   ```bash
   cargo test
   cargo run -- --help
   ```

## Project Structure

```
ubuntu-autoinstall-agent/
├── src/                          # Source code
│   ├── main.rs                   # CLI entry point
│   ├── cli/                      # Command-line interface
│   ├── config/                   # Configuration management
│   ├── image/                    # Image management
│   │   ├── builder.rs            # Image creation
│   │   ├── deployer.rs           # Image deployment
│   │   ├── customizer.rs         # Target customization
│   │   └── manager.rs            # Image lifecycle
│   ├── security/                 # Security and encryption
│   ├── network/                  # Network operations
│   ├── utils/                    # Utilities and helpers
│   ├── logging/                  # Logging system
│   └── error/                    # Error types and handling
├── examples/                     # Example configurations
│   ├── configs/                  # Target configurations
│   └── specs/                    # Image specifications
├── tests/                        # Integration tests
├── docs/                         # Documentation
└── scripts/                      # Build and utility scripts
```

## Coding Standards

### Rust Guidelines

1. **Follow Rust conventions**:
   - Use `snake_case` for functions and variables
   - Use `PascalCase` for types and enums
   - Use `SCREAMING_SNAKE_CASE` for constants

2. **Error handling**:
   - Use `anyhow::Result` for main error types
   - Create specific error types for different modules
   - Always handle errors gracefully

3. **Documentation**:
   - Document all public APIs with `///` comments
   - Include examples in documentation
   - Use `#[doc(hidden)]` for internal APIs

4. **Testing**:
   - Write unit tests for all modules
   - Include integration tests for complete workflows
   - Use property-based testing where appropriate

### Code Style

We use standard Rust formatting with `rustfmt`:

```bash
# Format all code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

Linting with `clippy`:

```bash
# Run linter
cargo clippy

# Run with all features
cargo clippy --all-features
```

### Example Code Structure

```rust
//! Module documentation
//!
//! This module handles [specific functionality].

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Public struct documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleStruct {
    /// Field documentation
    pub field: String,
}

impl ExampleStruct {
    /// Constructor documentation
    ///
    /// # Arguments
    ///
    /// * `field` - Description of the field
    ///
    /// # Examples
    ///
    /// ```
    /// let example = ExampleStruct::new("value".to_string());
    /// ```
    pub fn new(field: String) -> Self {
        Self { field }
    }

    /// Method documentation
    pub async fn do_something(&self) -> Result<()> {
        // Implementation
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let example = ExampleStruct::new("test".to_string());
        assert_eq!(example.field, "test");
    }

    #[tokio::test]
    async fn test_do_something() {
        let example = ExampleStruct::new("test".to_string());
        assert!(example.do_something().await.is_ok());
    }
}
```

## Development Workflow

### 1. Planning

Before starting development:

1. **Check existing issues** for related work
2. **Create an issue** describing the feature/bug
3. **Discuss approach** in the issue comments
4. **Get approval** from maintainers for major changes

### 2. Development

1. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Write tests first** (TDD approach):
   ```bash
   # Write failing tests
   cargo test  # Should fail

   # Implement feature
   # Run tests until they pass
   cargo test
   ```

3. **Follow the development cycle**:
   - Write tests
   - Implement functionality
   - Run all tests
   - Update documentation
   - Test manually

### 3. Testing

#### Unit Tests

Run unit tests for specific modules:

```bash
# All tests
cargo test

# Specific module
cargo test config::tests

# With output
cargo test -- --nocapture
```

#### Integration Tests

Test complete workflows:

```bash
# Run integration tests
cargo test --test integration

# Test specific scenario
cargo test --test integration test_full_deployment
```

#### Manual Testing

Test with actual VMs:

```bash
# Create test image
cargo run -- create-image --arch amd64 --output ./test-images/

# Test deployment to VM
cargo run -- deploy --target test-vm --config examples/configs/test.yaml
```

### 4. Documentation

Update documentation for changes:

1. **Code documentation**: Update `///` comments
2. **README.md**: Update usage examples if needed
3. **CHANGELOG.md**: Add entry for changes
4. **API docs**: Run `cargo doc` to verify

### 5. Pull Request

1. **Push your branch**:
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create pull request** with:
   - Clear title and description
   - Reference to related issues
   - Testing performed
   - Breaking changes (if any)

3. **Address review feedback**
4. **Squash commits** if requested

## Module Development Guidelines

### CLI Module (`src/cli/`)

- Use `clap` for argument parsing
- Validate inputs early
- Provide helpful error messages
- Support both interactive and non-interactive modes

```rust
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ubuntu-autoinstall-agent")]
#[command(about = "Ubuntu AutoInstall deployment tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    CreateImage(CreateImageArgs),
    Deploy(DeployArgs),
}
```

### Configuration Module (`src/config/`)

- Use `serde` for serialization
- Validate configurations on load
- Support environment variable substitution
- Provide clear error messages for invalid configs

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetConfig {
    pub hostname: String,
    pub architecture: Architecture,
    pub network: NetworkConfig,
    pub luks_config: LuksConfig,
}

impl TargetConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validation logic
        Ok(())
    }
}
```

### Image Module (`src/image/`)

- Handle VM creation and management
- Implement proper cleanup on errors
- Support multiple architectures
- Provide progress feedback

### Security Module (`src/security/`)

- Never log sensitive data
- Use secure random generation
- Validate all cryptographic operations
- Follow security best practices

### Network Module (`src/network/`)

- Handle timeouts and retries
- Validate certificates
- Support progress tracking
- Handle network errors gracefully

## Testing Guidelines

### Unit Tests

- Test all public APIs
- Test error conditions
- Use dependency injection for mocking
- Test edge cases and boundary conditions

### Integration Tests

- Test complete workflows
- Use temporary directories and files
- Clean up after tests
- Test with real VMs when possible

### Property-Based Testing

Use `proptest` for complex property testing:

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_config_roundtrip(config in config_strategy()) {
            let yaml = serde_yaml::to_string(&config)?;
            let parsed: TargetConfig = serde_yaml::from_str(&yaml)?;
            prop_assert_eq!(config, parsed);
        }
    }
}
```

## Performance Guidelines

### Async Operations

- Use `tokio` for async runtime
- Make I/O operations async
- Use connection pooling where appropriate
- Handle cancellation properly

### Memory Management

- Avoid unnecessary allocations
- Use streaming for large files
- Clean up resources promptly
- Monitor memory usage in tests

### Disk I/O

- Use async file operations
- Implement proper buffering
- Handle large files efficiently
- Provide progress feedback

## Security Guidelines

### Sensitive Data

- Never log passwords or keys
- Use environment variables for secrets
- Clear sensitive data from memory
- Validate all inputs

### Cryptographic Operations

- Use well-established libraries
- Follow current best practices
- Handle errors securely
- Support key rotation

### Network Security

- Validate all certificates
- Use HTTPS for all downloads
- Implement proper timeouts
- Handle authentication securely

## Release Process

### Version Management

We use semantic versioning (SemVer):

- **MAJOR**: Breaking changes
- **MINOR**: New features, backwards compatible
- **PATCH**: Bug fixes, backwards compatible

### Release Checklist

1. **Update version** in `Cargo.toml`
2. **Update CHANGELOG.md** with new version
3. **Run full test suite**:
   ```bash
   cargo test --all-features
   cargo test --release
   ```
4. **Build release binaries**:
   ```bash
   ./build-cross-platform.sh
   ```
5. **Test release binaries** on different platforms
6. **Create git tag**:
   ```bash
   git tag -a v1.0.0 -m "Release version 1.0.0"
   git push origin v1.0.0
   ```
7. **Publish to GitHub releases**

## Common Issues and Solutions

### QEMU/KVM Problems

```bash
# Check virtualization support
egrep -c '(vmx|svm)' /proc/cpuinfo

# Check libvirt service
sudo systemctl status libvirtd

# Check user permissions
groups $USER  # Should include 'libvirt'
```

### Build Issues

```bash
# Clean build
cargo clean
cargo build

# Update dependencies
cargo update

# Check for conflicts
cargo tree
```

### Test Failures

```bash
# Run specific test with output
cargo test test_name -- --nocapture

# Run tests serially
cargo test -- --test-threads=1

# Run ignored tests
cargo test -- --ignored
```

## Getting Help

- **GitHub Issues**: For bugs and feature requests
- **GitHub Discussions**: For questions and design discussions
- **Code Review**: Comment on pull requests
- **Documentation**: Check the `docs/` directory

## Recognition

Contributors will be recognized in:

- **CHANGELOG.md**: For significant contributions
- **GitHub contributors page**: Automatic recognition
- **Release notes**: For major features

Thank you for contributing to the Ubuntu AutoInstall Agent!
