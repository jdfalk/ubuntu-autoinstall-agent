<!-- file: CODING_AGENT_INSTRUCTIONS.md -->
<!-- version: 1.0.0 -->
<!-- guid: s3t4u5v6-w7x8-9012-3456-789012stuvwx -->

# Coding Agent Implementation Instructions

## Critical Requirements

This project is **ONLY** for Ubuntu autoinstall automation. Do NOT include any functionality from the copilot utility project.

### What This System Does

- Creates golden Ubuntu images in VMs
- Deploys images with LUKS full disk encryption
- Fully automated operation (zero manual intervention)
- Supports SSH and netboot deployment

### What This System Does NOT Do

- Protocol buffer operations (remove buf.yaml, buf.gen.yaml if they exist)
- Generic utility functions unrelated to Ubuntu deployment
- Copilot-specific logging or formatting
- Any dependency on other repositories

## Implementation Order

1. **Start with Foundation** (Phase 1 from IMPLEMENTATION.md)
   - Create Cargo.toml with ONLY the dependencies listed
   - Set up basic project structure exactly as specified
   - Create error types in src/error.rs

2. **CLI Interface** (Phase 2)
   - Implement clap-based CLI with the exact commands shown
   - Focus on: create-image, deploy-image, list-images
   - Keep it simple and focused

3. **Configuration System** (Phase 3)
   - Implement YAML loading for target configs and image specs
   - Use the examples/ directory as reference
   - Support environment variable substitution

4. **Core Functionality** (Phases 4-6)
   - VM creation and management with QEMU
   - Image building and generalization
   - LUKS encryption deployment
   - SSH-based deployment

## Critical Success Criteria

- **Zero compilation errors** on first attempt
- **All tests pass** including integration tests
- **Examples work** without modification
- **No manual intervention** required for normal operation

## Security Requirements

- LUKS encryption enabled by default
- SSH key authentication only
- Secure file permissions (600 for keys, 644 for configs)
- Input validation on all user-provided data

## Performance Requirements

- Parallel image creation for different architectures
- Efficient disk image compression
- Fast deployment over SSH
- Minimal resource usage during operation

## Documentation Requirements

- Update README.md with usage examples
- Include troubleshooting section
- Document all CLI commands and options
- Provide deployment workflow diagrams

## Quality Assurance

- Follow the exact module structure in DESIGN.md
- Use the error handling patterns specified
- Include comprehensive logging with different levels
- Write unit tests for all public functions
- Create integration tests for end-to-end workflows

## Final Validation

Before considering implementation complete:

1. Can create Ubuntu images without user interaction
2. Can deploy with LUKS encryption via SSH
3. Can handle both amd64 and arm64 architectures
4. All example configurations work correctly
5. Error messages are clear and actionable

Remember: This is an Ubuntu autoinstall system, not a general-purpose utility. Keep the scope focused and avoid feature creep.
