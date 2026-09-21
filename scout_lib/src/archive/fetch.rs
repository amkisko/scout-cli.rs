//! Per-resource archive fetchers.

use crate::archive::resource::{PullOptions, PullReport, DEFAULT_METRICS};
use crate::archive::store::{ArchiveStore, MetricMergeReport, StoreAction};
use crate::client::Client;
use serde_json::json;

pub async fn pull_metrics(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    options: &PullOptions,
    report: &mut PullReport,
) -> Result<(), String> {
    let metric_types: Vec<&str> = if options.metrics.is_empty() {
        DEFAULT_METRICS.to_vec()
    } else {
        options.metrics.iter().map(String::as_str).collect()
    };
    let mut combined = MetricMergeReport::default();
    for metric_type in metric_types {
        let series = client
            .get_metric(app_id, metric_type, Some(from), Some(to), None)
            .await
            .map_err(|error| error.to_string())?;
        let metric_report =
            store.merge_metric_series(app_id, metric_type, &series, options.force)?;
        combined.added_points += metric_report.added_points;
        combined.skipped_points += metric_report.skipped_points;
        combined.buckets_written += metric_report.buckets_written;
        combined.buckets_skipped += metric_report.buckets_skipped;
    }
    report.metric_points_added += combined.added_points;
    report.metric_points_skipped += combined.skipped_points;
    if combined.buckets_written > 0 {
        report.created += combined.buckets_written;
    }
    if combined.buckets_skipped > 0 {
        report.skipped += combined.buckets_skipped;
    }
    Ok(())
}

pub async fn pull_endpoints(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    report: &mut PullReport,
) -> Result<(), String> {
    if store.range_snapshot_exists(app_id, "endpoints", from, to) {
        store.index_existing_range_snapshot(app_id, "endpoints", from, to)?;
        report.skipped += 1;
        return Ok(());
    }
    let data = client
        .list_endpoints(
            app_id,
            Some(from),
            Some(to),
            None,
            Some("response_time"),
            Some(500),
            Some(0),
        )
        .await
        .map_err(|error| error.to_string())?;
    count_store(
        report,
        store.store_range_snapshot(app_id, "endpoints", from, to, data, false)?,
    );
    Ok(())
}

pub async fn pull_jobs(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    report: &mut PullReport,
) -> Result<(), String> {
    if store.range_snapshot_exists(app_id, "jobs", from, to) {
        store.index_existing_range_snapshot(app_id, "jobs", from, to)?;
        report.skipped += 1;
        return Ok(());
    }
    let data = client
        .list_jobs(app_id, Some(from), Some(to), None)
        .await
        .map_err(|error| error.to_string())?;
    count_store(
        report,
        store.store_range_snapshot(app_id, "jobs", from, to, data, false)?,
    );
    Ok(())
}

pub async fn pull_errors(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    report: &mut PullReport,
) -> Result<(), String> {
    if store.range_snapshot_exists(app_id, "errors", from, to) {
        store.index_existing_range_snapshot(app_id, "errors", from, to)?;
        report.skipped += 1;
        return Ok(());
    }
    let groups = client
        .list_error_groups(app_id, Some(from), Some(to), None)
        .await
        .map_err(|error| error.to_string())?;
    count_store(
        report,
        store.store_range_snapshot(
            app_id,
            "errors",
            from,
            to,
            json!({ "error_groups": groups }),
            false,
        )?,
    );
    Ok(())
}

pub async fn pull_anomalies(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    report: &mut PullReport,
) -> Result<(), String> {
    if store.range_snapshot_exists(app_id, "anomalies", from, to) {
        store.index_existing_range_snapshot(app_id, "anomalies", from, to)?;
        report.skipped += 1;
        return Ok(());
    }
    let events = client
        .list_anomaly_events(app_id, Some(from), Some(to), None, Some("all"), None, None)
        .await
        .map_err(|error| error.to_string())?;
    count_store(
        report,
        store.store_range_snapshot(
            app_id,
            "anomalies",
            from,
            to,
            json!({ "anomaly_events": events }),
            false,
        )?,
    );
    Ok(())
}

pub(crate) fn count_store(report: &mut PullReport, action: StoreAction) {
    match action {
        StoreAction::Created | StoreAction::Merged => report.created += 1,
        StoreAction::Skipped => report.skipped += 1,
    }
}
