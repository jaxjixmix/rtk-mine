//! Filter for test runners: cargo test, pytest, npm test, Playwright, Vitest, Jest, go test.
//!
//! Strategy: parse test output to extract only failures and errors. Show a
//! summary count of pass/fail/skip. Strip ANSI progress bars, screenshot paths,
//! HTML report URLs, and other noise. The goal: an LLM sees only what broke,
//! not 1000 lines of passing dots.

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

    // Detect framework family.
    if is_playwright_style(&lines) {
        return filter_playwright_style(&lines, show_passing, max_failures);
    }

    // Original cargo/pytest/go parser.
    filter_cargo_pytest_style(&lines, show_passing, max_failures)
}

/// Detect Playwright / Vitest / Jest style output:
/// "Running N tests using M workers" or checkmark/cross prefixes.
fn is_playwright_style(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        let t = l.trim();
        (t.starts_with("Running ") && t.contains("tests using") && t.contains("workers"))
            || t.starts_with("✓ ")
            || t.starts_with("✗ ")
            || t.starts_with("× ")
    })
}

/// Parse Playwright / Vitest / Jest output.
fn filter_playwright_style(
    lines: &[&str],
    show_passing: bool,
    max_failures: usize,
) -> (String, bool) {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let skipped = 0u32;

    let mut failures_output = String::new();
    let mut failure_count = 0u32;
    let mut in_failure = false;
    let mut failure_block = String::new();
    let mut total_from_header: Option<u32> = None;

    // Lines to skip entirely.
    let skip_line = |t: &str| -> bool {
        t.is_empty()
            || t.starts_with("Serving HTML report")
            || t.starts_with("Press Ctrl+C")
            || t.starts_with("attachment #")
            || t.starts_with("attachment:")
            || t.starts_with("Error Context:")
            || t.starts_with("Call log:")
            || t.starts_with("Error: expect(")
            || t == "⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯"
    };

    for line in lines {
        let trimmed = line.trim();

        // "Running 18 tests using 4 workers" → extract total.
        if trimmed.starts_with("Running ") && trimmed.contains("tests using") {
            if let Some(n) = extract_first_number(trimmed) {
                total_from_header = Some(n);
            }
            continue;
        }

        // Skip noise lines.
        if skip_line(trimmed) {
            // Flush failure block if we hit a separator within failure.
            if in_failure && (trimmed.starts_with("attachment") || trimmed.starts_with("Error Context") || trimmed.starts_with("Call log")) {
                if !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push('\n');
                }
                failure_block.clear();
                in_failure = false;
            }
            continue;
        }

        // ✓ test name (duration)  → passed
        if trimmed.starts_with("✓ ") {
            passed += 1;
            if show_passing {
                failures_output.push_str(trimmed);
                failures_output.push('\n');
            }
            // Flush any pending failure block.
            if in_failure {
                if !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push('\n');
                }
                failure_block.clear();
                in_failure = false;
            }
            continue;
        }

        // ✗ or × test name (duration) → failed
        if trimmed.starts_with("✗ ") || trimmed.starts_with("× ") {
            failed += 1;
            if in_failure {
                if !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push('\n');
                }
                failure_block.clear();
            }
            if failure_count < max_failures as u32 {
                failure_count += 1;
                in_failure = true;
                failure_block = format!("FAIL: {}\n", trimmed);
            }
            continue;
        }

        // Numbered failure: "  1) [chromium] › path › test name"
        if let Some(rest) = trimmed.strip_prefix(|c: char| c == ' ' || c.is_ascii_digit())
        {
            let rest = rest.trim_start();
            if rest.starts_with(") [") || rest.starts_with(") ") {
                // Flush previous failure block.
                if in_failure && !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push('\n');
                }
                failure_block.clear();
                // Don't count here — ✗/× lines do the counting. Just start a new block.
                in_failure = true;
                failure_block = format!("FAIL: {}\n", trimmed);
                continue;
            }
        }

        // Summary lines: "  17 passed (9.6s)" or "  1 failed"
        // Pattern: optional whitespace + number + space + "passed" or "failed"
        let t = trimmed.trim();
        // Only parse if the line actually contains these keywords.
        if t.contains(" passed") && !t.contains("Error:") {
            if let Some(n) = t.split(" passed").next().and_then(extract_first_number) {
                passed = passed.max(n);
            }
        }
        if t.contains(" failed") && !t.contains("Error:") {
            if let Some(n) = t.split(" failed").next().and_then(extract_first_number) {
                failed = failed.max(n);
            }
        }
        if t.contains(" passed") || t.contains(" failed") {
            continue;
        }

        // In failure block — accumulate error details.
        if in_failure {
            // Stop collecting at certain boundaries.
            if trimmed.starts_with("---") || trimmed.starts_with("===") {
                if !failure_block.trim().is_empty() {
                    failures_output.push_str(&failure_block);
                    failures_output.push('\n');
                }
                failure_block.clear();
                in_failure = false;
                continue;
            }
            // Keep error details but skip excessively long lines.
            let display = if trimmed.len() > 300 {
                format!("{}...[truncated]", &trimmed[..300])
            } else {
                trimmed.to_string()
            };
            failure_block.push_str(&display);
            failure_block.push('\n');
        }
    }

    // Flush final failure block.
    if !failure_block.trim().is_empty() {
        failures_output.push_str(&failure_block);
    }

    // Use header total if available, otherwise sum what we counted.
    let total = total_from_header.unwrap_or(passed + failed + skipped);

    let mut output = format!(
        "[test] {} total: {} passed, {} failed\n",
        total, passed, failed
    );

    if failed > 0 {
        output.push_str("\n--- failures ---\n");
        output.push_str(&failures_output);
        if failure_count < failed {
            output.push_str(&format!(
                "\n[test] showing {}/{} failures ({} more omitted)\n",
                failure_count,
                failed,
                (failed as usize).saturating_sub(failure_count as usize)
            ));
        }
    } else if show_passing {
        output.push_str("\n--- all passing ---\n");
        output.push_str(&failures_output);
    }

    (output, false)
}

