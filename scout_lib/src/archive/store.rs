//! Read and write archive files with idempotent semantics.

use crate::archive::layout::{
    AppManifest, ArchiveLayout, Manifest, RangeSnapshotMeta, MANIFEST_VERSION,
};
use crate::archive::metrics::MetricBucket;
use crate::helpers::format_time;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreAction {
    Created,
    Skipped,
    Merged,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricMergeReport {
    pub added_points: u64,
    pub skipped_points: u64,
    pub buckets_written: u64,
    pub buckets_skipped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeSnapshotFile {
    pub resource: String,
    pub from: String,
    pub to: String,
    pub fetched_at: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshotFile {
    pub resource: String,
    pub entity_id: String,
    pub fetched_at: String,
    pub data: Value,
}

pub struct ArchiveStore {
    layout: ArchiveLayout,
    manifest: Manifest,
}

impl ArchiveStore {
    pub fn open(layout: ArchiveLayout) -> Result<Self, String> {
        fs::create_dir_all(layout.root()).map_err(|error| error.to_string())?;
        let manifest = load_manifest(&layout)?;
        Ok(Self { layout, manifest })
    }

    pub fn from_env() -> Result<Self, String> {
        Self::open(ArchiveLayout::from_env())
    }

    pub fn layout(&self) -> &ArchiveLayout {
        &self.layout
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn app_manifest(&self, app_id: u64) -> Option<&AppManifest> {
        self.manifest.apps.get(&app_id.to_string())
    }

    pub fn save_manifest(&self) -> Result<(), String> {
        write_json_atomic(&self.layout.manifest_path(), &self.manifest)
    }

    pub fn range_snapshot_exists(&self, app_id: u64, resource: &str, from: &str, to: &str) -> bool {
        self.layout
            .range_snapshot_path(app_id, resource, from, to)
            .is_file()
    }

    pub fn index_existing_range_snapshot(
        &mut self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
    ) -> Result<(), String> {
        let path = self.layout.range_snapshot_path(app_id, resource, from, to);
        if path.is_file() {
            self.record_range_snapshot(app_id, resource, from, to, &path);
        }
        Ok(())
    }

    pub fn record_existing_entity(&mut self, app_id: u64, resource: &str) {
        self.sync_entity_count(app_id, resource);
    }

    pub fn store_range_snapshot(
        &mut self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
        data: Value,
        force: bool,
    ) -> Result<StoreAction, String> {
        let path = self.layout.range_snapshot_path(app_id, resource, from, to);
        let existed = path.is_file();
        if existed && !force {
            self.record_range_snapshot(app_id, resource, from, to, &path);
            return Ok(StoreAction::Skipped);
        }
        let snapshot = RangeSnapshotFile {
            resource: resource.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            fetched_at: format_time(Utc::now()),
            data,
        };
        write_json_atomic(&path, &snapshot)?;
        self.record_range_snapshot(app_id, resource, from, to, &path);
        Ok(if existed {
            StoreAction::Merged
        } else {
            StoreAction::Created
        })
    }

    pub fn load_range_snapshot(
        &self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
    ) -> Result<RangeSnapshotFile, String> {
        let path = self.layout.range_snapshot_path(app_id, resource, from, to);
        read_json(&path)
    }

    pub fn find_range_snapshot(
        &self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
    ) -> Option<RangeSnapshotMeta> {
        self.manifest
            .apps
            .get(&app_id.to_string())
            .and_then(|app| {
                app.range_snapshots.iter().find(|record| {
                    record.resource == resource && record.from == from && record.to == to
                })
            })
            .cloned()
    }

    pub fn merge_metric_series(
        &mut self,
        app_id: u64,
        metric_type: &str,
        series: &Value,
        force: bool,
    ) -> Result<MetricMergeReport, String> {
        let existing_buckets = self.load_metric_buckets(app_id, metric_type)?;
        let (merged_buckets, merge_stats) = crate::archive::metrics::merge_series_into_buckets(
            metric_type,
            series,
            &existing_buckets,
        );

        let mut report = MetricMergeReport {
            added_points: merge_stats.added_points,
            skipped_points: merge_stats.skipped_points,
            ..MetricMergeReport::default()
        };

        if merge_stats.added_points == 0 && !force {
            report.buckets_skipped = merged_buckets.len() as u64;
            return Ok(report);
        }

        for (date, bucket) in &merged_buckets {
            let path = self.layout.metric_bucket_path(app_id, metric_type, date);
            if path.is_file() && merge_stats.added_points == 0 && !force {
                report.buckets_skipped += 1;
                continue;
            }
            write_json_atomic(&path, bucket)?;
            report.buckets_written += 1;
            self.record_metric_bucket(app_id, metric_type, date);
        }
        Ok(report)
    }

    pub fn load_metric_bucket(
        &self,
        app_id: u64,
        metric_type: &str,
        date: &str,
    ) -> Result<MetricBucket, String> {
        let path = self.layout.metric_bucket_path(app_id, metric_type, date);
        read_json(&path)
    }

    pub fn list_metric_bucket_dates(&self, app_id: u64, metric_type: &str) -> Vec<String> {
        self.manifest
            .apps
            .get(&app_id.to_string())
            .and_then(|app| app.metric_buckets.get(metric_type))
            .cloned()
            .unwrap_or_default()
    }

    pub fn store_app_metadata(
        &mut self,
        app_id: u64,
        data: Value,
        force: bool,
    ) -> Result<StoreAction, String> {
        let path = self.layout.app_metadata_path(app_id);
        if path.is_file() && !force {
            return Ok(StoreAction::Skipped);
        }
        let payload = serde_json::json!({
            "fetched_at": format_time(Utc::now()),
            "data": data,
        });
        write_json_atomic(&path, &payload)?;
        Ok(StoreAction::Created)
    }

    pub fn entity_exists(&self, app_id: u64, resource: &str, entity_id: &str) -> bool {
        self.layout
            .entity_path(app_id, resource, entity_id)
            .is_file()
    }

    pub fn store_entity(
        &mut self,
        app_id: u64,
        resource: &str,
        entity_id: &str,
        data: Value,
        force: bool,
    ) -> Result<StoreAction, String> {
        let path = self.layout.entity_path(app_id, resource, entity_id);
        if path.is_file() && !force {
            self.sync_entity_count(app_id, resource);
            return Ok(StoreAction::Skipped);
        }
        let snapshot = EntitySnapshotFile {
            resource: resource.to_string(),
            entity_id: entity_id.to_string(),
            fetched_at: format_time(Utc::now()),
            data,
        };
        write_json_atomic(&path, &snapshot)?;
        self.sync_entity_count(app_id, resource);
        Ok(StoreAction::Created)
    }

    pub fn record_pull_window(&mut self, app_id: u64, from: &str, to: &str) {
        let entry = self.manifest.apps.entry(app_id.to_string()).or_default();
        entry.last_pull_at = Some(format_time(Utc::now()));
        entry.last_pull_from = Some(from.to_string());
        entry.last_pull_to = Some(to.to_string());
    }

    fn load_metric_buckets(
        &self,
        app_id: u64,
        metric_type: &str,
    ) -> Result<HashMap<String, MetricBucket>, String> {
        let mut buckets = HashMap::new();
        for date in self.list_metric_bucket_dates(app_id, metric_type) {
            let bucket = self.load_metric_bucket(app_id, metric_type, &date)?;
            buckets.insert(date, bucket);
        }
        Ok(buckets)
    }

    pub(crate) fn record_range_snapshot(
        &mut self,
        app_id: u64,
        resource: &str,
        from: &str,
        to: &str,
        path: &Path,
    ) {
        let entry = self.manifest.apps.entry(app_id.to_string()).or_default();
        let relative_path = path
            .strip_prefix(self.layout.root())
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        if let Some(existing) = entry
            .range_snapshots
            .iter_mut()
            .find(|record| record.resource == resource && record.from == from && record.to == to)
        {
            existing.stored_at = format_time(Utc::now());
            existing.path = relative_path;
            return;
        }
        entry.range_snapshots.push(RangeSnapshotMeta {
            resource: resource.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            stored_at: format_time(Utc::now()),
            path: relative_path,
        });
    }

    pub(crate) fn record_metric_bucket(&mut self, app_id: u64, metric_type: &str, date: &str) {
        let entry = self.manifest.apps.entry(app_id.to_string()).or_default();
        let buckets = entry
            .metric_buckets
            .entry(metric_type.to_string())
            .or_default();
        if !buckets.iter().any(|existing| existing == date) {
            buckets.push(date.to_string());
            buckets.sort();
        }
    }

    pub(crate) fn sync_entity_count(&mut self, app_id: u64, resource: &str) {
        let dir = self
            .layout
            .entity_path(app_id, resource, "_")
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.layout.app_dir(app_id).join(resource).join("by_id"));
        let count = count_json_files(&dir);
        let entry = self.manifest.apps.entry(app_id.to_string()).or_default();
        match resource {
            "traces" => entry.entities.traces = count,
            "errors" => entry.entities.error_groups = count,
            "anomalies" => entry.entities.anomalies = count,
            _ => {}
        }
    }
}

fn count_json_files(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count() as u64
}

fn load_manifest(layout: &ArchiveLayout) -> Result<Manifest, String> {
    let path = layout.manifest_path();
    if !path.is_file() {
        return Ok(Manifest::new());
    }
    let mut manifest: Manifest = read_json(&path)?;
    if manifest.version == 0 {
        manifest.version = MANIFEST_VERSION;
    }
    Ok(manifest)
}

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&contents).map_err(|error| error.to_string())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let serialized = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temp_path = temp_path_for(path);
    fs::write(&temp_path, serialized).map_err(|error| error.to_string())?;
    fs::rename(&temp_path, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive.tmp".to_string());
    path.with_file_name(format!("{file_name}.tmp"))
}
