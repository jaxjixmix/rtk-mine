//! rtk-mine — Secure, Auditable CLI Proxy for LLM Token Reduction.
//!
//! Inspired by rtk-ai/rtk, rebuilt from scratch with security and
//! auditability as first-class features.
//!
//! ```text
//! eval "$(rtk-mine init)"   # one-command setup — hooks your shell
//! rtk-mine exec -- ls -la   # intercept and filter a command
//! rtk-mine audit             # view the audit log
//! rtk-mine audit stats       # savings summary
//! rtk-mine config init       # write default config
//! ```

mod config;
mod security;
mod audit;
mod proxy;
mod filters;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Secure, auditable CLI proxy that reduces LLM token consumption on dev commands.
#[derive(Parser)]
#[command(
    name = "rtk-mine",
    version,
    about = "Reduce LLM token usage by 60-90% on common dev commands — with security and audit built in.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a command through the proxy and return filtered output.
    Exec {
        /// The command to run (e.g., ls, git, cargo).
        program: String,
        /// Arguments to pass to the command.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Suppress stderr summary line (for agent consumption).
        #[arg(long)]
        quiet: bool,
    },

    /// Emit shell integration script (eval this in your shell).
    Init {
        /// Target shell: bash, zsh, or fish.
        #[arg(long, default_value = "auto")]
        shell: String,
        /// Agent mode: always proxy, even when output is piped.
        #[arg(long)]
        agent: bool,
        /// Suppress stderr summary in shell functions.
        #[arg(long)]
        quiet: bool,
    },

    /// Query the audit log (default: show recent entries).
    Audit {
        #[command(subcommand)]
        action: Option<AuditAction>,
    },

    /// Manage configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum AuditAction {
    /// Show recent audit entries.
    Log {
        /// Filter by program name.
        #[arg(long)]
        program: Option<String>,
        /// Max entries to show.
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show aggregate statistics.
    Stats {
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Write a default config file to the standard location.
    Init,
    /// Show the current configuration.
    Show,
    /// Show the config file path.
    Path,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Exec { program, args, quiet } => {
            cmd_exec(&program, &args, quiet);
        }
        Commands::Init { shell, agent, quiet } => {
            cmd_init(&shell, agent, quiet);
        }
        Commands::Audit { action } => match action {
            None | Some(AuditAction::Log { .. }) => {
                let (program, limit, json) = match action {
                    Some(AuditAction::Log { program, limit, json }) => (program, limit, json),
                    _ => (None, 20, false),
                };
                cmd_audit_log(program.as_deref(), limit, json);
            }
            Some(AuditAction::Stats { json }) => {
                cmd_audit_stats(json);
            }
        },
        Commands::Config { action } => match action {
            ConfigAction::Init => cmd_config_init(),
            ConfigAction::Show => cmd_config_show(),
            ConfigAction::Path => cmd_config_path(),
        },
    }
}

// ---- exec ----

fn cmd_exec(program: &str, args: &[String], quiet: bool) {
    let cfg = config::Config::load();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let result = proxy::execute(program, args, &cwd, &cfg);

    // Print the filtered output to stdout.
    print!("{}", result.output);

    // Summary to stderr — suppressed in quiet mode.
    if !quiet {
        let savings = if result.bytes_before > 0 {
            let saved = result.bytes_before.saturating_sub(result.bytes_after);
            format!("{:.0}%", (saved as f64 / result.bytes_before as f64) * 100.0)
        } else {
            "—".into()
        };

        if result.blocked {
            eprintln!(
                "[rtk-mine] BLOCKED: {} | reason: {}",
                program,
                result.block_reason.as_deref().unwrap_or("unknown")
            );
        } else {
            eprintln!(
                "[rtk-mine] {} | {} → {} bytes ({} saved) | filter: {} | secrets: {}",
                program,
                result.bytes_before,
                result.bytes_after,
                savings,
                result.filter_applied,
                result.secrets_found,
            );
        }
    }

    // Exit with the command's exit code, or 1 if blocked.
    std::process::exit(result.exit_code.unwrap_or(1));
}

// ---- init ----

fn cmd_init(shell: &str, agent: bool, quiet: bool) {
    let detected = if shell == "auto" {
        detect_shell()
    } else {
        shell.to_string()
    };

    let script = match detected.as_str() {
        "zsh" | "bash" => {
            if agent {
                shell_script_bash_zsh_agent(quiet)
            } else {
                shell_script_bash_zsh()
            }
        }
        "fish" => {
            if agent {
                shell_script_fish_agent(quiet)
            } else {
                shell_script_fish()
            }
        }
        _ => {
            eprintln!("[rtk-mine] unknown shell '{}' — emitting bash/zsh script", detected);
            if agent {
                shell_script_bash_zsh_agent(quiet)
            } else {
                shell_script_bash_zsh()
            }
        }
    };

    println!("{}", script);
    let mode = if agent { " (agent mode)" } else { "" };
    eprintln!(
        "[rtk-mine] shell integration for {}{} emitted — add to .{}rc or eval directly:",
        detected, mode, detected
    );
    if agent {
        eprintln!("  eval \"$(rtk-mine init --agent)\"");
    } else {
        eprintln!("  eval \"$(rtk-mine init)\"");
    }
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_default()
        .split('/')
        .last()
        .unwrap_or("bash")
        .to_string()
}