/// Original cargo test / pytest / go test parser.
fn filter_cargo_pytest_style(
    lines: &[&str],
    show_passing: bool,
    max_failures: usize,
) -> (String, bool) {
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let errors = 0u32;

    let mut failures_output = String::new();
    let mut failure_count = 0u32;

    let mut in_failure = false;
    let mut failure_block = String::new();

    for line in lines {
        let trimmed = line.trim();

        // Cargo test-style.
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

        // Pytest-style.
        if (trimmed.contains("PASSED") || trimmed.contains("passed"))
            && (trimmed.starts_with("test_") || trimmed.contains("::"))
        {
            passed += 1;
            if show_passing {
                failures_output.push_str(trimmed);
                failures_output.push('\n');
            }
            continue;
        }
        if (trimmed.contains("FAILED") || trimmed.contains("failed"))
            && (trimmed.starts_with("test_") || trimmed.contains("::"))
        {
            failed += 1;
            if failure_count < max_failures as u32 {
                failure_count += 1;
                in_failure = true;
                failure_block = format!("FAIL: {}\n", trimmed);
            }
            continue;
        }

        // Cargo test header/footer lines.
        if trimmed.starts_with("running ") && trimmed.contains("test") { continue; }
        if trimmed.starts_with("test result:") { continue; }

        // Go test-style.
        if trimmed == "PASS" || trimmed == "FAIL" || trimmed == "ok" { continue; }

        // In failure block — accumulate error details.
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

    if !failure_block.trim().is_empty() {
        failures_output.push_str(&failure_block);
    }

    let total = passed + failed + skipped + errors;
    let mut output = format!(
        "[test] {} total: {} passed, {} failed, {} skipped, {} errors\n",
        total, passed, failed, skipped, errors
    );

    if total == 0 {
        let max_lines = 100;
        let total_lines = lines.len();
        let truncated = total_lines > max_lines;
        let shown = if truncated { &lines[..max_lines] } else { lines };
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

/// Extract the first integer from a string.
fn extract_first_number(s: &str) -> Option<u32> {
    s.chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
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

    #[test]
    fn test_playwright_output() {
        let raw = "\
Running 18 tests using 4 workers

  ✓ test one (1.2s)
  ✗ test two (3.4s)

  1) [chromium] › e2e/spec.ts:16:3 › suite › test name

    Error: expect(locator).toHaveCount(expected) failed

    Expected: 7
    Received: 8

    Call log:
      - Expect \"toHaveCount\" with timeout 5000ms

    attachment #1: screenshot (image/png) ──────────────────────
    test-results/test-failed-1.png
    ───────────────────────────────────────────────────────────

    Error Context: test-results/error-context.md

  1 failed
    [chromium] › e2e/spec.ts:16:3 › suite › test name
  1 passed (4.6s)

  Serving HTML report at http://localhost:9323. Press Ctrl+C to quit.
";
        let config = Config::default();
        let (out, _) = filter(raw, &config);
        assert!(out.contains("18 total"));
        assert!(out.contains("1 passed"));
        assert!(out.contains("1 failed"));
        assert!(out.contains("FAIL:"));
        assert!(out.contains("Expected: 7"));
        assert!(!out.contains("Serving HTML report"));
        assert!(!out.contains("attachment #1"));
    }

    #[test]
    fn test_playwright_no_checkmarks() {
        // Real-world Playwright output from `npm run test:e2e` — no ✓/✗ prefixes,
        // only the failure block and summary lines.
        let raw = "\
npm run test:e2e

> test:e2e
> playwright test
[WebServer] You are using Node.js 22.9.0.

Running 18 tests using 4 workers
  1) [chromium] › e2e/dev-preview.spec.ts:16:3 › test name

    Error: expect(locator).toHaveCount(expected) failed

    Locator:  getByTestId('component-list')
    Expected: 7
    Received: 8
    Timeout:  5000ms

    Call log:
      - Expect \"toHaveCount\" with timeout 5000ms
      - waiting for getByTestId('component-list')
        9 × locator resolved to 8 elements

      18 |     await expect(items).toHaveCount(7);
         |                         ^

    attachment #1: screenshot (image/png) ──────────────────────
    test-results/test-failed-1.png
    ───────────────────────────────────────────────────────────

    Error Context: test-results/error-context.md

  1 failed
    [chromium] › e2e/dev-preview.spec.ts:16:3 › test name
  17 passed (9.6s)

  Serving HTML report at http://localhost:9323. Press Ctrl+C to quit.
";
        let config = Config::default();
        let (out, _) = filter(raw, &config);
        assert!(out.contains("18 total"), "expected '18 total' in:\n{}", out);
        assert!(out.contains("17 passed"), "expected '17 passed' in:\n{}", out);
        assert!(out.contains("1 failed"), "expected '1 failed' in:\n{}", out);
        assert!(!out.contains("Serving HTML report"));
        assert!(!out.contains("attachment #1"));
        assert!(out.contains("Expected: 7"));
    }

    #[test]
    fn test_playwright_all_passing() {
        let raw = "\
Running 5 tests using 2 workers
  ✓ test a (1s)
  ✓ test b (2s)
  ✓ test c (3s)
  ✓ test d (4s)
  ✓ test e (5s)
  5 passed (15s)
";
        let config = Config::default();
        let (out, _) = filter(raw, &config);
        assert!(out.contains("5 total"));
        assert!(out.contains("5 passed"));
        assert!(out.contains("0 failed"));
        assert!(!out.contains("FAIL:"));
    }
}
