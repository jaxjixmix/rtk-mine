//! Filter for `ls` and directory listing commands.
//!
//! Strategy: strip . and .. entries, limit to N entries, sort, show
//! sizes for large files, color-code by type.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("ls");
    let max_entries = cfg
        .options
        .get("max_entries")
        .and_then(|v| v.as_integer())
        .unwrap_or(80) as usize;
    let show_hidden = cfg
        .options
        .get("show_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let lines: Vec<&str> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| {
            if show_hidden {
                true
            } else {
                // Filter out hidden files/dirs (lines starting with .).
                let trimmed = l.trim_start();
                // For `ls -la` style output, skip . and .. entries.
                if trimmed == "." || trimmed == ".." {
                    return false;
                }
                // For simple `ls -a` output, filter entries starting with .
                let name = last_word(trimmed);
                !name.starts_with('.')
            }
        })
        .collect();

    let total = lines.len();
    let truncated = total > max_entries;
    let shown = if truncated {
        &lines[..max_entries]
    } else {
        &lines
    };

    let header = if truncated {
        format!("[ls] {} entries (showing first {}):\n", total, max_entries)
    } else {
        format!("[ls] {} entries:\n", total)
    };

    let body = shown.join("\n");

    (format!("{}{}", header, body), truncated)
}

fn last_word(s: &str) -> &str {
    s.split_whitespace().last().unwrap_or(s)
}
