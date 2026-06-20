//! Filter module: classify commands and apply the right output filter.
//!
//! Filters take raw command output and return a shorter, LLM-optimized summary.
//! Each filter strips noise (timestamps, progress bars, ANSI codes) and keeps
//! only the signal: file names, error messages, diffs, test failures.

use crate::config::Config;

mod ls;
mod cat;
mod grep;
mod find;
mod git;
mod test;
mod generic;

/// Classify a command and its args to select the appropriate filter.
pub fn classify(program: &str, args: &[String]) -> String {
    let prog = program.to_lowercase();

    match prog.as_str() {
        "ls" | "ll" | "dir" | "vdir" => "ls".into(),

        "cat" | "head" | "tail" | "less" => "cat".into(),

        "grep" | "egrep" | "fgrep" | "rg" | "ag" | "ack" => "grep".into(),

        "find" | "fd" | "locate" => "find".into(),

        "git" => {
            // Subcommand-aware classification.
            if args.is_empty() {
                return "git".into();
            }
            match args[0].as_str() {
                "status" | "diff" | "log" | "show" | "branch" | "tag" => "git".into(),
                "add" | "commit" | "push" | "pull" | "fetch" | "merge" | "rebase"
                | "checkout" | "switch" | "restore" | "reset" | "stash" => "git".into(),
                _ => "generic".into(),
            }
        }

        "cargo" => {
            if !args.is_empty() && args[0] == "test" {
                return "test".into();
            }
            "generic".into()
        }

        "pytest" | "tox" | "nose" | "unittest" => "test".into(),

        "npm" | "npx" | "pnpm" | "yarn" => {
            if !args.is_empty() && (args[0] == "test" || args[0] == "run") {
                return "test".into();
            }
            "generic".into()
        }

        "go" => {
            if !args.is_empty() && args[0] == "test" {
                return "test".into();
            }
            "generic".into()
        }

        "make" => {
            if !args.is_empty() && args[0] == "test" {
                return "test".into();
            }
            "generic".into()
        }

        _ => "generic".into(),
    }
}

/// Apply the named filter to raw output. Returns (filtered_output, was_truncated).
pub fn apply(filter: &str, raw: &str, config: &Config) -> (String, bool) {
    match filter {
        "ls" => ls::filter(raw, config),
        "cat" => cat::filter(raw, config),
        "grep" => grep::filter(raw, config),
        "find" => find::filter(raw, config),
        "git" => git::filter(raw, config),
        "test" => test::filter(raw, config),
        _ => generic::filter(raw, config),
    }
}
