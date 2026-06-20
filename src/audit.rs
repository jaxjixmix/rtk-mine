//! Audit module: structured, append-only JSON Lines logging of every proxied command.
//!
//! Each audit entry captures: what ran, when, the security verdict, bytes before/after
//! filtering, any secrets detected, and the exit code. Logs rotate based on retention
//! policy. The `audit` subcommand provides querying.

use crate::config::{AuditConfig, Config};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// ISO-8601 timestamp.
    pub timestamp: DateTime<Utc>,
    /// Unique entry ID.
    pub id: String,
    /// The program that was executed.
    pub program: String,
    /// Full argument list.
    pub args: Vec<String>,
    /// Working directory at time of execution.
    pub cwd: String,
    /// Exit code (None if command was blocked).
    pub exit_code: Option<i32>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Raw output bytes before filtering.
    pub bytes_before: usize,
    /// Filtered output bytes after filtering.
    pub bytes_after: usize,
    /// Token savings percentage.
    pub savings_pct: f64,
    /// Which filter was applied.
    pub filter_applied: String,
    /// Security verdict: "allow" or "deny:reason".
    pub security_verdict: String,
    /// Number of secrets detected and redacted.
    pub secrets_found: usize,
    /// Whether the command timed out.
    pub timed_out: bool,
}

impl AuditEntry {
    /// Create a new entry with current timestamp.
    pub fn new(
        program: &str,
        args: &[String],
        cwd: &Path,
        exit_code: Option<i32>,
        duration_ms: u64,
        bytes_before: usize,
        bytes_after: usize,
        filter_applied: &str,
        security_verdict: &str,
        secrets_found: usize,
        timed_out: bool,
    ) -> Self {
        let savings_pct = if bytes_before > 0 {
            let saved = bytes_before.saturating_sub(bytes_after);
            (saved as f64 / bytes_before as f64) * 100.0
        } else {
            0.0
        };

        Self {
            timestamp: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
            program: program.to_string(),
            args: args.to_vec(),
            cwd: cwd.display().to_string(),
            exit_code,
            duration_ms,
            bytes_before,
            bytes_after,
            savings_pct,
            filter_applied: filter_applied.to_string(),
            security_verdict: security_verdict.to_string(),
            secrets_found,
            timed_out,
        }
    }
}

/// Append an audit entry to the log file.
pub fn log_entry(entry: &AuditEntry, config: &Config) -> std::io::Result<()> {
    if !config.audit.enabled {
        return Ok(());
    }

    let log_path = config.audit_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let line = serde_json::to_string(entry).unwrap_or_default();
        writeln!(file, "{}", line)?;
        // file is dropped here — data is flushed to disk.
    }

    // Rotate if needed.
    rotate_if_needed(&log_path, &config.audit)?;

    Ok(())
}

/// Rotate the audit log if it exceeds the retention policy.
fn rotate_if_needed(log_path: &Path, audit_config: &AuditConfig) -> std::io::Result<()> {
    if audit_config.max_entries == 0 && audit_config.retention_days == 0 {
        return Ok(());
    }

    // If the log doesn't exist (first run), nothing to rotate.
    if !log_path.exists() {
        return Ok(());
    }

    // Read all entries.
    let file = File::open(log_path)?;
    let reader = BufReader::new(file);
    let mut entries: Vec<AuditEntry> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
            entries.push(entry);
        }
    }

    let cutoff = Utc::now()
        - chrono::Duration::days(audit_config.retention_days as i64);

    // Filter by retention.
    if audit_config.retention_days > 0 {
        entries.retain(|e| e.timestamp >= cutoff);
    }

    // Filter by max entries.
    if audit_config.max_entries > 0 && entries.len() > audit_config.max_entries {
        let skip = entries.len() - audit_config.max_entries;
        entries.drain(0..skip);
    }

    // Write back.
    let mut file = std::fs::File::create(log_path)?;
    for entry in &entries {
        let line = serde_json::to_string(entry).unwrap_or_default();
        writeln!(file, "{}", line)?;
    }

    Ok(())
}

/// Query audit log entries.
pub fn query_entries(
    config: &Config,
    program_filter: Option<&str>,
    limit: usize,
) -> std::io::Result<Vec<AuditEntry>> {
    let log_path = config.audit_path();
    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&log_path)?;
    let reader = BufReader::new(file);
    let mut entries: Vec<AuditEntry> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
            if let Some(prog) = program_filter {
                if entry.program != prog {
                    continue;
                }
            }
            entries.push(entry);
        }
    }

    entries.reverse(); // newest first.
    entries.truncate(limit);
    Ok(entries)
}

/// Compute aggregate stats from audit entries.
#[derive(Debug, Serialize)]
pub struct AuditStats {
    pub total_commands: usize,
    pub total_bytes_saved: u64,
    pub avg_savings_pct: f64,
    pub total_secrets_redacted: usize,
    pub blocked_commands: usize,
    pub timed_out: usize,
    pub top_commands: Vec<(String, usize)>,
}

/// Compute statistics from audit log.
pub fn compute_stats(config: &Config) -> std::io::Result<AuditStats> {
    let entries = query_entries(config, None, 10_000)?;

    let total_commands = entries.len();
    let total_bytes_saved: u64 = entries
        .iter()
        .map(|e| (e.bytes_before.saturating_sub(e.bytes_after)) as u64)
        .sum();
    let avg_savings_pct = if total_commands > 0 {
        entries.iter().map(|e| e.savings_pct).sum::<f64>() / total_commands as f64
    } else {
        0.0
    };
    let total_secrets_redacted = entries.iter().map(|e| e.secrets_found).sum();
    let blocked_commands = entries
        .iter()
        .filter(|e| e.security_verdict.starts_with("deny"))
        .count();
    let timed_out = entries.iter().filter(|e| e.timed_out).count();

    let mut cmd_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for entry in &entries {
        *cmd_counts.entry(entry.program.clone()).or_default() += 1;
    }
    let mut top_commands: Vec<(String, usize)> = cmd_counts.into_iter().collect();
    top_commands.sort_by(|a, b| b.1.cmp(&a.1));
    top_commands.truncate(10);

    Ok(AuditStats {
        total_commands,
        total_bytes_saved,
        avg_savings_pct,
        total_secrets_redacted,
        blocked_commands,
        timed_out,
        top_commands,
    })
}
