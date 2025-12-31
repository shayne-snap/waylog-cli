#!/bin/bash
# WayLog CLI Installation Script

set -e

echo "🚀 Installing WayLog CLI..."

# Detect OS
OS="$(uname -s)"
case "${OS}" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    *)          MACHINE="UNKNOWN:${OS}"
esac

if [ "$MACHINE" = "UNKNOWN:${OS}" ]; then
    echo "❌ Unsupported operating system: ${OS}"
    exit 1
fi

echo "📦 Detected OS: $MACHINE"

# Build release binary
echo "🔨 Building release binary..."
cargo build --release

# Install location
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# Copy binary
echo "📋 Installing to $INSTALL_DIR/waylog..."
# Remove existing binary first to avoid macOS code signing issues
if [ -f "$INSTALL_DIR/waylog" ]; then
    rm "$INSTALL_DIR/waylog"
fi
cp target/release/waylog "$INSTALL_DIR/waylog"
chmod +x "$INSTALL_DIR/waylog"

# Check if in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  $INSTALL_DIR is not in your PATH"
    echo ""
    echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    echo ""
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
fi

echo ""
echo "✅ Installation complete!"
echo ""
echo "Usage:"
echo "  waylog run claude    # Run Claude Code with auto-sync"
echo "  waylog run gemini    # Run Gemini CLI with auto-sync"
echo ""
echo "For more information, run: waylog --help"
