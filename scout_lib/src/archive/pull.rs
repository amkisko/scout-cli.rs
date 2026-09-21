//! Pull ScoutAPM data into the local archive.

use crate::archive::fetch::{pull_anomalies, pull_endpoints, pull_errors, pull_jobs, pull_metrics};
use crate::archive::item_series::{pull_endpoint_metrics, pull_insights, pull_job_metrics};
use crate::archive::resource::REQUEST_SPAN_SECS;
use crate::archive::store::{ArchiveStore, StoreAction};
use crate::archive::traces::{pull_trace_ids, pull_traces_for_range};
use crate::archive::windows::{clamp_resource_window, resolve_requested_window, split_range};
use crate::client::Client;
use chrono::Utc;

pub use crate::archive::resource::{
    PullOptions, PullPlan, PullRefusal, PullReport, PullResource, DEFAULT_METRICS,
};
pub use crate::archive::traces::pull_trace_by_id;

pub fn plan_pull(
    store: &ArchiveStore,
    app_id: u64,
    options: &PullOptions,
) -> Result<PullPlan, String> {
    let (from, to) = resolve_requested_window(
        store,
        app_id,
        options.from.as_deref(),
        options.to.as_deref(),
        options.range.as_deref(),
        options.incremental,
    )?;
    let chunks = split_range(&from, &to, REQUEST_SPAN_SECS)?;
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
    if !report.refusals.is_empty() {
        parts.push(format!(
            "{} resource window(s) refused",
            report.refusals.len()
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

    store.reindex_app(app_id)?;
    store.save_manifest()?;

    let (from, to) = resolve_requested_window(
        store,
        app_id,
        options.from.as_deref(),
        options.to.as_deref(),
        options.range.as_deref(),
        options.incremental,
    )?;
    let chunks = split_range(&from, &to, REQUEST_SPAN_SECS)?;
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
        match pull_app_metadata(client, store, app_id, options.force).await {
            Ok(action) => match action {
                StoreAction::Created | StoreAction::Merged => report.created += 1,
                StoreAction::Skipped => report.skipped += 1,
            },
            Err(message) => push_refusal(&mut report, "app", "", "", message),
        }
        store.save_manifest()?;
    }

    if !options.trace_ids.is_empty() {
        progress(&format!(
            "Fetching {} trace(s) by ID…",
            options.trace_ids.len()
        ));
        match pull_trace_ids(client, store, app_id, &options.trace_ids, options.force).await {
            Ok(trace_report) => {
                report.traces_created += trace_report.created;
                report.traces_skipped += trace_report.skipped;
            }
            Err(message) => push_refusal(&mut report, "traces", &from, &to, message),
        }
        store.save_manifest()?;
    }

    let now = Utc::now();
    for (index, (chunk_from, chunk_to)) in chunks.iter().enumerate() {
        progress(&format!(
            "Chunk {}/{}: {chunk_from} .. {chunk_to}",
            index + 1,
            chunks.len()
        ));
        for resource in &options.resources {
            if matches!(resource, PullResource::App) {
                continue;
            }
            let Some((window_from, window_to)) =
                clamp_resource_window(*resource, chunk_from, chunk_to, now)?
            else {
                continue;
            };
            pull_one_resource(
                client,
                store,
                app_id,
                *resource,
                &window_from,
                &window_to,
                options,
                &mut report,
                progress,
            )
            .await;
            store.save_manifest()?;
        }
    }

    if report.refusals.is_empty() {
        store.record_pull_window(app_id, &from, &to);
    }
    store.save_manifest()?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
async fn pull_one_resource(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    resource: PullResource,
    from: &str,
    to: &str,
    options: &PullOptions,
    report: &mut PullReport,
    progress: impl Fn(&str),
) {
    let name = resource.as_str();
    progress(&format!("  {name}…"));
    let result = match resource {
        PullResource::App => Ok(()),
        PullResource::Metrics => {
            pull_metrics(client, store, app_id, from, to, options, report).await
        }
        PullResource::Endpoints => pull_endpoints(client, store, app_id, from, to, report).await,
        PullResource::Jobs => pull_jobs(client, store, app_id, from, to, report).await,
        PullResource::Errors => pull_errors(client, store, app_id, from, to, report).await,
        PullResource::Anomalies => pull_anomalies(client, store, app_id, from, to, report).await,
        PullResource::Insights => pull_insights(client, store, app_id, from, to, report).await,
        PullResource::EndpointMetrics => {
            pull_endpoint_metrics(client, store, app_id, from, to, options, report, progress).await
        }
        PullResource::JobMetrics => {
            pull_job_metrics(client, store, app_id, from, to, options, report, progress).await
        }
        PullResource::Traces => {
            match pull_traces_for_range(
                client,
                store,
                app_id,
                from,
                to,
                options.trace_endpoint_limit,
                options.force,
                |message| progress(message),
            )
            .await
            {
                Ok(trace_report) => {
                    report.traces_created += trace_report.created;
                    report.traces_skipped += trace_report.skipped;
                    Ok(())
                }
                Err(message) => Err(message),
            }
        }
    };
    if let Err(message) = result {
        push_refusal(report, name, from, to, message);
    }
}

fn push_refusal(report: &mut PullReport, resource: &str, from: &str, to: &str, message: String) {
    report.refusals.push(PullRefusal {
        resource: resource.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        message,
    });
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

#[cfg(test)]
mod tests {
    use super::*;

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
            refusals: vec![PullRefusal {
                resource: "errors".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                message: "from date cannot be older than 7 days".to_string(),
            }],
            ..PullReport::default()
        };
        let summary = format_pull_summary(&report);
        assert!(summary.contains("2 snapshot(s) created"));
        assert!(summary.contains("10 metric point(s) added"));
        assert!(summary.contains("4 trace(s) stored"));
        assert!(summary.contains("1 resource window(s) refused"));
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
        assert!(plan.resources.contains(&"insights".to_string()));
        assert!(!plan.resources.contains(&"traces".to_string()));
        std::env::remove_var("SCOUT_ARCHIVE_HOME");
        let _ = std::fs::remove_dir_all(temp);
    }
}
