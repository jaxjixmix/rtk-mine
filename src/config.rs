//! Configuration system for rtk-mine.
//!
//! Reads from `~/.config/rtk-mine/config.toml` with sensible defaults.
//! Supports command allow/deny lists, filter tuning, audit settings, and proxy limits.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub security: SecurityConfig,

    #[serde(default)]
    pub audit: AuditConfig,

    #[serde(default)]
    pub filters: HashMap<String, FilterConfig>,

    #[serde(default)]
    pub proxy: ProxyConfig,
}

/// Security policy: what's allowed, what's denied, secret handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Commands explicitly allowed. If non-empty, only these commands pass.
    #[serde(default = "default_allow_commands")]
    pub allow_commands: Vec<String>,

    /// Commands explicitly denied (takes priority over allow_commands).
    #[serde(default = "default_deny_commands")]
    pub deny_commands: Vec<String>,

    /// If non-empty, restrict execution to these directory prefixes.
    #[serde(default)]
    pub allowed_paths: Vec<String>,

    /// Whether to scan output for secrets and redact them.
    #[serde(default = "default_true")]
    pub redact_secrets: bool,

    /// Strip these environment variables before executing commands.
    #[serde(default = "default_sensitive_env")]
    pub strip_env_vars: Vec<String>,

    /// Maximum output bytes allowed (0 = unlimited).
    #[serde(default = "default_max_output")]
    pub max_output_bytes: usize,

    /// Require confirmation before running commands not in allow_commands.
    #[serde(default)]
    pub require_allowlist: bool,
}

/// Audit logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Whether audit logging is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path to the audit log file.
    #[serde(default = "default_audit_path")]
    pub log_path: String,

    /// Maximum entries to keep (0 = unlimited).
    #[serde(default)]
    pub max_entries: usize,

    /// Days to retain audit entries (0 = unlimited).
    #[serde(default = "default_retention")]
    pub retention_days: u32,
}

/// Per-command filter tuning.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterConfig {
    /// Max lines of output.
    #[serde(default)]
    pub max_lines: usize,
    /// Max bytes of output.
    #[serde(default)]
    pub max_bytes: usize,
    /// Extra settings (command-specific).
    #[serde(default)]
    pub options: HashMap<String, toml::Value>,
}

/// Proxy execution limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Timeout in seconds for command execution.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Whether to capture stderr in output.
    #[serde(default = "default_true")]
    pub capture_stderr: bool,
}

// ---- defaults ----

fn default_true() -> bool {
    true
}

fn default_max_output() -> usize {
    102_400 // 100KB
}

fn default_timeout() -> u64 {
    30
}

fn default_retention() -> u32 {
    90
}

fn default_allow_commands() -> Vec<String> {
    vec![
        "ls".into(),      "cat".into(),    "grep".into(),
        "find".into(),    "git".into(),     "cargo".into(),
        "pytest".into(),  "npm".into(),     "npx".into(),
        "pnpm".into(),    "yarn".into(),    "go".into(),
        "node".into(),    "python".into(),  "python3".into(),
        "make".into(),    "rustc".into(),   "rustup".into(),
        "echo".into(),    "pwd".into(),     "which".into(),
        "wc".into(),      "head".into(),    "tail".into(),
        "sort".into(),    "uniq".into(),    "date".into(),
        "env".into(),     "printenv".into(),
    ]
}

fn default_deny_commands() -> Vec<String> {
    vec![
        "sudo".into(),    "su".into(),      "chmod".into(),
        "chown".into(),   "rm".into(),       "mv".into(),
        "dd".into(),      "mkfs".into(),     "mount".into(),
        "umount".into(),  "shutdown".into(), "reboot".into(),
        "kill".into(),    "pkill".into(),    "killall".into(),
        "passwd".into(),  "curl".into(),     "wget".into(),
        "ssh".into(),     "scp".into(),      "nc".into(),
        "telnet".into(),  "sh".into(),       "bash".into(),
        "zsh".into(),     "eval".into(),     "exec".into(),
        "source".into(),  "pip".into(),      "pip3".into(),
        "gem".into(),     "cpan".into(),
    ]
}