fn shell_script_bash_zsh() -> String {
    r#"# rtk-mine shell integration — wraps common commands through the proxy.

__rtk_mine_exec() {
    local cmd=$1
    shift
    # Only proxy if we're in a terminal (not a pipe/redirect).
    if [ -t 1 ]; then
        rtk-mine exec -- "$cmd" "$@"
    else
        command "$cmd" "$@"
    fi
}

# File listing
ls()    { __rtk_mine_exec ls "$@"; }
ll()    { __rtk_mine_exec ls -l "$@"; }
la()    { __rtk_mine_exec ls -la "$@"; }

# File viewing
cat()   { __rtk_mine_exec cat "$@"; }
head()  { __rtk_mine_exec head "$@"; }
tail()  { __rtk_mine_exec tail "$@"; }

# Searching
grep()  { __rtk_mine_exec grep "$@"; }
rg()    { __rtk_mine_exec rg "$@"; }
find()  { __rtk_mine_exec find "$@"; }

# Version control
git() {
    # Don't proxy destructive or interactive git commands.
    case "$1" in
        add|commit|push|pull|fetch|merge|rebase|checkout|switch|restore|reset|stash)
            command git "$@"
            ;;
        *)
            __rtk_mine_exec git "$@"
            ;;
    esac
}

# Build / test
cargo() {
    case "$1" in
        test|build|check|clippy)
            __rtk_mine_exec cargo "$@"
            ;;
        *)
            command cargo "$@"
            ;;
    esac
}
pytest() { __rtk_mine_exec pytest "$@"; }

# Quick stats
rtk-stats() { rtk-mine audit stats; }

# Source this file or eval: eval "$(rtk-mine init)"
"#.to_string()
}

// ---- Agent-mode variants (no tty check, always proxy) ----

fn shell_script_bash_zsh_agent(quiet: bool) -> String {
    let quiet_flag = if quiet { " --quiet" } else { "" };
    format!(
        r#"# rtk-mine agent integration — always proxies commands for LLM consumption.
# Use: eval "$(rtk-mine init --agent)"

__rtk_mine_exec() {{
    local cmd=$1
    shift
    rtk-mine exec{quiet_flag} -- "$cmd" "$@"
}}

# File listing
ls()    {{ __rtk_mine_exec ls "$@"; }}
ll()    {{ __rtk_mine_exec ls -l "$@"; }}
la()    {{ __rtk_mine_exec ls -la "$@"; }}

# File viewing
cat()   {{ __rtk_mine_exec cat "$@"; }}
head()  {{ __rtk_mine_exec head "$@"; }}
tail()  {{ __rtk_mine_exec tail "$@"; }}

# Searching
grep()  {{ __rtk_mine_exec grep "$@"; }}
rg()    {{ __rtk_mine_exec rg "$@"; }}
find()  {{ __rtk_mine_exec find "$@"; }}

# Version control
git() {{
    case "$1" in
        add|commit|push|pull|fetch|merge|rebase|checkout|switch|restore|reset|stash)
            command git "$@"
            ;;
        *)
            __rtk_mine_exec git "$@"
            ;;
    esac
}}

# Build / test
cargo() {{
    case "$1" in
        test|build|check|clippy)
            __rtk_mine_exec cargo "$@"
            ;;
        *)
            command cargo "$@"
            ;;
    esac
}}
pytest() {{ __rtk_mine_exec pytest "$@"; }}

# Quick stats
rtk-stats() {{ rtk-mine audit stats; }}
"#
    )
}

fn shell_script_fish_agent(quiet: bool) -> String {
    let quiet_flag = if quiet { " --quiet" } else { "" };
    format!(
        r#"# rtk-mine agent integration for fish — always proxies commands.
# Use: eval (rtk-mine init --agent)

function __rtk_mine_exec
    set cmd $argv[1]
    set -e argv[1]
    rtk-mine exec{quiet_flag} -- $cmd $argv
end

function ls     ; __rtk_mine_exec ls $argv ; end
function ll     ; __rtk_mine_exec ls -l $argv ; end
function la     ; __rtk_mine_exec ls -la $argv ; end
function cat    ; __rtk_mine_exec cat $argv ; end
function head   ; __rtk_mine_exec head $argv ; end
function tail   ; __rtk_mine_exec tail $argv ; end
function grep   ; __rtk_mine_exec grep $argv ; end
function rg     ; __rtk_mine_exec rg $argv ; end
function find   ; __rtk_mine_exec find $argv ; end

function git
    switch $argv[1]
        case add commit push pull fetch merge rebase checkout switch restore reset stash
            command git $argv
        case '*'
            __rtk_mine_exec git $argv
    end
end

function cargo
    switch $argv[1]
        case test build check clippy
            __rtk_mine_exec cargo $argv
        case '*'
            command cargo $argv
    end
end

function pytest ; __rtk_mine_exec pytest $argv ; end
function rtk-stats ; rtk-mine audit stats ; end
"#
    )
}

