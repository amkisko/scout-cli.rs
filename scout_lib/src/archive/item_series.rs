//! Insights lists and per-endpoint / per-job metric series.

use crate::archive::fetch::{count_store, pull_endpoints, pull_jobs};
use crate::archive::resource::{PullOptions, PullReport, DEFAULT_METRICS};
use crate::archive::store::ArchiveStore;
use crate::archive::traces::{endpoint_id_from_record, job_id_from_record, scan_limit};
use crate::client::Client;
use serde_json::{json, Value};

const DEFAULT_JOB_METRICS: [&str; 5] = [
    "throughput",
    "execution_time",
    "latency",
    "errors",
    "allocations",
];
const INSIGHTS_HISTORY_PAGE: u32 = 20;

pub async fn pull_insights(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    report: &mut PullReport,
) -> Result<(), String> {
    if store.range_snapshot_exists(app_id, "insights", from, to) {
        store.index_existing_range_snapshot(app_id, "insights", from, to)?;
        report.skipped += 1;
        return Ok(());
    }
    let current = client
        .get_all_insights(app_id, None)
        .await
        .map_err(|error| error.to_string())?;
    let history = fetch_insights_history(client, app_id, from, to).await?;
    count_store(
        report,
        store.store_range_snapshot(
            app_id,
            "insights",
            from,
            to,
            json!({ "current": current, "history": history }),
            false,
        )?,
    );
    Ok(())
}

async fn fetch_insights_history(
    client: &Client,
    app_id: u64,
    from: &str,
    to: &str,
) -> Result<Value, String> {
    let mut insights = Vec::new();
    let mut page = 1u32;
    let mut cursor = None;
    let mut total_count = 0u64;
    loop {
        let data = match client
            .get_insights_history(
                app_id,
                Some(from),
                Some(to),
                Some(INSIGHTS_HISTORY_PAGE),
                cursor,
                Some("forward"),
                Some(page),
            )
            .await
        {
            Ok(data) => data,
            Err(error) if page == 1 => return Err(error.to_string()),
            Err(_) => break,
        };
        if let Some(count) = data.get("total_count").and_then(Value::as_u64) {
            total_count = count;
        }
        if let Some(rows) = data.get("insights").and_then(Value::as_array) {
            insights.extend(rows.iter().cloned());
        }
        let pagination = data.get("pagination").cloned().unwrap_or(Value::Null);
        let has_more = pagination
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !has_more || page >= 50 {
            break;
        }
        cursor = pagination.get("pagination_cursor").and_then(Value::as_u64);
        page = pagination
            .get("pagination_page")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .unwrap_or(page + 1);
    }
    Ok(json!({
        "total_count": total_count,
        "insights": insights,
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn pull_endpoint_metrics(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    options: &PullOptions,
    report: &mut PullReport,
    progress: impl Fn(&str),
) -> Result<(), String> {
    if !store.range_snapshot_exists(app_id, "endpoints", from, to) {
        pull_endpoints(client, store, app_id, from, to, report).await?;
    }
    let records = store
        .load_range_snapshot(app_id, "endpoints", from, to)
        .ok()
        .map(|file| crate::archive::diff::extract_endpoint_array_for_export(&file.data))
        .unwrap_or_default();
    let ids: Vec<String> = records
        .iter()
        .filter_map(endpoint_id_from_record)
        .take(scan_limit(options.trace_endpoint_limit))
        .collect();
    for (index, endpoint_id) in ids.iter().enumerate() {
        progress(&format!("  endpoint metrics {}/{}", index + 1, ids.len()));
        for metric_type in DEFAULT_METRICS {
            let series = client
                .get_endpoint_metrics(app_id, endpoint_id, metric_type, Some(from), Some(to), None)
                .await
                .map_err(|error| error.to_string())?;
            let metric_key = format!("endpoint__{}__{metric_type}", sanitize_id(endpoint_id));
            let metric_report =
                store.merge_metric_series(app_id, &metric_key, &series, options.force)?;
            report.metric_points_added += metric_report.added_points;
            report.metric_points_skipped += metric_report.skipped_points;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn pull_job_metrics(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    options: &PullOptions,
    report: &mut PullReport,
    progress: impl Fn(&str),
) -> Result<(), String> {
    if !store.range_snapshot_exists(app_id, "jobs", from, to) {
        pull_jobs(client, store, app_id, from, to, report).await?;
    }
    let records = store
        .load_range_snapshot(app_id, "jobs", from, to)
        .ok()
        .map(|file| crate::archive::diff::extract_jobs_array_for_export(&file.data))
        .unwrap_or_default();
    let ids: Vec<String> = records
        .iter()
        .filter_map(job_id_from_record)
        .take(scan_limit(options.trace_endpoint_limit))
        .collect();
    for (index, job_id) in ids.iter().enumerate() {
        progress(&format!("  job metrics {}/{}", index + 1, ids.len()));
        for metric_type in DEFAULT_JOB_METRICS {
            let series = client
                .get_job_metrics(app_id, job_id, metric_type, Some(from), Some(to), None)
                .await
                .map_err(|error| error.to_string())?;
            let metric_key = format!("job__{}__{metric_type}", sanitize_id(job_id));
            let metric_report =
                store.merge_metric_series(app_id, &metric_key, &series, options.force)?;
            report.metric_points_added += metric_report.added_points;
            report.metric_points_skipped += metric_report.skipped_points;
        }
    }
    Ok(())
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}
