#!/usr/bin/env bash
# rtk-mine — one-command installer (binary + shell hooks + config)
#
# Downloads/builds the binary, generates config, and sets up shell
# integration for all coding agents in one shot.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jaxjixmix/rtk-mine/main/install.sh | bash
#
# Or locally:
#   ./install.sh
#
# Options:
#   SKIP_HOOKS=1 ./install.sh   — skip shell hook installation
#   SKIP_CONFIG=1 ./install.sh  — skip config generation

set -euo pipefail

BOLD="\033[1m"
GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

REPO="jaxjixmix/rtk-mine"
BINARY="rtk-mine"
INSTALL_DIR="${RTK_MINE_INSTALL_DIR:-/usr/local/bin}"

info()  { echo -e "${GREEN}→${RESET} $*"; }
warn()  { echo -e "${YELLOW}⚠${RESET} $*"; }
err()   { echo -e "${RED}✗${RESET} $*" >&2; }
header(){ echo -e "\n${BOLD}$*${RESET}\n"; }

# ── Stage 1: Install binary ────────────────────────────────────────

header "rtk-mine installer"

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

if command -v cargo &>/dev/null; then
    info "Cargo detected — building from source..."
    TMPDIR=$(mktemp -d)
    trap "rm -rf $TMPDIR" EXIT
    git clone --depth 1 "https://github.com/${REPO}.git" "$TMPDIR" 2>/dev/null || {
        warn "Cannot clone from GitHub. Building from current directory..."
        TMPDIR="$(pwd)"
    }
    cd "$TMPDIR"
    cargo build --release 2>&1 | tail -3
    BINARY_PATH="$TMPDIR/target/release/$BINARY"
else
    info "Cargo not found — downloading pre-built binary..."
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
"$INSTALL_DIR/$BINARY" --version

# ── Stage 2: Generate config ────────────────────────────────────────

if [ "${SKIP_CONFIG:-0}" != "1" ]; then
    info "Generating default configuration..."
    "$INSTALL_DIR/$BINARY" config init || warn "Config generation skipped (may need manual setup)"
fi

# ── Stage 3: Shell hooks (all agents) ───────────────────────────────

if [ "${SKIP_HOOKS:-0}" = "1" ]; then
    info "Skipping shell hooks (SKIP_HOOKS=1)"
else
    header "Shell integration"
    HOOK='command -v rtk-mine >/dev/null && eval "$(rtk-mine init --agent --quiet)"'

    install_hook() {
        local file="$1"
        local label="$2"
        if [ -f "$file" ] && grep -qF "rtk-mine init" "$file" 2>/dev/null; then
            info "✓ already in $label ($file)"
        else
            echo "$HOOK" >> "$file"
            info "+ added to $label ($file)"
        fi
    }

    mkdir -p "$INSTALL_DIR"  # ensure dir exists for PATH check

    install_hook "$HOME/.zshenv"        "zsh non-interactive (CodeWhale)"
    install_hook "$HOME/.zshrc"         "zsh interactive (OpenCode, Claude Code)"
    [ -f "$HOME/.bashrc" ]       && install_hook "$HOME/.bashrc"       "bash interactive"
    [ -f "$HOME/.bash_profile" ] && install_hook "$HOME/.bash_profile" "bash login"

    # BASH_ENV for bash non-interactive (Copilot, CodeWhale bash)
    if [ ! -f "$HOME/.bashenv" ] || ! grep -qF "rtk-mine init" "$HOME/.bashenv" 2>/dev/null; then
        echo "$HOOK" > "$HOME/.bashenv"
        info "+ created ~/.bashenv with BASH_ENV export"
        for rc in "$HOME/.bashrc" "$HOME/.bash_profile"; do
            if [ -f "$rc" ] && ! grep -qF "BASH_ENV" "$rc" 2>/dev/null; then
                echo "export BASH_ENV=\"$HOME/.bashenv\"" >> "$rc"
            fi
        done
    fi
fi

# ── Stage 4: PATH wrappers (Copilot CLI, agents bypassing shell) ────

if [ "${SKIP_WRAPPERS:-0}" != "1" ]; then
    WRAPPER_DIR="$HOME/.rtk-mine/bin"
    mkdir -p "$WRAPPER_DIR"

    # Master wrapper — uses argv[0] to determine the real command.
    cat > "$WRAPPER_DIR/rtk-wrapper" << 'WRAPPER_EOF'
#!/usr/bin/env bash
# rtk-mine PATH wrapper — intercepts commands for agents that bypass shell functions.
# Usage: symlink this as any command name (e.g., ln -s rtk-wrapper ls).
cmd=$(basename "$0")
if [ "$cmd" = "rtk-wrapper" ]; then
    echo "rtk-wrapper: symlink this script as the command you want to proxy." >&2
    exit 1
fi
rtk-mine exec -- "$cmd" "$@"
WRAPPER_EOF
    chmod +x "$WRAPPER_DIR/rtk-wrapper"

    # Wrappable commands.
    CMDS=(ls cat head tail grep rg find git cargo pytest npm npx pnpm yarn go make)
    for cmd in "${CMDS[@]}"; do
        if [ ! -e "$WRAPPER_DIR/$cmd" ]; then
            ln -sf "$WRAPPER_DIR/rtk-wrapper" "$WRAPPER_DIR/$cmd"
            info "+ wrapper: $WRAPPER_DIR/$cmd"
        fi
    done

    # Add to PATH in shell configs (before other entries).
    for rc in "$HOME/.zshenv" "$HOME/.zshrc"; do
        if [ -f "$rc" ] && ! grep -qF "$WRAPPER_DIR" "$rc" 2>/dev/null; then
            # Insert at top so wrappers take priority.
            sed -i '' "1i\\
export PATH=\"$WRAPPER_DIR:\$PATH\"
" "$rc" 2>/dev/null || true
            info "+ PATH prepend in $rc"
        fi
    done
fi

# ── Stage 5: Done ───────────────────────────────────────────────────

header "Setup complete!"
echo ""
echo "  To activate immediately in this terminal:"
echo ""
echo -e "    ${BOLD}source ~/.zshenv && source ~/.zshrc${RESET}"
echo ""
echo "  Quick test:"
echo ""
echo "    type ls                         # should show 'ls is a shell function'"
echo "    rtk-mine exec -- ls -la         # test proxy directly"
echo "    rtk-mine audit                  # recent audit log"
echo "    rtk-mine audit stats            # savings dashboard"
echo ""
