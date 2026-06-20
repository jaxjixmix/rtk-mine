# CLAUDE.md — rtk-mine integration for Claude Code

This project uses [rtk-mine](https://github.com/user/rtk-mine) to reduce LLM token consumption on dev commands. When rtk-mine is active, command output is filtered and optimized for LLM consumption.

## Quick Setup

```bash
# Install (if not already installed)
cargo install --git https://github.com/user/rtk-mine.git

# Generate default config
rtk-mine config init

# Activate agent-mode shell integration (always proxies, even in pipes)
eval "$(rtk-mine init --agent --quiet)"
```

For persistent setup, add this to `~/.zshrc` or `~/.bashrc`:
```bash
eval "$(rtk-mine init --agent --quiet)"
```

## What Changes When Active

When rtk-mine is active (via `eval`), these commands are automatically proxied:

- `ls`, `cat`, `grep`, `rg`, `find` — output is filtered, truncated, and summarized
- `git status`, `git diff`, `git log` — structured summaries with file counts
- `cargo test`, `pytest`, `npm test` — only failures/errors shown
- Dangerous commands (`sudo`, `rm`, `curl`, `ssh`) are blocked

You can run any command without the proxy:
```bash
rtk-mine exec -- <command>    # one-shot proxy
command <cmd>                  # bypass the shell function entirely
```

## Checking Status

```bash
rtk-mine audit                 # recent proxied commands
rtk-mine audit stats           # savings dashboard
rtk-mine config show           # current configuration
```

## Important Notes

- rtk-mine **never sends data to external servers** — no telemetry
- All commands are logged to an audit trail for transparency
- Secrets (API keys, tokens) in command output are automatically redacted
- The proxy respects your `.gitignore` — it won't expose ignored files

## Agent-Specific Integration

### Claude Code

Add to your project's `CLAUDE.md` or user-level Claude config:

```bash
# Activate rtk-mine for this session
eval "$(rtk-mine init --agent --quiet)"
```

### CodeWhale

Add to your project's `.codewhale/instructions.md` or user-level setup:

```bash
eval "$(rtk-mine init --agent --quiet)"
```

### GitHub Copilot (in terminal/agent mode)

Add to `~/.zshrc` or `~/.bashrc`:
```bash
eval "$(rtk-mine init --agent --quiet)"
```

### OpenCode

Add to your startup script (`.zshrc`, `.bashrc`, or project env file):
```bash
eval "$(rtk-mine init --agent --quiet)"
```

## Troubleshooting

**Command not being proxied?**
- Run `type ls` — if it shows `ls is a shell function`, the proxy is active
- Run `rtk-mine exec -- ls -la` to test the proxy directly
- The `--agent` flag ensures proxying even with piped output

**Too much output being filtered?**
- Edit `~/.config/rtk-mine/config.toml` to adjust filter limits
- Increase `max_lines` or `max_entries` for individual commands
- Set `show_hidden = true` in `[filters.ls]` to see hidden files

**Audit log location:**
- Default: `~/.config/rtk-mine/audit.log`
- Configure: `[audit]` section in `~/.config/rtk-mine/config.toml`
