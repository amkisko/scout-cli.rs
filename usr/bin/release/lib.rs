//! Shared release helpers: workspace version, packaging sync, and loc limits.

mod loc_limits;
mod packaging;

pub use loc_limits::{
    check_loc_limits, write_baseline as write_loc_baseline, LocFinding, LocFindingKind, LocReport,
    HARD_LIMIT as HARD_LOC_LIMIT, SOFT_LIMIT as SOFT_LOC_LIMIT,
};
pub use packaging::{check_packaging, sync_packaging};

use std::fs;
use std::path::Path;

pub fn workspace_version(root: &Path) -> String {
    let content = fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml");
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version = ") {
            return line
                .trim_start_matches("version = ")
                .trim_matches('"')
                .trim()
                .to_string();
        }
    }
    panic!("version not found in workspace Cargo.toml");
}

/// Fail unless CHANGELOG.md has a release heading for this version.
/// Accepts `## 1.2.3`, `## 1.2.3 (date)`, `## [1.2.3]`, `## [1.2.3] - date`.
/// Rejects Unreleased headings.
pub fn require_changelog_version(root: &Path, version: &str) {
    let path = root.join("CHANGELOG.md");
    let Ok(content) = fs::read_to_string(&path) else {
        eprintln!(
            "\x1b[1;31mrelease requires CHANGELOG.md with a ## {version} heading\x1b[0m"
        );
        std::process::exit(1);
    };
    if !changelog_includes_version(&content, version) {
        eprintln!(
            "\x1b[1;31mrelease requires CHANGELOG.md to include a version heading for {version} \
             (e.g. ## {version} or ## [{version}])\x1b[0m"
        );
        std::process::exit(1);
    }
}

pub fn changelog_includes_version(content: &str, version: &str) -> bool {
    content.lines().any(|line| changelog_line_has_version(line, version))
}

fn changelog_line_has_version(line: &str, version: &str) -> bool {
    let trimmed = line.trim_end();
    // Reject Unreleased headings, including "## 1.2.3 (Unreleased)".
    if !trimmed.starts_with("##") || trimmed.to_ascii_lowercase().contains("unreleased") {
        return false;
    }
    let rest = trimmed[2..].trim_start();
    if let Some(inner) = rest.strip_prefix('[') {
        return inner
            .split_once(']')
            .is_some_and(|(candidate, _)| candidate == version);
    }
    if rest == version {
        return true;
    }
    rest.strip_prefix(version)
        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(|c: char| c.is_whitespace() || c == '('))
}
