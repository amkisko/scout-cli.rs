//! Pull ScoutAPM data into the local archive.

use crate::archive::store::{ArchiveStore, MetricMergeReport, StoreAction};
use crate::client::Client;
use crate::helpers::{calculate_range, format_time, parse_time};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_METRICS: [&str; 6] = [
    "apdex",
    "response_time",
    "response_time_95th",
    "errors",
    "throughput",
    "queue_time",
];

const MAX_RANGE_SECS: i64 = 14 * 24 * 3600;
const INCREMENTAL_OVERLAP_SECS: i64 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullResource {
    App,
    Metrics,
    Endpoints,
    Jobs,
    Errors,
    Anomalies,
    Traces,
}

impl PullResource {
    pub fn as_str(self) -> &'static str {
        match self {
            PullResource::App => "app",
            PullResource::Metrics => "metrics",
            PullResource::Endpoints => "endpoints",
            PullResource::Jobs => "jobs",
            PullResource::Errors => "errors",
            PullResource::Anomalies => "anomalies",
            PullResource::Traces => "traces",
        }
    }

    pub fn parse_list(values: &[String]) -> Result<Vec<PullResource>, String> {
        if values.is_empty() {
            return Ok(vec![
                PullResource::App,
                PullResource::Metrics,
                PullResource::Endpoints,
                PullResource::Jobs,
                PullResource::Errors,
                PullResource::Anomalies,
            ]);
        }
        let mut resources = Vec::new();
        for value in values {
            let resource = match value.as_str() {
                "app" => PullResource::App,
                "metrics" => PullResource::Metrics,
                "endpoints" => PullResource::Endpoints,
                "jobs" => PullResource::Jobs,
                "errors" => PullResource::Errors,
                "anomalies" => PullResource::Anomalies,
                "traces" => PullResource::Traces,
                other => {
                    return Err(format!(
                        "unknown archive resource '{other}'. Expected: app, metrics, endpoints, jobs, errors, anomalies, traces"
                    ));
                }
            };
            if !resources.contains(&resource) {
                resources.push(resource);
            }
        }
        Ok(resources)
    }
}

