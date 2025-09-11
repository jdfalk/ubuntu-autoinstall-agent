#!/bin/bash
# file: build-on-linux.sh
# version: 1.0.0
# guid: z1y2x3w4-v5u6-7890-1234-567890zyxwvu

# Ubuntu AutoInstall Agent - Linux Build Script
# This script builds the Ubuntu AutoInstall Agent on a Linux server

set -euo pipefail

echo "🔧 Building Ubuntu AutoInstall Agent on Linux..."

# Check if we're on Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo "❌ This script must be run on Linux"
    exit 1
fi

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "📦 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
fi

# Install system dependencies
echo "📦 Installing system dependencies..."
if command -v apt &> /dev/null; then
    sudo apt update
    sudo apt install -y build-essential pkg-config libssl-dev
elif command -v dnf &> /dev/null; then
    sudo dnf install -y gcc pkgconf openssl-devel
elif command -v yum &> /dev/null; then
    sudo yum install -y gcc pkgconfig openssl-devel
else
    echo "⚠️  Please install build tools manually (gcc, pkg-config, openssl-dev)"
fi

# Build the project
echo "🏗️  Building project..."
cargo build --release

echo "✅ Build completed!"
echo "📍 Binary location: target/release/ubuntu-autoinstall-agent"

# Make the binary executable
chmod +x target/release/ubuntu-autoinstall-agent

# Show build info
echo ""
echo "🔍 Build Information:"
ls -la target/release/ubuntu-autoinstall-agent
file target/release/ubuntu-autoinstall-agent

echo ""
echo "🎉 Ubuntu AutoInstall Agent is ready!"
echo "Run './target/release/ubuntu-autoinstall-agent --help' to get started"
