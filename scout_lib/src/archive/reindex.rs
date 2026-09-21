//! Recover archive manifest records from files already on disk.

use crate::archive::store::{read_json, ArchiveStore, RangeSnapshotFile};
use std::fs;
use std::path::PathBuf;

impl ArchiveStore {
    pub fn reindex_app(&mut self, app_id: u64) -> Result<(), String> {
        self.reindex_range_dir(app_id, "endpoints")?;
        self.reindex_range_dir(app_id, "jobs")?;
        self.reindex_range_dir(app_id, "errors")?;
        self.reindex_range_dir(app_id, "anomalies")?;
        self.reindex_range_dir(app_id, "insights")?;
        self.reindex_metric_dirs(app_id)?;
        self.sync_entity_count(app_id, "traces");
        Ok(())
    }

    fn reindex_range_dir(&mut self, app_id: u64, resource: &str) -> Result<(), String> {
        let dir = self
            .layout()
            .range_snapshot_path(app_id, resource, "a", "b")
            .parent()
            .map(PathBuf::from);
        let Some(dir) = dir else {
            return Ok(());
        };
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(&dir).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let snapshot: RangeSnapshotFile = match read_json(&path) {
                Ok(snapshot) => snapshot,
                Err(_) => continue,
            };
            self.record_range_snapshot(app_id, resource, &snapshot.from, &snapshot.to, &path);
        }
        Ok(())
    }

    fn reindex_metric_dirs(&mut self, app_id: u64) -> Result<(), String> {
        let metrics_root = self.layout().app_dir(app_id).join("metrics");
        if !metrics_root.is_dir() {
            return Ok(());
        }
        for metric_entry in fs::read_dir(&metrics_root).map_err(|error| error.to_string())? {
            let metric_path = metric_entry.map_err(|error| error.to_string())?.path();
            if !metric_path.is_dir() {
                continue;
            }
            let metric_type = metric_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let buckets = metric_path.join("buckets");
            if !buckets.is_dir() {
                continue;
            }
            for bucket_entry in fs::read_dir(&buckets).map_err(|error| error.to_string())? {
                let bucket_path = bucket_entry.map_err(|error| error.to_string())?.path();
                let Some(date) = bucket_path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if bucket_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    self.record_metric_bucket(app_id, &metric_type, date);
                }
            }
        }
        Ok(())
    }
}
