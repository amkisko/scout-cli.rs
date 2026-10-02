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
