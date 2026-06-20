# Changelog

All notable changes to rtk-mine will be documented in this file.

## [0.1.0] — 2026-06-20

### Initial Release

Secure, auditable CLI proxy that reduces LLM token consumption by 60–90% on everyday dev commands. Inspired by rtk-ai/rtk (64k ★), rebuilt from scratch with security and auditability as first-class features.

### Features

#### Core Proxy
- Command interception via shell functions (`eval "$(rtk-mine init)"`)
- Single static Rust binary (~2.2MB), zero runtime dependencies
- Three execution modes: interactive, agent, agent+quiet
- Stdout/stderr capture with pipe support (agent mode)

#### CLI
- `rtk-mine exec [--quiet] -- <command> [args...]` — execute and filter
- `rtk-mine init [--shell <bash|zsh|fish>] [--agent] [--quiet]` — shell integration
- `rtk-mine audit [log] [--program <name>] [--limit <n>] [--json]` — query audit
- `rtk-mine audit stats [--json]` — savings dashboard
- `rtk-mine audit clear` — wipe audit log
- `rtk-mine config init|show|path` — manage configuration

#### Shell Integration
- Bash, Zsh, and Fish support
- Agent mode (`--agent`): always proxy, no tty check — for Claude Code, CodeWhale, Copilot, OpenCode
- Quiet mode (`--quiet`): suppresses stderr summary for clean agent output
- Wraps: ls, ll, la, cat, head, tail, grep, rg, find, git, cargo, pytest, npm, npx, pnpm, yarn, go, make
- State-mutating git/cargo subcommands pass through directly (not proxied)

#### Filters (7 specialized + 1 fallback)

| Filter | Commands | Strategy |
|--------|----------|----------|
| ls | ls, ll, la, dir | Count + top N entries, hide dotfiles |
| cat | cat, head, tail, less | Line limits, blank compression, binary detection |
| grep | grep, rg, ag, ack | Match limits, ANSI stripping, summary count |
| find | find, fd, locate | Result limits, deduplication, path trimming |
| git | git status/diff/log/branch/show/tag | Structured summaries: staged/unstaged/untracked counts, per-file diff stats, one-line log |
| test | cargo test, pytest, npm test, Playwright, Vitest, Jest, go test | Only failures + pass/fail summary. Strips screenshots, HTML report URLs, call logs |
| generic | all others | Line limits, blank compression, line truncation, ANSI stripping |

#### Security
- **Command gate**: 25+ commands denied by default (sudo, rm, curl, ssh, bash, sh, pip, etc.)
- **Command-runner scanning**: env, nice, nohup, time, exec — arguments scanned against deny list
- **Interpreter -c/-e scanning**: python, node, ruby, perl — blocks dangerous patterns (import os, os.system, subprocess, eval, exec, rm -rf, etc.)
- **Secret scanner**: 11 regex patterns — OpenAI/Anthropic keys, GitHub tokens, AWS keys, JWT, private keys, DB URLs, generic KEY=VALUE secrets
- **Secret scan before truncation**: scans full raw output, not just first N lines
- **Path sandbox**: optional directory restriction for all proxied commands
- **Environment filtering**: 18+ sensitive env vars stripped before command execution (word-boundary matching)
- **Timeout enforcement**: 30s default, SIGKILL on timeout, exit code 124
- **Output size limit**: 100KB default max_output_bytes, enforced after filtering

#### Audit
- Append-only JSON Lines log at `~/.config/rtk-mine/audit.log`
- Each entry: timestamp, UUID, program, args, cwd, exit_code, duration_ms, bytes_before, bytes_after, savings_pct, filter_applied, security_verdict, secrets_found, timed_out
- Retention: configurable days and max entries
- Query by program, limit, JSON output
- Stats dashboard with totals, averages, top commands
- Path canonicalization to prevent `..` traversal

#### Agent Integration
- `CLAUDE.md` with per-agent setup for Claude Code, CodeWhale, Copilot, OpenCode
- One-liner: `eval "$(rtk-mine init --agent --quiet)"`
- Agent mode removes tty guard — proxies even when output is piped
- Quiet mode suppresses stderr summary for clean agent output

#### Configuration
- TOML config at `~/.config/rtk-mine/config.toml`
- Sensible defaults for all settings
- Per-command filter tuning (max_lines, max_entries, show_hidden, etc.)
- `rtk-mine config init` generates with fallback to cwd

#### Privacy
- Zero telemetry — nothing phones home
- No network traffic initiated beyond requested commands
- Audit log stays local, path is configurable

### Testing
- 9 unit tests: secret detection (3), deny list (1), allow list (1), cargo test parser (1), Playwright output parser (3)
- Comprehensive pentest: 16 subtests across 7 attack vectors — 0 unpatched critical/high findings

### Build
- `cargo build --release` produces ~2.2MB stripped binary
- `cargo test` — 9/9 passing, zero warnings
- Dependencies: clap, serde, serde_json, toml, chrono, regex, dirs, uuid (all pure Rust)
