#!/usr/bin/env bash
# rtk-mine — universal shell hook installer for all coding agents
#
# This script adds the rtk-mine init hook to every shell init file
# that coding agents might source. Run once on your machine.
#
#   chmod +x scripts/install-hook.sh && ./scripts/install-hook.sh

set -euo pipefail

HOOK='eval "$(rtk-mine init --agent --quiet)"'
FILES=(
    "$HOME/.zshenv"     # zsh non-interactive (CodeWhale)
    "$HOME/.zshrc"      # zsh interactive (OpenCode, Claude Code)
    "$HOME/.bashrc"     # bash interactive
    "$HOME/.bash_profile" # bash login
    "$HOME/.profile"    # POSIX fallback
)

added=0
for f in "${FILES[@]}"; do
    if grep -qF "rtk-mine init" "$f" 2>/dev/null; then
        echo "✓ already in $f"
    else
        echo "$HOOK" >> "$f"
        echo "+ added to $f"
        added=$((added + 1))
    fi
done

# Also set BASH_ENV for bash non-interactive shells (Copilot, CodeWhale bash)
if ! grep -qF "rtk-mine init" "$HOME/.bashenv" 2>/dev/null; then
    echo "$HOOK" > "$HOME/.bashenv"
    echo "export BASH_ENV=\"$HOME/.bashenv\"" >> "$HOME/.bashrc" 2>/dev/null || true
    echo "export BASH_ENV=\"$HOME/.bashenv\"" >> "$HOME/.bash_profile" 2>/dev/null || true
    echo "+ created ~/.bashenv with BASH_ENV export"
fi

echo ""
echo "Done. Restart your agent session or run:"
echo "  source ~/.zshenv && source ~/.zshrc"
echo ""
echo "Verify with:"
echo "  type ls   # should show 'ls is a shell function'"
