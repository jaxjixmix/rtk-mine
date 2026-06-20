#!/usr/bin/env bash
# rtk-mine — one-command installer
# 
# This script detects your OS/arch, downloads the appropriate binary,
# and installs it to /usr/local/bin (or ~/.local/bin).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/user/rtk-mine/main/install.sh | bash
#
# Or locally:
#   ./install.sh

set -euo pipefail

BOLD="\033[1m"
GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

REPO="user/rtk-mine"
BINARY="rtk-mine"
INSTALL_DIR="${RTK_MINE_INSTALL_DIR:-/usr/local/bin}"

info()  { echo -e "${GREEN}→${RESET} $*"; }
warn()  { echo -e "${YELLOW}⚠${RESET} $*"; }
err()   { echo -e "${RED}✗${RESET} $*" >&2; }
header(){ echo -e "\n${BOLD}$*${RESET}\n"; }

header "rtk-mine installer"

# Detect OS and architecture.
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux|darwin) ;;
    *) err "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)  ARCH="aarch64" ;;
    *) err "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"
info "Detected platform: $TARGET"

# --- Check if we can build from source (preferred, since this is a Rust project) ---
if command -v cargo &>/dev/null; then
    info "Cargo detected — building from source..."
    
    TMPDIR=$(mktemp -d)
    trap "rm -rf $TMPDIR" EXIT
    
    # Clone and build.
    git clone --depth 1 "https://github.com/${REPO}.git" "$TMPDIR" 2>/dev/null || {
        warn "Cannot clone from GitHub. Building from current directory..."
        TMPDIR="$(pwd)"
    }
    
    cd "$TMPDIR"
    cargo build --release 2>&1 | tail -3
    
    BINARY_PATH="$TMPDIR/target/release/$BINARY"
else
    info "Cargo not found — downloading pre-built binary..."
    
    # Try GitHub releases.
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)".*/\1/' || echo "v0.1.0")
    
    TARBALL="${BINARY}-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${TARBALL}"
    
    TMPDIR=$(mktemp -d)
    trap "rm -rf $TMPDIR" EXIT
    
    info "Downloading $URL..."
    curl -fsSL "$URL" -o "$TMPDIR/$TARBALL" || {
        err "Download failed. Please install Rust and build from source:"
        err "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        err "  cargo install --git https://github.com/${REPO}.git"
        exit 1
    }
    
    tar xzf "$TMPDIR/$TARBALL" -C "$TMPDIR"
    BINARY_PATH="$TMPDIR/$BINARY"
fi

# Install binary.
if [ ! -f "$BINARY_PATH" ]; then
    err "Binary not found at $BINARY_PATH"
    exit 1
fi

if [ -w "$INSTALL_DIR" ]; then
    cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY"
else
    info "Need sudo to install to $INSTALL_DIR"
    sudo cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY"
fi

chmod +x "$INSTALL_DIR/$BINARY"

info "Installed $BINARY to $INSTALL_DIR/$BINARY"

# Verify.
"$INSTALL_DIR/$BINARY" version

# Initialize config.
info "Initializing default configuration..."
"$INSTALL_DIR/$BINARY" config init

# Shell setup hint.
header "Setup complete!"
echo ""
echo "  Add this to your shell config (~/.zshrc, ~/.bashrc, or ~/.config/fish/config.fish):"
echo ""
echo -e "    ${BOLD}eval \"\$($BINARY init)\"${RESET}"
echo ""
echo "  Or run it directly to test:"
echo ""
echo "    eval \"\$($BINARY init)\""
echo "    ls -la          # proxied through rtk-mine"
echo "    rtk-mine audit  # see what happened"
echo ""
echo "  Quick commands:"
echo "    rtk-mine exec -- <command>   # one-off proxy"
echo "    rtk-mine audit               # recent audit log"
echo "    rtk-mine audit stats         # savings dashboard"
echo "    rtk-mine config show         # current config"
echo ""