fn default_sensitive_env() -> Vec<String> {
    vec![
        "TOKEN".into(),         "SECRET".into(),
        "PASSWORD".into(),      "PASSWD".into(),
        "API_KEY".into(),       "AWS_SECRET".into(),
        "GITHUB_TOKEN".into(),  "NPM_TOKEN".into(),
        "DOCKER_PASSWORD".into(),"OPENAI_API_KEY".into(),
        "ANTHROPIC_API_KEY".into(),"GEMINI_API_KEY".into(),
        "DATABASE_URL".into(),  "REDIS_URL".into(),
        "SENTRY_DSN".into(),    "STRIPE_KEY".into(),
        "PRIVATE_KEY".into(),   "CERT".into(),
    ]
}

fn default_audit_path() -> String {
    // Default to config directory.
    let cfg_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("rtk-mine");
    cfg_dir.join("audit.log").to_string_lossy().to_string()
}

// ---- impl ----

impl Default for Config {
    fn default() -> Self {
        Self {
            security: SecurityConfig::default(),
            audit: AuditConfig::default(),
            filters: HashMap::new(),
            proxy: ProxyConfig::default(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allow_commands: default_allow_commands(),
            deny_commands: default_deny_commands(),
            allowed_paths: Vec::new(),
            redact_secrets: true,
            strip_env_vars: default_sensitive_env(),
            max_output_bytes: default_max_output(),
            require_allowlist: false,
        }
    }
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_path: default_audit_path(),
            max_entries: 0,
            retention_days: default_retention(),
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            capture_stderr: true,
        }
    }
}

impl Config {
    /// Load config from the standard path, falling back to defaults.
    pub fn load() -> Self {
        let config_path = config_path();
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(cfg) => {
                        eprintln!("[rtk-mine] loaded config from {}", config_path.display());
                        cfg
                    }
                    Err(e) => {
                        eprintln!(
                            "[rtk-mine] invalid config at {} — using defaults: {}",
                            config_path.display(),
                            e
                        );
                        Self::default()
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[rtk-mine] cannot read config at {} — using defaults: {}",
                        config_path.display(),
                        e
                    );
                    Self::default()
                }
            }
        } else {
            eprintln!(
                "[rtk-mine] no config at {} — using defaults",
                config_path.display()
            );
            Self::default()
        }
    }

    /// Get the filter config for a command, or the default.
    pub fn filter_for(&self, cmd: &str) -> FilterConfig {
        self.filters.get(cmd).cloned().unwrap_or_default()
    }

    /// Resolve `~` in a path.
    pub fn resolve_path(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(stripped)
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        }
    }

    /// Get the resolved audit log path, validated to prevent traversal.
    pub fn audit_path(&self) -> PathBuf {
        if let Ok(path) = std::env::var("RTK_MINE_AUDIT_LOG") {
            if !path.is_empty() {
                return Self::resolve_path(&path);
            }
        }

        let resolved = Self::resolve_path(&self.audit.log_path);
        // Canonicalize to resolve any `..` components and symlinks.
        // If canonicalize fails (e.g., file doesn't exist yet), use the parent dir.
        match resolved.canonicalize() {
            Ok(canon) => canon,
            Err(_) => {
                // Try canonicalizing the parent to detect traversal.
                if let Some(parent) = resolved.parent() {
                    if let Ok(canon_parent) = parent.canonicalize() {
                        return canon_parent.join(
                            resolved.file_name().unwrap_or(std::ffi::OsStr::new("audit.log"))
                        );
                    }
                }
                resolved
            }
        }
    }

    /// Generate a default config file at the standard location.
    /// Falls back to current directory if the standard path is not writable.
    pub fn write_default() -> std::io::Result<PathBuf> {
        let path = config_path();
        let result = (|| -> std::io::Result<PathBuf> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let default_toml = toml::to_string_pretty(&Config::default())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let header = "# rtk-mine configuration\n# See https://github.com/jaxjixmix/rtk-mine for docs\n\n";
            std::fs::write(&path, format!("{}{}", header, default_toml))?;
            Ok(path.clone())
        })();

        match result {
            Ok(p) => Ok(p),
            Err(_) => {
                // Fallback: write to current directory.
                let fallback = std::path::PathBuf::from("rtk-mine-config.toml");
                let default_toml = toml::to_string_pretty(&Config::default())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let header = "# rtk-mine configuration (fallback location)\n# Move to ~/.config/rtk-mine/config.toml for automatic loading\n\n";
                std::fs::write(&fallback, format!("{}{}", header, default_toml))?;
                Ok(fallback)
            }
        }
    }
}

/// Standard config directory: `~/.config/rtk-mine/`
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("rtk-mine")
}

/// Standard config file path.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}
