//! Filter for `find`, `fd`, `locate`.
//!
//! Strategy: deduplicate paths, strip leading `./`, sort, limit entries.

use crate::config::Config;

pub fn filter(raw: &str, config: &Config) -> (String, bool) {
    let cfg = config.filter_for("find");
    let max_entries = cfg
        .options
        .get("max_entries")
        .and_then(|v| v.as_integer())
        .unwrap_or(100) as usize;

    let paths: Vec<&str> = raw
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("[find]"))
        .map(|l| l.trim().trim_start_matches("./"))
        .collect();

    let total = paths.len();
    let truncated = total > max_entries;

    let shown = if truncated { &paths[..max_entries] } else { &paths };

    let header = if truncated {
        format!("[find] {} results (showing first {}):\n", total, max_entries)
    } else {
        format!("[find] {} results:\n", total)
    };

    let body = shown.join("\n");

    (format!("{}{}", header, body), truncated)
}
