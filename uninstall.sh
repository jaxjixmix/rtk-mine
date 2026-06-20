#!/usr/bin/env bash
# rtk-mine uninstaller — removes the binary, config, wrappers, and shell hooks.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jaxjixmix/rtk-mine/main/uninstall.sh | bash
#   ./uninstall.sh
#
# Options:
#   KEEP_CONFIG=1 ./uninstall.sh   — keep config and audit log
#   KEEP_BINARY=1 ./uninstall.sh   — keep the binary
#   DRY_RUN=1    ./uninstall.sh    — show what would be removed

set -euo pipefail

GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"
BOLD="\033[1m"

info()  { echo -e "${GREEN}→${RESET} $*"; }
warn()  { echo -e "${YELLOW}⚠${RESET} $*"; }
err()   { echo -e "${RED}✗${RESET} $*" >&2; }
header(){ echo -e "\n${BOLD}$*${RESET}\n"; }

DRY_RUN="${DRY_RUN:-0}"
KEEP_CONFIG="${KEEP_CONFIG:-0}"
KEEP_BINARY="${KEEP_BINARY:-0}"

header "rtk-mine uninstaller"

# ── Remove shell hooks ──────────────────────────────────────────────

header "Shell hooks"
FILES=(
    "$HOME/.zshenv"
    "$HOME/.zshrc"
    "$HOME/.bashrc"
    "$HOME/.bash_profile"
    "$HOME/.bashenv"
)
for f in "${FILES[@]}"; do
    if [ -f "$f" ] && grep -qF "rtk-mine" "$f" 2>/dev/null; then
        if [ "$DRY_RUN" = "1" ]; then
            info "[dry-run] would remove rtk-mine hooks from $f"
        else
            sed -i '' '/rtk-mine/d' "$f" 2>/dev/null || sed -i '/rtk-mine/d' "$f"
            info "cleaned $f"
        fi
    fi
done

# Remove BASH_ENV export lines.
for f in "$HOME/.bashrc" "$HOME/.bash_profile"; do
    if [ -f "$f" ] && grep -qF 'rtk-mine' "$f" 2>/dev/null; then
        if [ "$DRY_RUN" != "1" ]; then
            sed -i '' '/BASH_ENV.*rtk-mine/d' "$f" 2>/dev/null || true
        fi
    fi
done

# ── Remove PATH wrappers ────────────────────────────────────────────

header "PATH wrappers"
WRAPPER_DIR="$HOME/.rtk-mine/bin"
if [ -d "$WRAPPER_DIR" ]; then
    if [ "$DRY_RUN" = "1" ]; then
        info "[dry-run] would remove $WRAPPER_DIR"
    else
        rm -rf "$WRAPPER_DIR"
        info "removed $WRAPPER_DIR"
    fi
fi

# ── Remove binary ───────────────────────────────────────────────────

if [ "$KEEP_BINARY" != "1" ]; then
    header "Binary"
    for dir in /usr/local/bin "$HOME/.cargo/bin" "$HOME/.local/bin"; do
        if [ -f "$dir/rtk-mine" ]; then
            if [ "$DRY_RUN" = "1" ]; then
                info "[dry-run] would remove $dir/rtk-mine"
            else
                rm -f "$dir/rtk-mine"
                info "removed $dir/rtk-mine"
            fi
        fi
    done
    # Also check cargo install.
    if command -v cargo >/dev/null 2>&1; then
        if cargo install --list 2>/dev/null | grep -q "rtk-mine"; then
            if [ "$DRY_RUN" = "1" ]; then
                info "[dry-run] would run: cargo uninstall rtk-mine"
            else
                cargo uninstall rtk-mine 2>/dev/null || true
                info "uninstalled via cargo"
            fi
        fi
    fi
fi

# ── Remove config and data ──────────────────────────────────────────

if [ "$KEEP_CONFIG" != "1" ]; then
    header "Config & data"
    CONFIG_DIR="$HOME/.config/rtk-mine"
    if [ -d "$CONFIG_DIR" ]; then
        if [ "$DRY_RUN" = "1" ]; then
            info "[dry-run] would remove $CONFIG_DIR"
        else
            rm -rf "$CONFIG_DIR"
            info "removed $CONFIG_DIR"
        fi
    fi
    # macOS alternative config location.
    MACOS_CONFIG="$HOME/Library/Application Support/rtk-mine"
    if [ -d "$MACOS_CONFIG" ]; then
        if [ "$DRY_RUN" = "1" ]; then
            info "[dry-run] would remove $MACOS_CONFIG"
        else
            rm -rf "$MACOS_CONFIG"
            info "removed $MACOS_CONFIG"
        fi
    fi
    # CWD fallback files.
    for f in rtk-mine-audit.log rtk-mine-config.toml; do
        if [ -f "$f" ]; then
            rm -f "$f"
            info "removed ./$f"
        fi
    done
fi

# ── Done ────────────────────────────────────────────────────────────

header "Uninstall complete!"
echo ""
if [ "$DRY_RUN" = "1" ]; then
    echo "  This was a dry run. Run without DRY_RUN=1 to actually remove."
else
    echo "  Run these to clean your current shell:"
    echo ""
    echo -e "    ${BOLD}unset -f ls cat grep find git cargo pytest npm npx pnpm yarn go make 2>/dev/null${RESET}"
    echo -e "    ${BOLD}exec \$SHELL${RESET}"
fi
echo ""
