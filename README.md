# rtk-mine

**Secure, auditable CLI proxy that reduces LLM token consumption by 60–90% on everyday dev commands.**

Inspired by [rtk-ai/rtk](https://github.com/rtk-ai/rtk) (64k ★), rebuilt from scratch with **security** and **auditability** as first-class features.

---

## Why?

When an LLM coding agent runs `ls -R`, `git status`, or `npm test`, the raw output can be thousands of lines — most of it noise. That eats your context window and costs real money. rtk-mine intercepts those commands and returns a concise, LLM-optimized summary.

What makes rtk-mine different:
- **Security-first**: command allow/deny lists, secret redaction, path sandboxing
- **Fully auditable**: every command is logged to a structured JSON audit trail
- **Zero-config**: sensible defaults — one command to set up, zero to maintain

---

## Quick Start

### Install

```bash
# Option 1: build from source (requires Rust)
cargo install --git https://github.com/jaxjixmix/rtk-mine.git

# Option 2: one-liner
curl -fsSL https://raw.githubusercontent.com/jaxjixmix/rtk-mine/main/install.sh | bash
```

### Setup

```bash
# Generate default config
rtk-mine config init

# For interactive use (terminal)
eval "$(rtk-mine init)"

# For agent use (Claude Code, CodeWhale, Copilot, OpenCode)
eval "$(rtk-mine init --agent --quiet)"
```

For persistent setup, add the `eval` line to `~/.zshrc` or `~/.bashrc`.

### Use

Once hooked, your regular commands are automatically proxied:

```bash
ls -la          # filtered: "80 entries, 3 hidden suppressed"
git status      # filtered: "5 files (2 staged, 3 unstaged)"
cargo test      # filtered: "42 passed, 1 failed" + failure output
git diff        # filtered: "3 files changed" + summary
```

To run a command without the shell hook:

```bash
rtk-mine exec -- ls -la /some/deep/directory
```

### Audit Everything

```bash
# Recent audit entries
rtk-mine audit                    # last 20 commands
rtk-mine audit --program git      # only git commands
rtk-mine audit --limit 50 --json  # JSON output

# Savings dashboard
rtk-mine audit stats
```

Sample output:
```
╔═══════════════════════════════════════════╗
║  rtk-mine Audit Statistics               ║
╠═══════════════════════════════════════════╣
║ Total commands:          347              ║
║ Total bytes saved:     2.4MB              ║
║ Avg savings:            78.2%             ║
║ Secrets redacted:          2              ║
║ Blocked commands:          1              ║
║ Timed out:                 0              ║
╠═══════════════════════════════════════════╣
║ Top commands:                             ║
║   ls                      89              ║
║   git                     72              ║
║   cargo                   45              ║
╚═══════════════════════════════════════════╝
```

---

## Agent Integration

rtk-mine is designed to work seamlessly with all major coding agents. The agent mode (`--agent`) removes the terminal check so commands are proxied even when output is piped — exactly what agents need. The quiet flag (`--quiet`) suppresses the stderr summary line so agent output stays clean.

### Setup per agent

**Claude Code**
```bash
# Add to ~/.zshrc, ~/.bashrc, or your project's CLAUDE.md:
eval "$(rtk-mine init --agent --quiet)"
```

**CodeWhale**
```bash
# Add to ~/.zshrc, ~/.bashrc, or .codewhale/instructions.md:
eval "$(rtk-mine init --agent --quiet)"
```

**GitHub Copilot** (terminal/agent mode)
```bash
# Add to ~/.zshrc or ~/.bashrc:
eval "$(rtk-mine init --agent --quiet)"
```

**OpenCode**
```bash
# Add to startup script or project env:
eval "$(rtk-mine init --agent --quiet)"
```

### Verifying agent integration

```bash
# Check that shell functions are active
type ls
# Expected: ls is a shell function

# Test proxy with piped output (the --agent flag makes this work)
ls -la | cat
# Output should be filtered, not raw

# View what's being saved
rtk-mine audit stats
```

### Bypassing the proxy

When you need the real command output (not filtered):
```bash
command ls -la              # bypass shell function
rtk-mine exec -- ls -la     # one-shot proxy (explicit)
\ls -la                     # bash: bypass alias/function
```

---

## Architecture

```
eval "$(rtk-mine init)"
    │
    ▼
  ls -la
    │
    ▼
┌─────────────────────────────┐
│  Security Gate              │
│  • Command allowlist check  │
│  • Path sandbox check       │
│  • Env var filtering        │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│  Command Execution          │
│  • Run real command         │
│  • Capture stdout/stderr    │
│  • 30s timeout              │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│  Filter Engine              │
│  • Classify command         │
│  • Apply specialized filter │
│  • Strip ANSI, truncate     │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│  Secret Scanner             │
│  • Detect API keys          │
│  • Detect JWTs, tokens      │
│  • Redact in output         │
└──────────┬──────────────────┘
           ▼
┌─────────────────────────────┐
│  Audit Logger               │
│  • JSON Lines entry         │
│  • bytes before/after       │
│  • filter applied, exit code│
│  • retention rotation        │
└─────────────────────────────┘
           │
           ▼
     Filtered output
```

---

## Security Features

### Command Allow/Deny Lists

Commands are **default-allow with deny overrides** out of the box, or can be locked to **strict allowlist mode**. The deny list always takes priority.

```toml
[security]
# Only commands you trust (when require_allowlist = true)
allow_commands = ["ls", "cat", "grep", "find", "git", "cargo", "npm", ...]

# Absolutely blocked — always takes priority
deny_commands = ["sudo", "rm", "curl", "ssh", "sh", "bash", ...]

# Require explicit allowlisting for any command
require_allowlist = false
```

**25+ commands denied by default:**
`sudo`, `su`, `chmod`, `chown`, `rm`, `mv`, `dd`, `mkfs`, `mount`, `umount`, `shutdown`, `reboot`, `kill`, `pkill`, `killall`, `passwd`, `curl`, `wget`, `ssh`, `scp`, `nc`, `telnet`, `sh`, `bash`, `zsh`, `eval`, `exec`, `source`, `pip`, `pip3`, `gem`, `cpan`

### Secret Redaction

Automatically detects and redacts **11 secret patterns** from command output before it reaches the LLM:

| Pattern | Detection | Redaction |
|---------|-----------|-----------|
| OpenAI API keys | `sk-...` (20–80 chars) | `sk-***[REDACTED]` |
| OpenAI project keys | `sk-proj-...` | `sk-proj-***[REDACTED]` |
| Anthropic API keys | `sk-ant-...` | `sk-ant-***[REDACTED]` |
| GitHub tokens | `ghp_...`, `gho_...`, `ghu_...`, `ghs_...`, `ghr_...` | `gh?_***[REDACTED]` |
| AWS access keys | `AKIA...` (16 chars) | `AKIA***[REDACTED]` |
| AWS secret keys | `aws_secret=...`, `aws_key=...` | `aws_$1=***[REDACTED]` |
| JWT tokens | `eyJ...` three-part tokens | `***[JWT REDACTED]` |
| Private keys | `-----BEGIN RSA PRIVATE KEY-----` etc. | `***[PRIVATE KEY REDACTED]` |
| Bearer tokens | `Bearer ...` (20+ chars) | `Bearer ***[REDACTED]` |
| DB connection strings | `postgres://user:pass@host/db` | `postgres://***[REDACTED]@***` |
| Generic secrets | `api_key=...`, `token=...`, `password=...` | `$1=***[REDACTED]` |

```bash
$ rtk-mine exec -- cat .env
# Original:  OPENAI_API_KEY=sk-abc123def456...
# Filtered:  OPENAI_API_KEY=sk-***[REDACTED]
[rtk-mine] cat | 48 → 32 bytes (33% saved) | filter: cat | secrets: 1
```

### Path Sandbox

Restrict all proxied commands to specific directories:

```toml
[security]
allowed_paths = ["~/projects", "/opt/company-app"]
```

When `allowed_paths` is non-empty, commands outside those paths are blocked with a clear reason.

### Environment Filtering

**18+ sensitive environment variables** are stripped before any command runs:

`TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `API_KEY`, `AWS_SECRET`, `GITHUB_TOKEN`, `NPM_TOKEN`, `DOCKER_PASSWORD`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `DATABASE_URL`, `REDIS_URL`, `SENTRY_DSN`, `STRIPE_KEY`, `PRIVATE_KEY`, `CERT`

This means even if a proxied command dumps its environment, secrets won't leak.

---

## Filters

| Command | Filter Strategy | Typical Savings |
|---------|----------------|-----------------|
| `ls`, `ll`, `la` | Count + top entries only, hide dotfiles | 70–95% |
| `cat`, `head`, `tail` | Limit lines, collapse blanks, detect binary | 60–90% |
| `grep`, `rg`, `ag` | Limit matches, strip ANSI, count summary | 50–85% |
| `find`, `fd`, `locate` | Limit results, deduplicate, trim paths | 60–90% |
| `git status` | Categorized file counts (staged/unstaged/untracked) | 80–95% |
| `git diff` | Per-file stats + truncated hunks | 60–85% |
| `git log` | Recent commits, one-line format | 70–90% |
| `git branch` | Branch list with current marker | 50–80% |
| `cargo test` | Only failures/errors + pass/fail summary | 90–98% |
| `pytest` | Only failures + count summary | 90–98% |
| `npm test`, `yarn test` | Only failures + count summary | 85–95% |
| `go test` | Only failures + count summary | 85–95% |
| Generic fallback | Line limit, ANSI strip, line truncation | 30–60% |

**Note:** Git subcommands that modify state (`add`, `commit`, `push`, `pull`, `merge`, `rebase`, `checkout`, etc.) are intentionally **not proxied** — they run through the real `git` directly. Same for `cargo run`, `cargo publish`, etc.

---

## Audit Log

Every command produces one JSON line in `~/.config/rtk-mine/audit.log`:

```json
{
  "timestamp": "2026-06-20T12:34:56Z",
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "program": "git",
  "args": ["status"],
  "cwd": "/home/user/project",
  "exit_code": 0,
  "duration_ms": 142,
  "bytes_before": 2847,
  "bytes_after": 156,
  "savings_pct": 94.5,
  "filter_applied": "git",
  "security_verdict": "allow",
  "secrets_found": 0,
  "timed_out": false
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | ISO-8601 | When the command was executed |
| `id` | UUID v4 | Unique entry identifier |
| `program` | string | The executed program |
| `args` | [string] | Full argument list |
| `cwd` | string | Working directory at execution time |
| `exit_code` | int? | Exit code (null if blocked) |
| `duration_ms` | int | Wall-clock execution time |
| `bytes_before` | int | Raw output size before filtering |
| `bytes_after` | int | Output size after filtering + redaction |
| `savings_pct` | float | Token reduction percentage |
| `filter_applied` | string | Which filter was used |
| `security_verdict` | string | `"allow"` or `"deny:<reason>"` |
| `secrets_found` | int | Number of secrets detected and redacted |
| `timed_out` | bool | Whether the command exceeded timeout |

### Retention

Configure automatic log rotation:

```toml
[audit]
enabled = true
log_path = "~/.config/rtk-mine/audit.log"
retention_days = 90      # auto-delete entries older than 90 days
max_entries = 10000       # keep at most 10,000 entries (0 = unlimited)
```

---

## Configuration

Full config at `~/.config/rtk-mine/config.toml` (generated by `rtk-mine config init`):

```toml
[security]
# Commands allowed through the proxy
allow_commands = ["ls", "cat", "grep", "find", "git", "cargo", "pytest",
                  "npm", "npx", "pnpm", "yarn", "go", "node", "python",
                  "python3", "make", "rustc", "rustup", "echo", "pwd",
                  "which", "wc", "head", "tail", "sort", "uniq", "date",
                  "env", "printenv"]

# Commands absolutely blocked (overrides allow list)
deny_commands = ["sudo", "su", "chmod", "chown", "rm", "mv", "dd", "mkfs",
                  "mount", "umount", "shutdown", "reboot", "kill", "pkill",
                  "killall", "passwd", "curl", "wget", "ssh", "scp", "nc",
                  "telnet", "sh", "bash", "zsh", "eval", "exec", "source",
                  "pip", "pip3", "gem", "cpan"]

# If true, only allow_commands are permitted (strict mode)
require_allowlist = false

# Scan and redact secrets in command output
redact_secrets = true

# Maximum output bytes before truncation (0 = unlimited)
max_output_bytes = 102400

# Environment variables stripped before command execution
strip_env_vars = ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "API_KEY",
                   "AWS_SECRET", "GITHUB_TOKEN", "NPM_TOKEN", "DOCKER_PASSWORD",
                   "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY",
                   "DATABASE_URL", "REDIS_URL", "SENTRY_DSN", "STRIPE_KEY",
                   "PRIVATE_KEY", "CERT"]

# Restrict execution to these directory prefixes (empty = no restriction)
allowed_paths = []

[audit]
enabled = true
log_path = "~/.config/rtk-mine/audit.log"
retention_days = 90
max_entries = 10000

[proxy]
timeout_seconds = 30
capture_stderr = true

# Per-command filter overrides
[filters.ls]
max_entries = 80
show_hidden = false

[filters.cat]
max_lines = 200
max_line_length = 500

[filters.grep]
max_matches = 50

[filters.find]
max_entries = 100

[filters.git]
max_lines = 200

[filters.test]
show_passing = false
max_failures = 50

[filters.generic]
max_lines = 200
max_line_length = 500
```

---

## CLI Reference

### `rtk-mine exec`

Execute and filter a command through the proxy.

```
rtk-mine exec [--quiet] -- <command> [args...]
```

| Flag | Description |
|------|-------------|
| `--quiet` | Suppress the stderr summary line (for agent use) |

```bash
rtk-mine exec -- ls -la                         # with summary
rtk-mine exec --quiet -- ls -la                  # no summary
rtk-mine exec -- git diff HEAD~1                 # proxied git diff
rtk-mine exec -- cargo test                      # only failures shown
```

### `rtk-mine init`

Emit shell integration script. Pipe to `eval` or add to your shell rc file.

```
rtk-mine init [--shell <bash|zsh|fish>] [--agent] [--quiet]
```

| Flag | Description |
|------|-------------|
| `--shell` | Target shell (default: auto-detect) |
| `--agent` | Always proxy, even when output is piped (for coding agents) |
| `--quiet` | Suppress stderr summary in shell functions |

```bash
eval "$(rtk-mine init)"                 # interactive terminal use
eval "$(rtk-mine init --agent)"         # agent use (debugging)
eval "$(rtk-mine init --agent --quiet)" # agent use (production, recommended)
```

### `rtk-mine audit`

Query the audit log.

```
rtk-mine audit [log] [--program <name>] [--limit <n>] [--json]
rtk-mine audit stats [--json]
```

| Subcommand | Description |
|------------|-------------|
| `audit` or `audit log` | Show recent entries (default: last 20) |
| `audit stats` | Show aggregate savings dashboard |

| Flag | Description |
|------|-------------|
| `--program <name>` | Filter by command (e.g., `--program git`) |
| `--limit <n>` | Max entries to show (default: 20) |
| `--json` | Output as JSON |

```bash
rtk-mine audit                          # last 20 commands
rtk-mine audit log --program git       # only git commands
rtk-mine audit log --limit 50 --json   # JSON output
rtk-mine audit stats                    # savings dashboard
rtk-mine audit stats --json             # dashboard as JSON
```

### `rtk-mine config`

Manage configuration.

```
rtk-mine config init     Write default config to ~/.config/rtk-mine/config.toml
rtk-mine config show     Display current configuration
rtk-mine config path     Show config file path
```

---

## Privacy & Telemetry

rtk-mine **never sends data to external servers.** There is no telemetry, no analytics, no phone-home. The audit log stays on your machine, in a path you control. No network traffic is initiated by rtk-mine beyond executing the commands you explicitly request.

---

## Design Principles

1. **Single binary, zero deps.** Statically-linked Rust binary. No Python, Node, or runtime required.
2. **Security by default.** Commands denied unless allowed. Secrets redacted. Sensitive env vars stripped.
3. **Everything is audited.** No command runs without leaving a trace. Append-only JSON Lines log.
4. **LLM-native output.** Every filter produces output an LLM can parse in minimal tokens.
5. **Seamless shell integration.** One `eval` and your shell is proxied. Add to `.zshrc` for persistence.
6. **Transparent.** Config, audit log, and filter source are open and inspectable.
7. **Agent-aware.** Dedicated `--agent` mode for Claude Code, CodeWhale, Copilot, and OpenCode.

### How rtk-mine differs from the original RTK

| Feature | Original RTK | rtk-mine |
|---------|-------------|----------|
| Command policy | Default-allow all commands | Default-deny dangerous commands |
| Secret handling | Not a core feature | 11-pattern scanner + redaction |
| Audit trail | Not available | Full JSON Lines audit log |
| Telemetry | Opt-in telemetry | No telemetry whatsoever |
| Agent integration | Standard shell hooks only | Dedicated `--agent` + `--quiet` modes |
| Config | Custom format | Standard TOML with full defaults |
| Path sandbox | Not available | Optional directory restriction |

---

## Troubleshooting

**Command not being proxied?**
```bash
type ls                                    # should show "ls is a shell function"
rtk-mine exec -- ls -la                    # test proxy directly
echo $RTK_MINE_DISABLED                    # check if disabled via env
```

**"Operation not permitted" writing audit log?**
Check that the directory in `[audit].log_path` is writable. On sandboxed systems (e.g., CI), set it to a writable location:
```toml
[audit]
log_path = "/tmp/rtk-mine-audit.log"
```

**Too much output being filtered?**
```bash
rtk-mine config show                       # review current filter settings
```
Edit `~/.config/rtk-mine/config.toml` to increase limits:
```toml
[filters.ls]
max_entries = 200      # show more entries

[filters.git]
max_lines = 500         # show more diff content
```

**Need to run a denied command?**
```bash
command sudo ...         # bypasses the shell function
```
Or temporarily disable rtk-mine:
```bash
unset -f ls cat grep git cargo pytest       # remove shell functions
eval "$(rtk-mine init --agent --quiet)"      # re-enable when done
```

**Want to see what's happening under the hood?**
```bash
rtk-mine audit                               # recent commands
rtk-mine audit stats                         # savings dashboard
rtk-mine config show                         # full configuration
```

---

## Contributing

Contributions welcome! Please open an issue or PR on [GitHub](https://github.com/jaxjixmix/rtk-mine).

### Project Structure

```
rtk-mine/
├── Cargo.toml              # Rust project manifest
├── CLAUDE.md               # Claude Code / agent integration guide
├── README.md               # This file
├── install.sh              # One-liner installer
└── src/
    ├── main.rs             # CLI entry point (clap subcommands)
    ├── config.rs           # TOML config system
    ├── security.rs         # Allowlist, secret scanner, path sandbox
    ├── audit.rs            # JSON Lines audit log
    ├── proxy.rs            # Execution engine
    └── filters/
        ├── mod.rs          # Command classifier + dispatch
        ├── ls.rs           # Directory listings
        ├── cat.rs          # File viewing
        ├── grep.rs         # Search results
        ├── find.rs         # File finding
        ├── git.rs          # Git status/diff/log/branch
        ├── test.rs         # Test runners (cargo/pytest/npm/go)
        └── generic.rs      # Fallback filter
```

### Building

```bash
cargo build --release     # optimized binary (~2.2MB)
cargo test                # run 6 unit tests
```

---

## License

Apache 2.0 — see [LICENSE](LICENSE).

---

## Credits

Inspired by [rtk-ai/rtk](https://github.com/rtk-ai/rtk) by Patrick Szymkowiak, Florian Bruniaux, and Adrien Eppling.