#[derive(Debug, Clone)]
pub struct PullOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub range: Option<String>,
    pub resources: Vec<PullResource>,
    pub metrics: Vec<String>,
    pub trace_ids: Vec<u64>,
    pub trace_endpoint_limit: u32,
    pub force: bool,
    pub incremental: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullReport {
    pub app_id: u64,
    pub from: String,
    pub to: String,
    pub chunks: u64,
    pub created: u64,
    pub skipped: u64,
    pub metric_points_added: u64,
    pub metric_points_skipped: u64,
    pub traces_created: u64,
    pub traces_skipped: u64,
    pub resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullPlan {
    pub app_id: u64,
    pub archive_home: String,
    pub from: String,
    pub to: String,
    pub chunk_count: u64,
    pub resources: Vec<String>,
    pub metrics: Vec<String>,
    pub trace_ids: Vec<u64>,
    pub incremental: bool,
    pub force: bool,
    pub dry_run: bool,
}

pub fn plan_pull(
    store: &ArchiveStore,
    app_id: u64,
    options: &PullOptions,
) -> Result<PullPlan, String> {
    let (from, to) = resolve_pull_window(store, app_id, options)?;
    let chunks = split_range(&from, &to)?;
    let metrics = if options.metrics.is_empty() {
        DEFAULT_METRICS
            .iter()
            .map(|metric| (*metric).to_string())
            .collect()
    } else {
        options.metrics.clone()
    };
    Ok(PullPlan {
        app_id,
        archive_home: store.layout().root().display().to_string(),
        from,
        to,
        chunk_count: chunks.len() as u64,
        resources: options
            .resources
            .iter()
            .map(|resource| resource.as_str().to_string())
            .collect(),
        metrics,
        trace_ids: options.trace_ids.clone(),
        incremental: options.incremental,
        force: options.force,
        dry_run: true,
    })
}

pub fn format_pull_summary(report: &PullReport) -> String {
    let mut parts = vec![
        format!("{} snapshot(s) created", report.created),
        format!("{} skipped", report.skipped),
    ];
    if report.metric_points_added > 0 || report.metric_points_skipped > 0 {
        parts.push(format!(
            "{} metric point(s) added ({} unchanged)",
            report.metric_points_added, report.metric_points_skipped
        ));
    }
    if report.traces_created > 0 || report.traces_skipped > 0 {
        parts.push(format!(
            "{} trace(s) stored ({} already archived)",
            report.traces_created, report.traces_skipped
        ));
    }
    format!("Pull complete: {}", parts.join(", "))
}

pub async fn pull_app(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    options: &PullOptions,
) -> Result<PullReport, String> {
    pull_app_impl(client, store, app_id, options, None).await
}

pub async fn pull_app_with_progress<F>(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    options: &PullOptions,
    on_progress: F,
) -> Result<PullReport, String>
where
    F: Fn(&str),
{
    pull_app_impl(client, store, app_id, options, Some(&on_progress)).await
}

async fn pull_app_impl(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    options: &PullOptions,
    on_progress: Option<&dyn Fn(&str)>,
) -> Result<PullReport, String> {
    let progress = |message: &str| {
        if let Some(callback) = on_progress {
            callback(message);
        }
    };

    let (from, to) = resolve_pull_window(store, app_id, options)?;
    let chunks = split_range(&from, &to)?;
    progress(&format!(
        "Time range {from} .. {to} ({} chunk(s))",
        chunks.len()
    ));
    let mut report = PullReport {
        app_id,
        from: from.clone(),
        to: to.clone(),
        chunks: chunks.len() as u64,
        resources: options
            .resources
            .iter()
            .map(|resource| resource.as_str().to_string())
            .collect(),
        ..PullReport::default()
    };

    if options.resources.contains(&PullResource::App) {
        progress("Fetching app metadata…");
        let action = pull_app_metadata(client, store, app_id, options.force).await?;
        update_action_counts(&mut report, action);
    }

    if !options.trace_ids.is_empty() {
        progress(&format!(
            "Fetching {} trace(s) by ID…",
            options.trace_ids.len()
        ));
        let trace_report =
            pull_trace_ids(client, store, app_id, &options.trace_ids, options.force).await?;
        report.traces_created += trace_report.created;
        report.traces_skipped += trace_report.skipped;
    }

    for (index, (chunk_from, chunk_to)) in chunks.iter().enumerate() {
        progress(&format!(
            "Chunk {}/{}: {chunk_from} .. {chunk_to}",
            index + 1,
            chunks.len()
        ));

        if options.resources.contains(&PullResource::Metrics) {
            progress("  metrics…");
            let metric_report =
                pull_metrics(client, store, app_id, chunk_from, chunk_to, options).await?;
            report.metric_points_added += metric_report.added_points;
            report.metric_points_skipped += metric_report.skipped_points;
            if metric_report.buckets_written > 0 {
                report.created += metric_report.buckets_written;
            }
            if metric_report.buckets_skipped > 0 {
                report.skipped += metric_report.buckets_skipped;
            }
        }

        if options.resources.contains(&PullResource::Endpoints) {
            progress("  endpoints…");
            let action = pull_endpoints(client, store, app_id, chunk_from, chunk_to).await?;
            update_action_counts(&mut report, action);
        }

        if options.resources.contains(&PullResource::Jobs) {
            progress("  jobs…");
            let action = pull_jobs(client, store, app_id, chunk_from, chunk_to).await?;
            update_action_counts(&mut report, action);
        }

        if options.resources.contains(&PullResource::Errors) {
            progress("  errors…");
            let action = pull_errors(client, store, app_id, chunk_from, chunk_to).await?;
            update_action_counts(&mut report, action);
        }

        if options.resources.contains(&PullResource::Anomalies) {
            progress("  anomalies…");
            let action = pull_anomalies(client, store, app_id, chunk_from, chunk_to).await?;
            update_action_counts(&mut report, action);
        }

        if options.resources.contains(&PullResource::Traces) {
            progress("  traces…");
            let trace_report =
                pull_traces_for_range(client, store, app_id, chunk_from, chunk_to, options).await?;
            report.traces_created += trace_report.created;
            report.traces_skipped += trace_report.skipped;
        }
    }

    store.record_pull_window(app_id, &from, &to);
    store.save_manifest()?;
    Ok(report)
}

fn resolve_pull_window(
    store: &ArchiveStore,
    app_id: u64,
    options: &PullOptions,
) -> Result<(String, String), String> {
    if let Some(range) = &options.range {
        let (from, to) = calculate_range(range, options.to.as_deref())?;
        return Ok((from, to));
    }
    if let (Some(from), Some(to)) = (&options.from, &options.to) {
        return Ok((from.clone(), to.clone()));
    }
    if options.incremental {
        if let Some(last_to) = store
            .app_manifest(app_id)
            .and_then(|manifest| manifest.last_pull_to.clone())
        {
            let end = options
                .to
                .clone()
                .unwrap_or_else(|| format_time(Utc::now()));
            let last_to_time = parse_time(&last_to)?;
            let start_time = last_to_time - Duration::seconds(INCREMENTAL_OVERLAP_SECS);
            return Ok((format_time(start_time), end));
        }
    }
    calculate_range("1day", options.to.as_deref())
}

fn split_range(from: &str, to: &str) -> Result<Vec<(String, String)>, String> {
    let from_time = parse_time(from)?;
    let to_time = parse_time(to)?;
    if from_time >= to_time {
        return Err("from_time must be before to_time".to_string());
    }
    let mut chunks = Vec::new();
    let mut chunk_start = from_time;
    while chunk_start < to_time {
        let mut chunk_end = chunk_start + Duration::seconds(MAX_RANGE_SECS);
        if chunk_end > to_time {
            chunk_end = to_time;
        }
        chunks.push((format_time(chunk_start), format_time(chunk_end)));
        if chunk_end >= to_time {
            break;
        }
        chunk_start = chunk_end;
    }
    Ok(chunks)
}

async fn pull_app_metadata(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    force: bool,
) -> Result<StoreAction, String> {
    let app = client
        .get_app(app_id)
        .await
        .map_err(|error| error.to_string())?;
    store.store_app_metadata(app_id, app, force)
}

async fn pull_metrics(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    options: &PullOptions,
) -> Result<MetricMergeReport, String> {
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
        let report = store.merge_metric_series(app_id, metric_type, &series, options.force)?;
        combined.added_points += report.added_points;
        combined.skipped_points += report.skipped_points;
        combined.buckets_written += report.buckets_written;
        combined.buckets_skipped += report.buckets_skipped;
    }
    Ok(combined)
}

