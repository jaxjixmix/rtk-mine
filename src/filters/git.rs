//! Filter for `git` subcommands: status, diff, log, branch.
//!
//! Strategy: collapse verbose output to structured summaries — file counts by
//! status for `git status`, file-level stats + truncated diffs for `git diff`,
//! one-line summaries for `git log`.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("git");
    let max_lines = cfg.max_lines.max(
        cfg.options
            .get("max_lines")
            .and_then(|v| v.as_integer())
            .unwrap_or(200) as usize,
    );

    let lines: Vec<&str> = raw.lines().collect();
    let total = lines.len();

    // Detect what kind of git output this is.
    let is_status = lines.iter().any(|l| l.contains("Changes not staged")
        || l.contains("Changes to be committed")
        || l.contains("Untracked files")
        || l.contains("nothing to commit"));
    let is_diff = lines.iter().any(|l| l.starts_with("diff --git"));
    let is_log = lines.iter().any(|l| l.starts_with("commit "));
    let is_branch = lines.iter().any(|l| l.starts_with('*') || l.starts_with("  "));

    if is_status {
        return filter_status(&lines, total, max_lines);
    }
    if is_diff {
        return filter_diff(&lines, total, max_lines);
    }
    if is_log {
        return filter_log(&lines, total, max_lines);
    }
    if is_branch {
        return filter_branch(&lines, total);
    }

    // Fallback: just limit output lines.
    generic_lines(&lines, total, max_lines, "git")
}

fn filter_status(lines: &[&str], _total: usize, max_lines: usize) -> (String, bool) {
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;
    let mut files: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("On branch")
            || trimmed.starts_with("Your branch")
            || trimmed.starts_with("(use ")
            || trimmed.starts_with("no changes")
            || trimmed.starts_with("nothing to commit")
            || trimmed.starts_with("nothing added")
            || trimmed.starts_with("No commits yet")
            || trimmed == "Untracked files:"
            || trimmed == "Changes not staged for commit:"
            || trimmed == "Changes to be committed:"
        {
            if trimmed.contains("nothing to commit") {
                return ("[git status] clean — nothing to commit\n".into(), false);
            }
            continue;
        }
        if trimmed.starts_with("modified:") || trimmed.starts_with("new file:") || trimmed.starts_with("deleted:") || trimmed.starts_with("renamed:") {
            staged += 1;
            files.push(trimmed.to_string());
        } else if trimmed.starts_with("\tmodified:") || trimmed.starts_with("\tdeleted:") {
            unstaged += 1;
            files.push(format!("  {}", trimmed.trim()));
        } else if !trimmed.starts_with("(") {
            untracked += 1;
            files.push(format!("  ?? {}", trimmed));
        }
    }

    let total_files = staged + unstaged + untracked;
    let mut output = format!(
        "[git status] {} files ({} staged, {} unstaged, {} untracked):\n",
        total_files, staged, unstaged, untracked
    );

    let truncated = files.len() > max_lines;
    let shown = if truncated { &files[..max_lines] } else { &files };
    for f in shown {
        output.push_str(f);
        output.push('\n');
    }

    if truncated {
        output.push_str(&format!(
            "\n[git status] truncated — showing {}/{} files\n",
            max_lines, files.len()
        ));
    }

    (output, truncated)
}

fn filter_diff(lines: &[&str], _total: usize, max_lines: usize) -> (String, bool) {
    let mut files_changed = Vec::new();
    let mut current_file = String::new();
    let mut hunk_lines = 0u32;
    let mut output = String::new();
    let mut truncated = false;

    for line in lines {
        if line.starts_with("diff --git") {
            if !current_file.is_empty() {
                files_changed.push((current_file.clone(), hunk_lines));
            }
            current_file = line
                .strip_prefix("diff --git a/")
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("?")
                .to_string();
            hunk_lines = 0;
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("index ") {
            continue;
        }
        hunk_lines += 1;
        // Keep actual diff content (limited).
        if output.lines().count() < max_lines {
            output.push_str(line);
            output.push('\n');
        } else {
            truncated = true;
        }
    }
    if !current_file.is_empty() {
        files_changed.push((current_file, hunk_lines));
    }

    let mut summary = format!(
        "[git diff] {} files changed:\n",
        files_changed.len()
    );
    for (file, lines) in &files_changed {
        summary.push_str(&format!("  {} (+{} lines)\n", file, lines));
    }

    if !output.is_empty() {
        summary.push_str("\n--- diff preview ---\n");
        summary.push_str(&output);
    }

    if truncated {
        summary.push_str("\n[git diff] truncated — use larger diff limits for full content\n");
    }

    (summary, truncated)
}

fn filter_log(lines: &[&str], _total: usize, max_lines: usize) -> (String, bool) {
    let commits: Vec<&str> = lines
        .iter()
        .filter(|l| l.starts_with("commit ") || l.starts_with("Author:") || l.starts_with("Date:") || l.trim().is_empty() || !l.starts_with(" "))
        .copied()
        .collect();

    let truncated = commits.len() > max_lines;
    let shown = if truncated { &commits[..max_lines] } else { &commits };

    let mut output = format!("[git log] {} commits:\n", commits.len());
    for line in shown {
        if !line.trim().is_empty() || !output.ends_with("\n\n") {
            output.push_str(line);
            output.push('\n');
        }
    }

    (output, truncated)
}

fn filter_branch(lines: &[&str], total: usize) -> (String, bool) {
    let branches: Vec<&str> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();

    let mut output = format!("[git branch] {} branches:\n", branches.len());
    for b in &branches {
        output.push_str(b);
        output.push('\n');
    }

    (output, total > 50)
}

fn generic_lines(lines: &[&str], total: usize, max_lines: usize, tag: &str) -> (String, bool) {
    let truncated = total > max_lines;
    let shown = if truncated { &lines[..max_lines] } else { &lines };

    let mut output = format!("[{}] {} lines:\n", tag, total);
    for line in shown {
        output.push_str(line);
        output.push('\n');
    }

    if truncated {
        output.push_str(&format!(
            "\n[{}] truncated — {}/{} lines shown\n",
            tag, max_lines, total
        ));
    }

    (output, truncated)
}
