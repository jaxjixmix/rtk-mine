//! Integration tests — runs the actual binary and verifies every filter works.
//!
//! These tests spawn `rtk-mine exec -- <command>` and check the output.

use std::process::Command;

fn rtk_mine() -> Command {
    let mut cmd = Command::new("./target/release/rtk-mine");
    cmd.env_remove("PATH"); // don't inherit wrapper-polluted PATH
    cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin");
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn run(args: &[&str]) -> (String, i32) {
    let output = rtk_mine()
        .arg("exec")
        .arg("--quiet")
        .arg("--")
        .args(args)
        .output()
        .expect("failed to run rtk-mine");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, code)
}

// Macro to make test assertions cleaner.
macro_rules! assert_output {
    ($stdout:expr, $contains:expr, $test_name:expr) => {
        assert!(
            $stdout.contains($contains),
            "{} failed: expected output to contain '{}'\nGot: {}",
            $test_name,
            $contains,
            $stdout
        );
    };
}

// ── ls filter ──────────────────────────────────────────────────────

#[test]
fn test_ls_filter() {
    let (out, code) = run(&["ls", "-la", "src/"]);
    assert_eq!(code, 0);
    assert_output!(out, "[ls]", "ls filter");
    assert_output!(out, "entries", "ls count");
    assert_output!(out, "main.rs", "ls lists main.rs");
}

// ── cat filter ─────────────────────────────────────────────────────

#[test]
fn test_cat_filter() {
    let (out, code) = run(&["cat", "Cargo.toml"]);
    assert_eq!(code, 0);
    assert_output!(out, "rtk-mine", "cat shows package name");
}

#[test]
fn test_cat_binary_detection() {
    let (out, code) = run(&["cat", "target/release/rtk-mine"]);
    assert_eq!(code, 0);
    assert_output!(out, "[cat] binary file", "cat detects binary");
}

// ── grep filter ────────────────────────────────────────────────────

#[test]
fn test_grep_filter() {
    let (out, code) = run(&["grep", "-r", "fn ", "src/"]);
    assert_eq!(code, 0);
    assert_output!(out, "[grep]", "grep header");
    assert_output!(out, "matches", "grep count");
}

// ── find filter ────────────────────────────────────────────────────

#[test]
fn test_find_filter() {
    let (out, code) = run(&["find", "src", "-name", "*.rs"]);
    assert_eq!(code, 0);
    assert_output!(out, "[find]", "find header");
    assert_output!(out, "results", "find count");
}

// ── git filter ─────────────────────────────────────────────────────

#[test]
fn test_git_status_filter() {
    let (out, code) = run(&["git", "status"]);
    assert_eq!(code, 0);
    assert_output!(out, "[git status]", "git status header");
}

#[test]
fn test_git_log_filter() {
    let (out, code) = run(&["git", "log", "--oneline", "-5"]);
    assert_eq!(code, 0);
    assert_output!(out, "[git", "git log header");
}

// ── test filter (cargo) ────────────────────────────────────────────

#[test]
fn test_cargo_test_filter() {
    let (out, _code) = run(&["cargo", "test", "--lib"]);
    // exit code may vary, just check the filter header — or raw fallback.
    assert!(out.contains("[test]") || out.contains("test result") || out.contains("could not parse"),
        "cargo test filter: expected test output, got: {}", out);
}

// ── generic filter ─────────────────────────────────────────────────

#[test]
fn test_generic_filter() {
    let (out, code) = run(&["echo", "hello world"]);
    assert_eq!(code, 0);
    // echo goes through generic — just verify it runs
    assert!(!out.is_empty(), "echo produces output");
}

// ── Security: deny list ────────────────────────────────────────────

