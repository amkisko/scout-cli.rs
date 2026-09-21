use scout_lib::{ArchiveLayout, ArchiveStore, StoreAction};
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_archive() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("scout-archive-itest-{nanos}"))
}

#[test]
fn range_snapshot_is_idempotent_without_force() {
    let root = temp_archive();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    let data = json!([{"name": "HomeController#index"}]);
    let action = store
        .store_range_snapshot(
            1,
            "endpoints",
            "2025-01-01T00:00:00Z",
            "2025-01-02T00:00:00Z",
            data.clone(),
            false,
        )
        .unwrap();
    assert_eq!(action, StoreAction::Created);
    store.save_manifest().unwrap();

    let skipped = store
        .store_range_snapshot(
            1,
            "endpoints",
            "2025-01-01T00:00:00Z",
            "2025-01-02T00:00:00Z",
            data,
            false,
        )
        .unwrap();
    assert_eq!(skipped, StoreAction::Skipped);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn metric_merge_writes_buckets_once() {
    let root = temp_archive();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    let series = json!({
        "avg": [{"timestamp": "2025-01-01T10:00:00Z", "value": 42.0}]
    });
    let first = store
        .merge_metric_series(7, "response_time", &series, false)
        .unwrap();
    assert_eq!(first.added_points, 1);
    assert_eq!(first.buckets_written, 1);
    store.save_manifest().unwrap();

    let second = store
        .merge_metric_series(7, "response_time", &series, false)
        .unwrap();
    assert_eq!(second.added_points, 0);
    assert_eq!(second.skipped_points, 1);
    assert_eq!(second.buckets_written, 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn metric_merge_stores_live_tuple_points() {
    let root = temp_archive();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    let series = json!({
        "throughput": [
            ["2026-09-20T12:10:00Z", 3.3],
            ["2026-09-21T09:00:00Z", 7.0]
        ]
    });
    let report = store
        .merge_metric_series(11, "throughput", &series, false)
        .unwrap();
    assert_eq!(report.added_points, 2);
    assert_eq!(report.buckets_written, 2);
    let bucket = store
        .load_metric_bucket(11, "throughput", "2026-09-20")
        .unwrap();
    assert_eq!(bucket.series["throughput"][0]["value"], 3.3);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn skip_records_existing_snapshot_into_manifest() {
    let root = temp_archive();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    let data = json!([{"name": "Home#index"}]);
    store
        .store_range_snapshot(
            9,
            "endpoints",
            "2025-01-01T00:00:00Z",
            "2025-01-02T00:00:00Z",
            data.clone(),
            false,
        )
        .unwrap();
    // Simulate abort before save_manifest by opening a fresh store.
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    assert!(store.app_manifest(9).is_none());
    let skipped = store
        .store_range_snapshot(
            9,
            "endpoints",
            "2025-01-01T00:00:00Z",
            "2025-01-02T00:00:00Z",
            data,
            false,
        )
        .unwrap();
    assert_eq!(skipped, StoreAction::Skipped);
    store.save_manifest().unwrap();
    let manifest = store.app_manifest(9).unwrap();
    assert_eq!(manifest.range_snapshots.len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reindex_counts_orphan_trace_files() {
    let root = temp_archive();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    store
        .store_entity(3, "traces", "99", json!({"id": 99}), false)
        .unwrap();
    let layout = ArchiveLayout::new(&root);
    let mut store = ArchiveStore::open(layout).unwrap();
    assert!(store.app_manifest(3).is_none());
    store.reindex_app(3).unwrap();
    store.save_manifest().unwrap();
    assert_eq!(store.app_manifest(3).unwrap().entities.traces, 1);
    let _ = std::fs::remove_dir_all(root);
}
