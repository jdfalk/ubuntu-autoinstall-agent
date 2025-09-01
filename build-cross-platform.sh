#!/bin/bash
# file: tools/copilot-agent-util-rust/build-cross-platform.sh
# version: 1.0.0
# guid: a1b2c3d4-e5f6-7890-abcd-ef1234567890

# Build cross-platform binaries for copilot-agent-util

set -e

echo "Building cross-platform binaries for copilot-agent-util..."

# Create dist directory
mkdir -p dist

# Build for macOS ARM (M1/M2)
echo "Building for macOS ARM64..."
cargo build --release --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/copilot-agent-util dist/copilot-agent-util-macos-arm64

# Build for macOS Intel
echo "Building for macOS x86_64..."
cargo build --release --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/copilot-agent-util dist/copilot-agent-util-macos-x86_64

# Build for Linux x86_64 (statically linked with MUSL)
echo "Building for Linux x86_64 (static)..."
cargo build --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/copilot-agent-util dist/copilot-agent-util-linux-x86_64

# Build for Linux ARM64 (if cross-compilation tools are available)
if rustup target list --installed | grep -q "aarch64-unknown-linux-musl"; then
    echo "Building for Linux ARM64 (static)..."
    cargo build --release --target aarch64-unknown-linux-musl
    cp target/aarch64-unknown-linux-musl/release/copilot-agent-util dist/copilot-agent-util-linux-arm64
else
    echo "Skipping Linux ARM64 build (target not installed)"
fi

echo "Cross-platform build complete!"
echo "Binaries available in dist/:"
ls -la dist/
