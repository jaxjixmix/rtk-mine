//! Generic / fallback filter for commands without a specialized filter.
//!
//! Strategy: strip ANSI, limit total lines, compress blank lines, truncate
//! long lines. Safe default that still reduces token usage significantly.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("generic");
    let max_lines = cfg.max_lines.max(
        cfg.options
            .get("max_lines")
            .and_then(|v| v.as_integer())
            .unwrap_or(200) as usize,
    );
    let max_line_len = cfg
        .options
        .get("max_line_length")
        .and_then(|v| v.as_integer())
        .unwrap_or(500) as usize;

    let clean = strip_ansi_codes(raw);
    let lines: Vec<&str> = clean.lines().collect();
    let total = lines.len();
    let truncated = total > max_lines;

    let shown = if truncated { &lines[..max_lines] } else { &lines };

    let mut output = String::new();
    let mut prev_blank = false;

    for line in shown {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if prev_blank {
                continue;
            }
            prev_blank = true;
            output.push('\n');
        } else {
            prev_blank = false;
            let display = if line.len() > max_line_len {
                format!("{}...[truncated]\n", &line[..max_line_len])
            } else {
                format!("{}\n", line)
            };
            output.push_str(&display);
        }
    }

    if truncated {
        output.push_str(&format!(
            "\n[generic] truncated — {} lines total, showing first {}\n",
            total, max_lines
        ));
    }

    (output, truncated)
}

fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}
