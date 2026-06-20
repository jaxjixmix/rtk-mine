//! Security module: command allowlisting, secret detection, and path sandboxing.
//!
//! Every command passes through three gates:
//! 1. **Command gate** — is this command allowed?
//! 2. **Path gate** — is the working directory allowed?
//! 3. **Secret gate** — does the output contain secrets that must be redacted?

use crate::config::SecurityConfig;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

/// Outcome of a security check.
#[derive(Debug, Clone)]
pub enum SecurityVerdict {
    /// Command is safe to execute.
    Allow,
    /// Command is blocked — with a reason.
    Deny(String),
}

/// Structured result from the secret scanner.
#[derive(Debug, Clone)]
pub struct SecretScan {
    /// The (possibly redacted) output.
    pub output: String,
    /// Number of secrets detected and redacted.
    pub secrets_found: usize,
}

/// Secret patterns — compiled once.
static SECRET_PATTERNS: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        // OpenAI keys
        (Regex::new(r"sk-[a-zA-Z0-9]{20,80}").unwrap(), "sk-***[REDACTED]"),
        (Regex::new(r"sk-proj-[a-zA-Z0-9_-]{20,80}").unwrap(), "sk-proj-***[REDACTED]"),
        // Anthropic keys
        (Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,80}").unwrap(), "sk-ant-***[REDACTED]"),
        // GitHub tokens
        (Regex::new(r"gh[pousr]_[a-zA-Z0-9]{20,60}").unwrap(), "gh?_***[REDACTED]"),
        // Generic API keys (key=value pairs)
        (Regex::new(r#"(?i)(api[_-]?key|apikey|secret|token|password|passwd)\s*[:=]\s*["']?[^\s"']{8,}["']?"#).unwrap(), "$1=***[REDACTED]"),
        // AWS keys
        (Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(), "AKIA***[REDACTED]"),
        (Regex::new(r"aws[_]?(secret|key|token)\s*[:=]\s*[^\s]+").unwrap(), "aws_$1=***[REDACTED]"),
        // JWT tokens
        (Regex::new(r"eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}").unwrap(), "***[JWT REDACTED]"),
        // Private key headers
        (Regex::new(r"-----BEGIN (RSA|EC|DSA|OPENSSH) PRIVATE KEY-----").unwrap(), "***[PRIVATE KEY REDACTED]"),
        // Generic bearer tokens
        (Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.]{20,}").unwrap(), "Bearer ***[REDACTED]"),
        // Database connection strings
        (Regex::new(r"(?i)(postgres|mysql|mongodb|redis)://[^@\s]+@[^\s]+").unwrap(), "$1://***[REDACTED]@***"),
    ]
});

// ---- Command gate ----

/// Programs that can run other commands through their arguments.
/// e.g., `env curl ...`, `nice bash ...`, `time ssh ...`.
const COMMAND_RUNNERS: &[&str] = &["env", "nice", "nohup", "time", "exec", "sudo", "su"];

/// Scriptable interpreters — their `-c` flag can execute arbitrary code.
const INTERPRETERS: &[&str] = &[
    "python", "python3", "ruby", "perl", "node", "php",
    "bash", "zsh", "sh", "fish", "lua", "tclsh",
];

/// Check whether a command should be allowed.
pub fn check_command(
    program: &str,
    args: &[String],
    config: &SecurityConfig,
) -> SecurityVerdict {
    // 1. Deny list — absolute block on the program itself.
    let prog_lower = program.to_lowercase();
    let base = Path::new(program)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_lowercase();

    for denied in &config.deny_commands {
        if prog_lower == *denied || base == *denied {
            return SecurityVerdict::Deny(format!(
                "command '{}' is on the deny list",
                program
            ));
        }
    }

    // 2. Scan command-runner args against the deny list.
    //    `env curl ...` → curl is denied → block.
    if COMMAND_RUNNERS.contains(&base.as_str()) {
        for arg in args {
            let arg_base = Path::new(arg)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(arg)
                .to_lowercase();
            for denied in &config.deny_commands {
                if arg_base == *denied {
                    return SecurityVerdict::Deny(format!(
                        "denied command '{}' invoked via '{} {}'",
                        arg, program, arg
                    ));
                }
            }
        }
    }

    // 3. Scan interpreter `-c` arguments for dangerous patterns.
    if INTERPRETERS.contains(&base.as_str()) {
        let mut next_is_code = false;
        for arg in args {
            if arg == "-c" || arg == "-e" {
                next_is_code = true;
                continue;
            }
            if next_is_code {
                let lower = arg.to_lowercase();
                // Block known-dangerous calls.
                if lower.contains("import os")
                    || lower.contains("subprocess")
                    || lower.contains("os.system")
                    || lower.contains("os.popen")
                    || lower.contains("eval(")
                    || lower.contains("exec(")
                    || lower.contains("__import__")
                    || lower.contains("rm -rf")
                    || lower.contains("; rm")
                    || lower.contains("shutil.rmtree")
                {
                    return SecurityVerdict::Deny(format!(
                        "potentially dangerous code in '{}' -c argument",
                        program
                    ));
                }
                next_is_code = false;
            }
        }
    }

    // 4. Allow list — if require_allowlist is set, only these pass.
    if config.require_allowlist {
        let allowed = config
            .allow_commands
            .iter()
            .any(|a| prog_lower == *a || base == *a);
        if !allowed {
            return SecurityVerdict::Deny(format!(
                "command '{}' is not on the allow list (require_allowlist is set)",
                program
            ));
        }
    }

    SecurityVerdict::Allow
}

