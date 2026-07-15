//! Archive directory layout and manifest schema.

use crate::config::scout_home;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const MANIFEST_VERSION: u32 = 1;
const MANIFEST_FILE: &str = "manifest.json";

/// Resolve archive root: `SCOUT_ARCHIVE_HOME` or `{scout_home}/archive`.
pub fn archive_home() -> PathBuf {
    if let Ok(path) = std::env::var("SCOUT_ARCHIVE_HOME") {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    scout_home()
        .map(|home| home.join("archive"))
        .unwrap_or_else(|| PathBuf::from(".scout/archive"))
}

/// Filesystem-safe key for a time range.
pub fn range_key(from: &str, to: &str) -> String {
    let compact = |value: &str| {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect::<String>()
    };
    format!("{}__{}", compact(from), compact(to))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RangeSnapshotMeta {
    pub resource: String,
    pub from: String,
    pub to: String,
    pub stored_at: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EntityCounts {
    pub traces: u64,
    pub error_groups: u64,
    pub anomalies: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AppManifest {
    pub last_pull_at: Option<String>,
    pub last_pull_from: Option<String>,
    pub last_pull_to: Option<String>,
    pub range_snapshots: Vec<RangeSnapshotMeta>,
    #[serde(default)]
    pub metric_buckets: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub entities: EntityCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub apps: HashMap<String, AppManifest>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            version: MANIFEST_VERSION,
            apps: HashMap::new(),
        }
    }
}

pub struct ArchiveLayout {
    root: PathBuf,
}

impl ArchiveLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_env() -> Self {
        Self::new(archive_home())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_FILE)
    }

    pub fn app_dir(&self, app_id: u64) -> PathBuf {
        self.root.join("apps").join(app_id.to_string())
    }

    pub fn app_metadata_path(&self, app_id: u64) -> PathBuf {
        self.app_dir(app_id).join("app.json")
    }

    pub fn range_snapshot_path(
        &self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
    ) -> PathBuf {
        self.app_dir(app_id)
            .join(resource)
            .join("ranges")
            .join(format!("{}.json", range_key(from, to)))
    }

    pub fn metric_bucket_path(&self, app_id: u64, metric_type: &str, date: &str) -> PathBuf {
        self.app_dir(app_id)
            .join("metrics")
            .join(metric_type)
            .join("buckets")
            .join(format!("{date}.json"))
    }

    pub fn entity_path(&self, app_id: u64, resource: &str, entity_id: &str) -> PathBuf {
        self.app_dir(app_id)
            .join(resource)
            .join("by_id")
            .join(format!("{entity_id}.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_key_is_stable_and_filename_safe() {
        let key = range_key("2025-01-01T00:00:00Z", "2025-01-02T00:00:00Z");
        assert_eq!(key, "20250101T000000Z__20250102T000000Z");
        assert!(!key.contains(':'));
        assert!(!key.contains('/'));
    }

    #[test]
    fn layout_paths_follow_convention() {
        let layout = ArchiveLayout::new("/tmp/scout-archive");
        assert_eq!(
            layout.manifest_path(),
            PathBuf::from("/tmp/scout-archive/manifest.json")
        );
        assert_eq!(
            layout.metric_bucket_path(42, "response_time", "2025-01-01"),
            PathBuf::from(
                "/tmp/scout-archive/apps/42/metrics/response_time/buckets/2025-01-01.json"
            )
        );
        assert_eq!(
            layout.range_snapshot_path(42, "endpoints", "2025-01-01T00:00:00Z", "2025-01-02T00:00:00Z"),
            PathBuf::from(
                "/tmp/scout-archive/apps/42/endpoints/ranges/20250101T000000Z__20250102T000000Z.json"
            )
        );
    }
}