#[test]
fn test_deny_sudo() {
    let (out, code) = run(&["sudo", "ls"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "sudo is denied");
}

#[test]
fn test_deny_curl() {
    let (out, code) = run(&["curl", "localhost"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "curl is denied");
}

#[test]
fn test_deny_ssh() {
    let (out, code) = run(&["ssh", "localhost"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "ssh is denied");
}

#[test]
fn test_deny_rm() {
    let (out, code) = run(&["rm", "nonexistent"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "rm is denied");
}

#[test]
fn test_deny_sh() {
    let (out, code) = run(&["sh", "-c", "echo hi"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "sh is denied");
}

// ── Security: command-runner scanning ──────────────────────────────

#[test]
fn test_deny_env_curl() {
    let (out, code) = run(&["env", "curl", "localhost"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "env curl is blocked");
}

#[test]
fn test_deny_env_bash() {
    let (out, code) = run(&["env", "bash"]);
    assert_ne!(code, 0);
    assert_output!(out, "BLOCKED", "env bash is blocked");
}

// ── Security: secret redaction ─────────────────────────────────────

#[test]
fn test_secret_redaction_openai_key() {
    let (out, code) = run(&["echo", "OPENAI_API_KEY=sk-abc123def456ghi789jkl012mno345pqr678stu"]);
    assert_eq!(code, 0);
    assert_output!(out, "REDACTED", "OpenAI key redacted");
    assert!(!out.contains("sk-abc123"), "secret value not in output");
}

// ── Safety: allowed commands work ──────────────────────────────────

#[test]
fn test_allowed_echo() {
    let (out, code) = run(&["echo", "hello"]);
    assert_eq!(code, 0);
    assert_output!(out, "hello", "echo works");
}

#[test]
fn test_allowed_pwd() {
    let (out, code) = run(&["pwd"]);
    assert_eq!(code, 0);
}

#[test]
fn test_allowed_wc() {
    let (out, code) = run(&["wc", "-l", "Cargo.toml"]);
    assert_eq!(code, 0);
}

// ── Init modes ─────────────────────────────────────────────────────

#[test]
fn test_init_default() {
    let output = Command::new("./target/release/rtk-mine")
        .arg("init")
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtk_mine_exec"), "init default has wrapper function");
}

#[test]
fn test_init_agent() {
    let output = Command::new("./target/release/rtk-mine")
        .arg("init")
        .arg("--agent")
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtk_mine_exec"), "init agent has wrapper function");
    // Agent mode has no tty check.
    assert!(!stdout.contains("[ -t 1 ]"), "agent mode has no tty check");
}

#[test]
fn test_init_agent_quiet() {
    let output = Command::new("./target/release/rtk-mine")
        .arg("init")
        .arg("--agent")
        .arg("--quiet")
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--quiet"), "quiet init includes --quiet flag");
}

// ── Audit ──────────────────────────────────────────────────────────

#[test]
fn test_audit_stats() {
    let output = Command::new("./target/release/rtk-mine")
        .args(&["audit", "stats"])
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("audit stats failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Statistics") || stdout.contains("commands"), "audit stats works");
}

#[test]
fn test_audit_log() {
    let output = Command::new("./target/release/rtk-mine")
        .args(&["audit"])
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("audit log failed");
    // Either entries found or "No audit entries" — both are valid.
    assert!(output.status.success(), "audit log succeeds");
}

// ── Config ─────────────────────────────────────────────────────────

#[test]
fn test_config_show() {
    let output = Command::new("./target/release/rtk-mine")
        .args(&["config", "show"])
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("config show failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[security]"), "config shows security section");
}

#[test]
fn test_config_path() {
    let output = Command::new("./target/release/rtk-mine")
        .args(&["config", "path"])
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("config path failed");
    assert!(output.status.success());
}

// ── Version ────────────────────────────────────────────────────────

#[test]
fn test_version() {
    let output = Command::new("./target/release/rtk-mine")
        .arg("--version")
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin")
        .output()
        .expect("version failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtk-mine"), "version shows name");
}

// ── Exit codes ─────────────────────────────────────────────────────

#[test]
fn test_blocked_exit_code() {
    let output = rtk_mine()
        .args(&["exec", "--quiet", "--", "sudo", "ls"])
        .output()
        .expect("exec failed");
    assert_ne!(output.status.code(), Some(0), "blocked commands exit non-zero");
}

#[test]
fn test_missing_command() {
    let output = rtk_mine()
        .args(&["exec", "--quiet", "--", "nonexistent_cmd_xyz"])
        .output()
        .expect("exec failed");
    // Should still exit even if the command fails to spawn.
    let _ = output.status.code();
}