// ---- Path gate ----

/// Check whether the working directory is within allowed paths.
pub fn check_path(cwd: &Path, config: &SecurityConfig) -> SecurityVerdict {
    if config.allowed_paths.is_empty() {
        return SecurityVerdict::Allow;
    }

    let cwd = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(cwd)
    };

    let cwd = cwd.canonicalize().unwrap_or(cwd);

    for allowed in &config.allowed_paths {
        let allowed_path = crate::config::Config::resolve_path(allowed);
        let allowed_path = allowed_path.canonicalize().unwrap_or(allowed_path);
        if cwd.starts_with(&allowed_path) {
            return SecurityVerdict::Allow;
        }
    }

    SecurityVerdict::Deny(format!(
        "working directory '{}' is not within any allowed path",
        cwd.display()
    ))
}

// ---- Secret gate ----

/// Scan text for secrets and redact them.
pub fn scan_and_redact(text: &str) -> SecretScan {
    let mut output = text.to_string();
    let mut secrets_found = 0;

    for (pattern, replacement) in SECRET_PATTERNS.iter() {
        let matches: Vec<_> = pattern.find_iter(&output).collect();
        secrets_found += matches.len();
        if !matches.is_empty() {
            output = pattern.replace_all(&output, *replacement).to_string();
        }
    }

    SecretScan {
        secrets_found,
        output,
    }
}

/// Strip sensitive environment variables from a command's environment.
pub fn filter_env(env_vars: &[(String, String)], config: &SecurityConfig) -> Vec<(String, String)> {
    env_vars
        .iter()
        .filter(|(key, _)| {
            let upper = key.to_uppercase();
            // Exact match against each sensitive pattern (case-insensitive).
            // Uses word-boundary check: matches "TOKEN" in "GITHUB_TOKEN"
            // but not "MY_TOKENIZER".
            !config.strip_env_vars.iter().any(|sensitive| {
                let s = sensitive.to_uppercase();
                upper == s
                    || upper.starts_with(&format!("{}_", s))
                    || upper.ends_with(&format!("_{}", s))
                    || upper.contains(&format!("_{}_", s))
            })
        })
        .cloned()
        .collect()
}

// ---- Helpers ----

/// Extract the base command name from a path.
pub fn command_name(program: &str) -> &str {
    Path::new(program)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_openai_key() {
        let input = "export OPENAI_API_KEY=sk-abc123def456ghi789jkl012mno345pqr678stu";
        let result = scan_and_redact(input);
        assert!(result.secrets_found > 0);
        assert!(!result.output.contains("sk-abc123"));
        assert!(result.output.contains("REDACTED"));
    }

    #[test]
    fn test_detect_jwt() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let result = scan_and_redact(input);
        assert!(result.secrets_found > 0);
        assert!(result.output.contains("JWT REDACTED"));
    }

    #[test]
    fn test_no_false_positive() {
        let input = "normal git output with commit hashes abc123 and file paths";
        let result = scan_and_redact(input);
        assert_eq!(result.secrets_found, 0);
        assert_eq!(result.output, input);
    }

    #[test]
    fn test_deny_list_blocks_sudo() {
        let config = SecurityConfig::default();
        let verdict = check_command("sudo", &[], &config);
        assert!(matches!(verdict, SecurityVerdict::Deny(_)));
    }

    #[test]
    fn test_allow_list_passes_ls() {
        let config = SecurityConfig::default();
        let verdict = check_command("ls", &[], &config);
        assert!(matches!(verdict, SecurityVerdict::Allow));
    }
}
