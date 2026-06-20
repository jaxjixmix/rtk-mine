//! Filter for `cat`, `head`, `tail`.
//!
//! Strategy: compress runs of blank lines, truncate long lines, detect binary output,
//! limit total lines/bytes.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("cat");
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

    // Detect binary content.
    if is_binary(raw) {
        return ("[cat] binary file (content suppressed)\n".into(), false);
    }

    let lines: Vec<&str> = raw.lines().collect();
    let total = lines.len();
    let truncated = total > max_lines;

    let mut output = String::new();
    let mut prev_blank = false;
    let shown = if truncated { &lines[..max_lines] } else { &lines };

    for line in shown {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if prev_blank {
                continue; // collapse consecutive blank lines.
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
            "\n[cat] truncated — {} lines total, showing first {}\n",
            total, max_lines
        ));
    }

    (output, truncated)
}

fn is_binary(content: &str) -> bool {
    let bytes = content.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    let null_count = sample.iter().filter(|&&b| b == 0).count();
    null_count > sample.len() / 8
}