fn shell_script_fish() -> String {
    r#"# rtk-mine shell integration for fish.

function __rtk_mine_exec
    set cmd $argv[1]
    set -e argv[1]
    if isatty stdout
        rtk-mine exec -- $cmd $argv
    else
        command $cmd $argv
    end
end

function ls     ; __rtk_mine_exec ls $argv ; end
function ll     ; __rtk_mine_exec ls -l $argv ; end
function la     ; __rtk_mine_exec ls -la $argv ; end
function cat    ; __rtk_mine_exec cat $argv ; end
function head   ; __rtk_mine_exec head $argv ; end
function tail   ; __rtk_mine_exec tail $argv ; end
function grep   ; __rtk_mine_exec grep $argv ; end
function rg     ; __rtk_mine_exec rg $argv ; end
function find   ; __rtk_mine_exec find $argv ; end

function git
    switch $argv[1]
        case add commit push pull fetch merge rebase checkout switch restore reset stash
            command git $argv
        case '*'
            __rtk_mine_exec git $argv
    end
end

function cargo
    switch $argv[1]
        case test build check clippy
            __rtk_mine_exec cargo $argv
        case '*'
            command cargo $argv
    end
end

function pytest ; __rtk_mine_exec pytest $argv ; end
function rtk-stats ; rtk-mine audit stats ; end
"#.to_string()
}

// ---- audit ----

fn cmd_audit_log(program: Option<&str>, limit: usize, json: bool) {
    let cfg = config::Config::load();
    match audit::query_entries(&cfg, program, limit) {
        Ok(entries) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap_or_default());
            } else {
                if entries.is_empty() {
                    println!("No audit entries found.");
                    return;
                }
                for entry in &entries {
                    println!(
                        "{} | {:>8} | {:>7} → {:<7} | {:>5.0}% | {} | exit={}",
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        entry.program,
                        fmt_bytes(entry.bytes_before),
                        fmt_bytes(entry.bytes_after),
                        entry.savings_pct,
                        entry.filter_applied,
                        entry.exit_code.map_or("-".into(), |c| c.to_string()),
                    );
                }
                println!("--- {} entries ---", entries.len());
            }
        }
        Err(e) => {
            eprintln!("[rtk-mine] audit query error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_audit_stats(json: bool) {
    let cfg = config::Config::load();
    match audit::compute_stats(&cfg) {
        Ok(stats) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
            } else {
                println!("╔═══════════════════════════════════════════╗");
                println!("║  rtk-mine Audit Statistics               ║");
                println!("╠═══════════════════════════════════════════╣");
                println!("║ Total commands:       {:>6}              ║", stats.total_commands);
                println!("║ Total bytes saved:    {:>6}              ║", fmt_bytes(stats.total_bytes_saved as usize));
                println!("║ Avg savings:          {:>5.1}%             ║", stats.avg_savings_pct);
                println!("║ Secrets redacted:     {:>6}              ║", stats.total_secrets_redacted);
                println!("║ Blocked commands:     {:>6}              ║", stats.blocked_commands);
                println!("║ Timed out:            {:>6}              ║", stats.timed_out);
                println!("╠═══════════════════════════════════════════╣");
                if !stats.top_commands.is_empty() {
                    println!("║ Top commands:                             ║");
                    for (cmd, count) in &stats.top_commands {
                        println!("║   {:<20} {:>5}                  ║", cmd, count);
                    }
                }
                println!("╚═══════════════════════════════════════════╝");
            }
        }
        Err(e) => {
            eprintln!("[rtk-mine] audit stats error: {}", e);
            std::process::exit(1);
        }
    }
}

// ---- config ----

fn cmd_config_init() {
    match config::Config::write_default() {
        Ok(path) => {
            println!("Default config written to: {}", path.display());
            eprintln!("[rtk-mine] edit this file to customize security, audit, and filter settings");
        }
        Err(e) => {
            eprintln!("[rtk-mine] failed to write config: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_config_show() {
    let cfg = config::Config::load();
    match toml::to_string_pretty(&cfg) {
        Ok(toml_str) => println!("{}", toml_str),
        Err(e) => {
            eprintln!("[rtk-mine] failed to serialize config: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_config_path() {
    println!("{}", config::config_path().display());
}

// ---- helpers ----

fn fmt_bytes(n: usize) -> String {
    if n >= 1_048_576 {
        format!("{:.1}MB", n as f64 / 1_048_576.0)
    } else if n >= 1024 {
        format!("{:.1}KB", n as f64 / 1024.0)
    } else {
        format!("{}B", n)
    }
}