async fn pull_endpoints(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
) -> Result<StoreAction, String> {
    if store.range_snapshot_exists(app_id, "endpoints", from, to) {
        return Ok(StoreAction::Skipped);
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
    store.store_range_snapshot(app_id, "endpoints", from, to, data, false)
}

async fn pull_jobs(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
) -> Result<StoreAction, String> {
    if store.range_snapshot_exists(app_id, "jobs", from, to) {
        return Ok(StoreAction::Skipped);
    }
    let data = client
        .list_jobs(app_id, Some(from), Some(to), None)
        .await
        .map_err(|error| error.to_string())?;
    store.store_range_snapshot(app_id, "jobs", from, to, data, false)
}

async fn pull_errors(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
) -> Result<StoreAction, String> {
    if store.range_snapshot_exists(app_id, "errors", from, to) {
        return Ok(StoreAction::Skipped);
    }
    let groups = client
        .list_error_groups(app_id, Some(from), Some(to), None)
        .await
        .map_err(|error| error.to_string())?;
    let data = json!({ "error_groups": groups });
    store.store_range_snapshot(app_id, "errors", from, to, data, false)
}

async fn pull_anomalies(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
) -> Result<StoreAction, String> {
    if store.range_snapshot_exists(app_id, "anomalies", from, to) {
        return Ok(StoreAction::Skipped);
    }
    let events = client
        .list_anomaly_events(app_id, Some(from), Some(to), None, Some("all"), None, None)
        .await
        .map_err(|error| error.to_string())?;
    let data = json!({ "anomaly_events": events });
    store.store_range_snapshot(app_id, "anomalies", from, to, data, false)
}

fn update_action_counts(report: &mut PullReport, action: StoreAction) {
    match action {
        StoreAction::Created | StoreAction::Merged => report.created += 1,
        StoreAction::Skipped => report.skipped += 1,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TracePullReport {
    created: u64,
    skipped: u64,
}

/// Fetch and store one trace by ID (idempotent).
pub async fn pull_trace_by_id(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    trace_id: u64,
    force: bool,
) -> Result<StoreAction, String> {
    if !force && store.entity_exists(app_id, "traces", &trace_id.to_string()) {
        return Ok(StoreAction::Skipped);
    }
    let trace = client
        .fetch_trace(app_id, trace_id)
        .await
        .map_err(|error| error.to_string())?;
    store.store_entity(app_id, "traces", &trace_id.to_string(), trace, force)
}

async fn pull_trace_ids(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    trace_ids: &[u64],
    force: bool,
) -> Result<TracePullReport, String> {
    let mut report = TracePullReport::default();
    for trace_id in trace_ids {
        match pull_trace_by_id(client, store, app_id, *trace_id, force).await? {
            StoreAction::Created | StoreAction::Merged => report.created += 1,
            StoreAction::Skipped => report.skipped += 1,
        }
    }
    Ok(report)
}

async fn pull_traces_for_range(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    options: &PullOptions,
) -> Result<TracePullReport, String> {
    let endpoints_snapshot = if store.range_snapshot_exists(app_id, "endpoints", from, to) {
        store.load_range_snapshot(app_id, "endpoints", from, to)?
    } else {
        pull_endpoints(client, store, app_id, from, to).await?;
        store.load_range_snapshot(app_id, "endpoints", from, to)?
    };
    let endpoint_ids =
        endpoint_ids_from_snapshot(&endpoints_snapshot.data, options.trace_endpoint_limit);
    let mut report = TracePullReport::default();
    for endpoint_id in endpoint_ids {
        let listing = client
            .list_endpoint_traces(app_id, &endpoint_id, Some(from), Some(to), None)
            .await
            .map_err(|error| error.to_string())?;
        for trace_id in trace_ids_from_listing(&listing) {
            match pull_trace_by_id(client, store, app_id, trace_id, options.force).await? {
                StoreAction::Created | StoreAction::Merged => report.created += 1,
                StoreAction::Skipped => report.skipped += 1,
            }
        }
    }
    Ok(report)
}

fn endpoint_ids_from_snapshot(data: &Value, limit: u32) -> Vec<String> {
    let endpoints = crate::archive::diff::extract_endpoint_array_for_export(data);
    let limit = if limit == 0 { 50 } else { limit } as usize;
    endpoints
        .into_iter()
        .filter_map(endpoint_id_from_record)
        .take(limit)
        .collect()
}

fn endpoint_id_from_record(endpoint: Value) -> Option<String> {
    if let Some(link) = endpoint.get("link").and_then(Value::as_str) {
        if let Some((_, id)) = link.split_once("/endpoints/") {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    endpoint
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn trace_ids_from_listing(listing: &Value) -> Vec<u64> {
    let traces = listing
        .get("traces")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| listing.as_array().cloned())
        .unwrap_or_default();
    traces
        .iter()
        .filter_map(|trace| trace.get("id").and_then(Value::as_u64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn split_range_chunks_long_windows() {
        let from = "2025-01-01T00:00:00Z";
        let to = "2025-01-20T00:00:00Z";
        let chunks = split_range(from, to).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, from);
        assert_eq!(chunks[1].1, to);
    }

    #[test]
    fn pull_resources_default_list_is_complete() {
        let resources = PullResource::parse_list(&[]).unwrap();
        let expected: HashSet<PullResource> = [
            PullResource::App,
            PullResource::Metrics,
            PullResource::Endpoints,
            PullResource::Jobs,
            PullResource::Errors,
            PullResource::Anomalies,
        ]
        .into_iter()
        .collect();
        assert_eq!(resources.into_iter().collect::<HashSet<_>>(), expected);
    }

    #[test]
    fn format_pull_summary_lists_counts() {
        let report = PullReport {
            app_id: 1,
            created: 2,
            skipped: 1,
            metric_points_added: 10,
            metric_points_skipped: 3,
            traces_created: 4,
            traces_skipped: 2,
            ..PullReport::default()
        };
        let summary = format_pull_summary(&report);
        assert!(summary.contains("2 snapshot(s) created"));
        assert!(summary.contains("10 metric point(s) added"));
        assert!(summary.contains("4 trace(s) stored"));
    }

    #[test]
    fn plan_pull_resolves_default_range() {
        let temp = std::env::temp_dir().join(format!("scout-plan-pull-{}", std::process::id()));
        std::fs::create_dir_all(&temp).unwrap();
        std::env::set_var("SCOUT_ARCHIVE_HOME", temp.to_string_lossy().as_ref());
        let store = ArchiveStore::from_env().unwrap();
        let options = PullOptions {
            from: None,
            to: None,
            range: Some("1day".to_string()),
            resources: PullResource::parse_list(&[]).unwrap(),
            metrics: Vec::new(),
            trace_ids: Vec::new(),
            trace_endpoint_limit: 50,
            force: false,
            incremental: false,
        };
        let plan = plan_pull(&store, 42, &options).unwrap();
        assert_eq!(plan.app_id, 42);
        assert_eq!(plan.chunk_count, 1);
        assert!(!plan.resources.is_empty());
        std::env::remove_var("SCOUT_ARCHIVE_HOME");
        let _ = std::fs::remove_dir_all(temp);
    }
}
