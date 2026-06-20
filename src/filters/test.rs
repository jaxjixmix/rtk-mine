//! Filter for test runners: cargo test, pytest, npm test, go test, etc.
//!
//! Strategy: parse test output to extract only failures and errors. Show a
//! summary count of pass/fail/skip. Strip ANSI progress bars and decorations.
//! The goal: an LLM sees only what broke, not 1000 lines of passing dots.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("test");
    let show_passing = cfg
        .options
        .get("show_passing")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_failures = cfg
        .options
        .get("max_failures")
        .and_then(|v| v.as_integer())
        .unwrap_or(50) as usize;

    let clean = strip_ansi_codes(raw);
    let lines: Vec<&str> = clean.lines().collect();

    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let errors = 0u32;

    let mut failures_output = String::new();
    let mut failure_count = 0u32;

    let mut in_failure = false;
    let mut failure_block = String::new();

    for line in &lines {
        let trimmed = line.trim();

        // Detect test framework by format.
        if trimmed.starts_with("test ") && trimmed.contains("... ok") {
            passed += 1;
            if show_passing {
                failures_output.push_str(trimmed);
                failures_output.push('\n');
            }
            continue;
        }

        if trimmed.starts_with("test ") && trimmed.contains("... FAILED") {
            failed += 1;
            if failure_count < max_failures as u32 {
                failure_count += 1;
                in_failure = true;
                failure_block = format!("FAIL: {}\n", trimmed);
            }
            continue;
        }

        if trimmed.starts_with("test ") && trimmed.contains("... ignored") {
            skipped += 1;
            continue;
        }

        // Pytest-style output.
        if trimmed.contains("PASSED") || trimmed.contains("passed") {
            if trimmed.starts_with("test_") || trimmed.contains("::") {
                passed += 1;
                if show_passing {
                    failures_output.push_str(trimmed);
                    failures_output.push('\n');
                }
                continue;
            }
        }
        if trimmed.contains("FAILED") || trimmed.contains("failed") {
            if trimmed.starts_with("test_") || trimmed.contains("::") {
                failed += 1;
                if failure_count < max_failures as u32 {
                    failure_count += 1;
                    in_failure = true;
                    failure_block = format!("FAIL: {}\n", trimmed);
                }
                continue;
            }
        }

        // Cargo test-style: "running X tests" as summary line.
        if trimmed.starts_with("running ") && trimmed.contains("test") {
            continue;
        }
        if trimmed.starts_with("test result:") {
            // Parse: "test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured"
            continue; // we compute our own summary.
        }

        // Go test-style.
        if trimmed == "PASS" || trimmed == "FAIL" || trimmed == "ok" {
            continue;
        }

        // In failure block.
        if in_failure {
            if trimmed.is_empty() || trimmed.starts_with("----") || trimmed.starts_with("note:") {
                if !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push_str("\n---\n\n");
                }
                failure_block.clear();
                in_failure = false;
            } else {
                failure_block.push_str(trimmed);
                failure_block.push('\n');
            }
        }
    }

    // Flush any remaining failure block.
    if !failure_block.trim().is_empty() {
        failures_output.push_str(&failure_block);
    }

    // Build summary.
    let total = passed + failed + skipped + errors;
    let mut output = format!(
        "[test] {} total: {} passed, {} failed, {} skipped, {} errors\n",
        total, passed, failed, skipped, errors
    );

    if total == 0 {
        // Couldn't parse test output — fallback to truncated raw output.
        let max_lines = 100;
        let total_lines = lines.len();
        let truncated = total_lines > max_lines;
        let shown = if truncated { &lines[..max_lines] } else { &lines };
        output.push_str("[test] (could not parse test framework, showing raw):\n");
        for l in shown {
            output.push_str(l);
            output.push('\n');
        }
        return (output, truncated);
    }

    if failed > 0 || errors > 0 {
        output.push_str("\n--- failures ---\n");
        output.push_str(&failures_output);
        if failure_count < failed + errors {
            output.push_str(&format!(
                "\n[test] showing {}/{} failures ({} more omitted)\n",
                failure_count,
                failed + errors,
                (failed + errors).saturating_sub(failure_count)
            ));
        }
    } else if show_passing {
        output.push_str("\n--- all passing ---\n");
        output.push_str(&failures_output);
    }

    (output, false)
}

/// Strip ANSI escape codes.
fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_test_output() {
        let raw = "\
running 5 tests
test test_add ... ok
test test_sub ... FAILED
test test_mul ... ok
test test_div ... ok
test test_mod ... ignored
test result: FAILED. 3 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out;
";
        let config = Config::default();
        let (out, _) = filter(raw, &config);
        assert!(out.contains("5 total"));
        assert!(out.contains("3 passed"));
        assert!(out.contains("1 failed"));
        assert!(out.contains("FAIL: test test_sub ... FAILED"));
    }
}
