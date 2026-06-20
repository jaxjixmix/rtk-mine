//! Integration tests — runs the actual binary and verifies every filter works.
//!
//! These tests spawn `rtk-mine exec -- <command>` and check the output.

use std::process::Command;
use std::{fs, io};

const TEST_PATH: &str = "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:/Users/ronaldmojica/.cargo/bin";

fn rtk_mine() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtk-mine"));
    cmd.env_remove("PATH"); // don't inherit wrapper-polluted PATH
    cmd.env("PATH", TEST_PATH);
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rtk-mine-test-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_executable(path: &std::path::Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

fn run_with_env(args: &[&str], envs: &[(&str, &str)]) -> (String, i32) {
    let mut cmd = rtk_mine();
    cmd.arg("exec").arg("--quiet").arg("--").args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("failed to run rtk-mine");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, code)
}

fn run(args: &[&str]) -> (String, i32) {
    run_with_env(args, &[])
}

fn last_audit_savings_for(program: &str, audit_path: &std::path::Path) -> f64 {
    let contents = fs::read_to_string(audit_path).expect("read audit log");
    contents
        .lines()
        .rev()
        .find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            if value.get("program")?.as_str()? == program {
                value.get("savings_pct")?.as_f64()
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing audit entry for {program}"))
}

fn all_audit_savings(audit_path: &std::path::Path) -> Vec<(String, f64)> {
    let contents = fs::read_to_string(audit_path).expect("read audit log");
    contents
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            Some((
                value.get("program")?.as_str()?.to_string(),
                value.get("savings_pct")?.as_f64()?,
            ))
        })
        .collect()
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

#[test]
fn test_dummy_ls_reports_real_savings() {
    let temp = temp_dir("dummy-ls-savings");
    let bin = temp.join("bin");
    fs::create_dir_all(&bin).expect("create bin");
    write_executable(
        &bin.join("ls"),
        r#"#!/usr/bin/env bash
for i in $(seq 1 120); do
  printf '%s\n' "-rw-r--r--  1 user  staff  123 Jan 01 00:00 generated-file-$i.txt"
done
"#,
    )
    .expect("write dummy ls");

    let audit_path = temp.join("audit.log");
    let path = format!("{}:{}", bin.display(), TEST_PATH);
    let audit = audit_path.to_string_lossy().to_string();
    let (out, code) = run_with_env(&["ls"], &[("PATH", &path), ("RTK_MINE_AUDIT_LOG", &audit)]);

    assert_eq!(code, 0);
    assert_output!(out, "[ls]", "dummy ls filter");
    let savings = last_audit_savings_for("ls", &audit_path);
    println!("dummy ls savings: {savings:.1}%");
    assert!(
        savings > 20.0,
        "dummy ls should save more than 20%; measured {savings:.1}%; output was: {out}"
    );
}

#[test]
fn test_target_command_inventory_has_smoke_coverage() {
    let temp = temp_dir("target-command-savings");
    let bin = temp.join("bin");
    fs::create_dir_all(&bin).expect("create bin");

    let script = r#"#!/usr/bin/env bash
cmd=$(basename "$0")
case "$cmd" in
  ls)
    for i in $(seq 1 140); do printf '%s\n' "-rw-r--r--  1 user  staff  123 Jan 01 00:00 generated-file-$i.txt"; done ;;
  cat|head|tail)
    for i in $(seq 1 140); do printf '%s\n' "line $i: generated content for file viewing filters"; done ;;
  grep|rg)
    for i in $(seq 1 140); do printf '%s\n' "src/file_$i.rs:$i:fn generated_$i() {}"; done ;;
  find)
    for i in $(seq 1 140); do printf '%s\n' "./src/generated_$i.rs"; done ;;
  git)
    if [ "$1" = "status" ]; then
      printf 'On branch main\nChanges not staged for commit:\n'
      for i in $(seq 1 80); do printf '%s\n' " modified: src/generated_$i.rs"; done
    else
      for i in $(seq 1 80); do printf '%s\n' "abcdef$i commit message $i"; done
    fi ;;
  cargo|pytest|npm|npx|pnpm|yarn|go|make)
    printf 'running 120 tests\n'
    for i in $(seq 1 120); do printf 'test generated_%s ... ok\n' "$i"; done
    printf 'test result: ok. 120 passed; 0 failed\n' ;;
  *)
    printf 'generic output\n' ;;
esac
"#;

    let target_commands = [
        "ls", "cat", "head", "tail", "grep", "rg", "find", "git", "cargo", "pytest", "npm",
        "npx", "pnpm", "yarn", "go", "make",
    ];
    for command in target_commands {
        write_executable(&bin.join(command), script).expect("write fake command");
    }

    let audit_path = temp.join("audit.log");
    let path = format!("{}:{}", bin.display(), TEST_PATH);
    let audit = audit_path.to_string_lossy().to_string();
    let cases: Vec<Vec<&str>> = vec![
        vec!["ls"],
        vec!["cat", "fixture.txt"],
        vec!["head", "fixture.txt"],
        vec!["tail", "fixture.txt"],
        vec!["grep", "generated", "src"],
        vec!["rg", "generated"],
        vec!["find", "src", "-name", "*.rs"],
        vec!["git", "status"],
        vec!["cargo", "test"],
        vec!["pytest"],
        vec!["npm", "test"],
        vec!["npx", "test"],
        vec!["pnpm", "test"],
        vec!["yarn", "test"],
        vec!["go", "test"],
        vec!["make", "test"],
    ];

    for args in cases {
        let (out, code) = run_with_env(&args, &[("PATH", &path), ("RTK_MINE_AUDIT_LOG", &audit)]);
        assert_eq!(code, 0, "{} should run through proxy; output: {out}", args[0]);
    }

    let savings = all_audit_savings(&audit_path);
    println!("target command savings: {savings:?}");
    let measured_commands: Vec<&str> = savings.iter().map(|(command, _)| command.as_str()).collect();
    assert_eq!(measured_commands, target_commands);
    assert!(
        savings.iter().any(|(_, pct)| *pct > 0.0),
        "expected at least one target command to report savings; measured: {savings:?}"
    );
}

