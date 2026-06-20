//! Proxy engine: executes commands through the security gate, captures output,
//! dispatches to the appropriate filter, and returns the optimized result.

use crate::audit::{self, AuditEntry};
use crate::config::Config;
use crate::filters;
use crate::security::{self, SecretScan, SecurityVerdict};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Instant;

/// Result of a proxy execution.
#[derive(Debug)]
pub struct ProxyResult {
    /// The filtered output (stdout + optionally stderr).
    pub output: String,
    /// Exit code of the command.
    pub exit_code: Option<i32>,
    /// Whether the command was blocked by security.
    pub blocked: bool,
    /// The reason if blocked.
    pub block_reason: Option<String>,
    /// Whether the command timed out.
    pub timed_out: bool,
    /// Name of the filter applied.
    pub filter_applied: String,
    /// Bytes before filtering.
    pub bytes_before: usize,
    /// Bytes after filtering.
    pub bytes_after: usize,
    /// Secrets detected and redacted.
    pub secrets_found: usize,
    /// Whether output was truncated.
    pub truncated: bool,
}

/// Execute a command through the proxy.
pub fn execute(
    program: &str,
    args: &[String],
    cwd: &Path,
    config: &Config,
) -> ProxyResult {
    let start = Instant::now();

    // ---- Security: command gate ----
    let verdict = security::check_command(program, &config.security);
    let (blocked, block_reason) = match &verdict {
        SecurityVerdict::Allow => (false, None),
        SecurityVerdict::Deny(reason) => (true, Some(reason.clone())),
    };

    if blocked {
        let reason = block_reason.clone().unwrap_or_default();
        let entry = AuditEntry::new(
            program, args, cwd, None,
            start.elapsed().as_millis() as u64,
            0, 0, "none", &format!("deny:{}", reason), 0, false,
        );
        if let Err(e) = audit::log_entry(&entry, config) {
            eprintln!("[rtk-mine] audit log error: {}", e);
        }

        return ProxyResult {
            output: format!("[rtk-mine] BLOCKED: {}\n", reason),
            exit_code: None,
            blocked: true,
            block_reason: Some(reason),
            timed_out: false,
            filter_applied: "none".into(),
            bytes_before: 0,
            bytes_after: 0,
            secrets_found: 0,
            truncated: false,
        };
    }

    // ---- Security: path gate ----
    let path_verdict = security::check_path(cwd, &config.security);
    if let SecurityVerdict::Deny(reason) = &path_verdict {
        let entry = AuditEntry::new(
            program, args, cwd, None,
            start.elapsed().as_millis() as u64,
            0, 0, "none", &format!("deny:{}", reason), 0, false,
        );
        if let Err(e) = audit::log_entry(&entry, config) {
            eprintln!("[rtk-mine] audit log error: {}", e);
        }

        return ProxyResult {
            output: format!("[rtk-mine] BLOCKED: {}\n", reason),
            exit_code: None,
            blocked: true,
            block_reason: Some(reason.clone()),
            timed_out: false,
            filter_applied: "none".into(),
            bytes_before: 0,
            bytes_after: 0,
            secrets_found: 0,
            truncated: false,
        };
    }

    // ---- Execute the command ----
    let output = run_command(program, args, cwd, config);

    let timed_out = output.status.code().is_none() && !output.status.success();
    let exit_code = output.status.code();
    let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_stderr = if config.proxy.capture_stderr {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::new()
    };

    let raw_output = if raw_stderr.is_empty() {
        raw_stdout.clone()
    } else {
        format!("{}\n{}", raw_stdout, raw_stderr)
    };
    let bytes_before = raw_output.len();

    // ---- Apply filter ----
    let cmd_name = security::command_name(program);
    let filter_name = filters::classify(cmd_name, args);
    let (filtered, truncated) = filters::apply(&filter_name, &raw_output, config);

    // ---- Security: secret scan ----
    let SecretScan { output: secured, secrets_found, .. } =
        if config.security.redact_secrets {
            security::scan_and_redact(&filtered)
        } else {
            SecretScan { output: filtered, secrets_found: 0, redacted: false }
        };

    let bytes_after = secured.len();

    // ---- Audit log ----
    let entry = AuditEntry::new(
        program,
        args,
        cwd,
        exit_code,
        start.elapsed().as_millis() as u64,
        bytes_before,
        bytes_after,
        &filter_name,
        "allow",
        secrets_found,
        timed_out,
    );
    if let Err(e) = audit::log_entry(&entry, config) {
        eprintln!("[rtk-mine] audit log error: {}", e);
    }

    ProxyResult {
        output: secured,
        exit_code,
        blocked: false,
        block_reason: None,
        timed_out,
        filter_applied: filter_name,
        bytes_before,
        bytes_after,
        secrets_found,
        truncated,
    }
}

/// Run a command with timeout and return its output.
fn run_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    config: &Config,
) -> Output {
    let _timeout = std::time::Duration::from_secs(config.proxy.timeout_seconds);

    // Build the command with filtered environment.
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.current_dir(cwd);

    // Filter environment variables.
    for (key, value) in std::env::vars() {
        let upper = key.to_uppercase();
        let sensitive = config
            .security
            .strip_env_vars
            .iter()
            .any(|s| upper.contains(&s.to_uppercase()));
        if !sensitive {
            cmd.env(key, value);
        }
    }

    match cmd.output() {
        Ok(output) => output,
        Err(e) => Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: format!("[rtk-mine] failed to execute '{}': {}", program, e).into_bytes(),
        },
    }
}
