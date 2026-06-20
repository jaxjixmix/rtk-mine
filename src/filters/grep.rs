//! Filter for `grep`, `rg`, `ag`.
//!
//! Strategy: strip ANSI codes, limit number of matches, add a summary count,
//! keep 1 line of context (not 3).

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("grep");
    let max_matches = cfg
        .options
        .get("max_matches")
        .and_then(|v| v.as_integer())
        .unwrap_or(50) as usize;

    let clean = strip_ansi_codes(raw);
    let lines: Vec<&str> = clean.lines().collect();

    // Separate match lines from context/separator lines.
    let mut matches = 0usize;
    let mut output = String::new();
    let mut truncated = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "--" || trimmed.is_empty() {
            if matches > 0 && !truncated {
                output.push_str("---\n");
            }
            continue;
        }

        // Count as a match if it's a filename:line:content pattern or raw match.
        if matches >= max_matches {
            truncated = true;
            break;
        }

        output.push_str(line);
        output.push('\n');
        matches += 1;
    }

    let header = if truncated {
        format!(
            "[grep] {} matches (showing first {}):\n",
            lines.len(),
            max_matches
        )
    } else {
        format!("[grep] {} matches:\n", matches)
    };

    (format!("{}{}", header, output), truncated)
}

/// Strip ANSI escape codes from output.
fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}