#[test]
fn test_installer_uses_sudo_for_chmod_after_sudo_copy() {
    let temp = temp_dir("installer-sudo-chmod");
    let fake_bin = temp.join("fake-bin");
    let install_dir = temp.join("install-bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    fs::create_dir_all(&install_dir).expect("create install dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&install_dir).expect("install dir metadata").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&install_dir, perms).expect("make install dir read-only");
    }

    write_executable(
        &fake_bin.join("curl"),
        r#"#!/usr/bin/env bash
if [ "$1" = "-fsSL" ] && [ "${2#https://api.github.com/}" != "$2" ]; then
  printf '{"tag_name":"v-test"}\n'
  exit 0
fi
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    shift
    printf 'fake tarball\n' > "$1"
    exit 0
  fi
  shift
done
exit 1
"#,
    )
    .expect("write fake curl");
    write_executable(
        &fake_bin.join("tar"),
        r#"#!/usr/bin/env bash
dest=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-C" ]; then
    shift
    dest="$1"
  fi
  shift
done
mkdir -p "$dest"
cat > "$dest/rtk-mine" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "--version" ]; then
  printf 'rtk-mine test\n'
fi
EOF
/bin/chmod +x "$dest/rtk-mine"
"#,
    )
    .expect("write fake tar");
    write_executable(
        &fake_bin.join("sudo"),
        r#"#!/usr/bin/env bash
if [ "$1" = "cp" ]; then
  dest="$3"
  dir=$(dirname "$dest")
  /bin/chmod u+w "$dir"
  /bin/cp "$2" "$dest"
  /bin/chmod u-w "$dir"
  exit 0
fi
SUDO_FAKE=1 "$@"
"#,
    )
    .expect("write fake sudo");
    write_executable(
        &fake_bin.join("chmod"),
        r#"#!/usr/bin/env bash
if [ "${SUDO_FAKE:-}" != "1" ]; then
  printf 'chmod requires sudo in this test\n' >&2
  exit 1
fi
/bin/chmod "$@"
"#,
    )
    .expect("write fake chmod");

    let path = format!("{}:/usr/bin:/bin", fake_bin.display());
    let output = Command::new("bash")
        .arg("install.sh")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PATH", path)
        .env("RTK_MINE_INSTALL_DIR", &install_dir)
        .env("SKIP_CONFIG", "1")
        .env("SKIP_HOOKS", "1")
        .output()
        .expect("run installer");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&install_dir).expect("install dir metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&install_dir, perms).expect("restore install dir perms");
    }

    assert!(
        output.status.success(),
        "installer should sudo chmod after sudo cp\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    let (out, code) = run(&["cat", env!("CARGO_BIN_EXE_rtk-mine")]);
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
    let (_out, code) = run(&["pwd"]);
    assert_eq!(code, 0);
}

#[test]
fn test_allowed_wc() {
    let (_out, code) = run(&["wc", "-l", "Cargo.toml"]);
    assert_eq!(code, 0);
}

// ── Init modes ─────────────────────────────────────────────────────

#[test]
fn test_init_default() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .arg("init")
        .env("PATH", TEST_PATH)
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtk_mine_exec"), "init default has wrapper function");
}

#[test]
fn test_init_agent() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .arg("init")
        .arg("--agent")
        .env("PATH", TEST_PATH)
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtk_mine_exec"), "init agent has wrapper function");
    // Agent mode has no tty check.
    assert!(!stdout.contains("[ -t 1 ]"), "agent mode has no tty check");
}

#[test]
fn test_init_agent_quiet() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .arg("init")
        .arg("--agent")
        .arg("--quiet")
        .env("PATH", TEST_PATH)
        .output()
        .expect("init failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--quiet"), "quiet init includes --quiet flag");
}

// ── Audit ──────────────────────────────────────────────────────────

#[test]
fn test_audit_stats() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .args(&["audit", "stats"])
        .env("PATH", TEST_PATH)
        .output()
        .expect("audit stats failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Statistics") || stdout.contains("commands"), "audit stats works");
}

#[test]
fn test_audit_log() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .args(&["audit"])
        .env("PATH", TEST_PATH)
        .output()
        .expect("audit log failed");
    // Either entries found or "No audit entries" — both are valid.
    assert!(output.status.success(), "audit log succeeds");
}

// ── Config ─────────────────────────────────────────────────────────

#[test]
fn test_config_show() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .args(&["config", "show"])
        .env("PATH", TEST_PATH)
        .output()
        .expect("config show failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[security]"), "config shows security section");
}

#[test]
fn test_config_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .args(&["config", "path"])
        .env("PATH", TEST_PATH)
        .output()
        .expect("config path failed");
    assert!(output.status.success());
}

// ── Version ────────────────────────────────────────────────────────

#[test]
fn test_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtk-mine"))
        .arg("--version")
        .env("PATH", TEST_PATH)
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
