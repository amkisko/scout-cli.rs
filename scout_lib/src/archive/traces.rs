//! Fetch and store traces as explicit samples, not as a bulk compare unit.

use crate::archive::store::{ArchiveStore, StoreAction};
use crate::client::Client;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TracePullReport {
    pub created: u64,
    pub skipped: u64,
}

pub fn scan_limit(limit: u32) -> usize {
    if limit == 0 {
        usize::MAX
    } else {
        limit as usize
    }
}

pub async fn pull_trace_by_id(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    trace_id: u64,
    force: bool,
) -> Result<StoreAction, String> {
    if !force && store.entity_exists(app_id, "traces", &trace_id.to_string()) {
        store.record_existing_entity(app_id, "traces");
        return Ok(StoreAction::Skipped);
    }
    let trace = client
        .fetch_trace(app_id, trace_id)
        .await
        .map_err(|error| error.to_string())?;
    store.store_entity(app_id, "traces", &trace_id.to_string(), trace, force)
}

pub async fn pull_trace_ids(
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

#[allow(clippy::too_many_arguments)]
pub async fn pull_traces_for_range<F>(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    endpoint_limit: u32,
    force: bool,
    on_progress: F,
) -> Result<TracePullReport, String>
where
    F: Fn(&str),
{
    let mut report = TracePullReport::default();
    let endpoint_ids =
        endpoint_ids_for_range(client, store, app_id, from, to, endpoint_limit).await?;
    for (index, endpoint_id) in endpoint_ids.iter().enumerate() {
        on_progress(&format!(
            "  traces endpoint {}/{} ({endpoint_id})",
            index + 1,
            endpoint_ids.len()
        ));
        let listing = client
            .list_endpoint_traces(app_id, endpoint_id, Some(from), Some(to), None)
            .await
            .map_err(|error| error.to_string())?;
        store_listed_traces(client, store, app_id, &listing, force, &mut report).await?;
        on_progress(&format!(
            "  traces stored {} ({} already archived)",
            report.created, report.skipped
        ));
    }

    let job_ids = job_ids_for_range(client, store, app_id, from, to, endpoint_limit).await?;
    for (index, job_id) in job_ids.iter().enumerate() {
        on_progress(&format!(
            "  traces job {}/{} ({job_id})",
            index + 1,
            job_ids.len()
        ));
        let listing = client
            .list_job_traces(app_id, job_id, Some(from), Some(to), None)
            .await
            .map_err(|error| error.to_string())?;
        store_listed_traces(client, store, app_id, &listing, force, &mut report).await?;
        on_progress(&format!(
            "  traces stored {} ({} already archived)",
            report.created, report.skipped
        ));
    }
    Ok(report)
}

async fn endpoint_ids_for_range(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    limit: u32,
) -> Result<Vec<String>, String> {
    let snapshot = if store.range_snapshot_exists(app_id, "endpoints", from, to) {
        store.index_existing_range_snapshot(app_id, "endpoints", from, to)?;
        store.load_range_snapshot(app_id, "endpoints", from, to)?
    } else {
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
        store.store_range_snapshot(app_id, "endpoints", from, to, data, false)?;
        store.load_range_snapshot(app_id, "endpoints", from, to)?
    };
    Ok(ids_from_records(
        &crate::archive::diff::extract_endpoint_array_for_export(&snapshot.data),
        endpoint_id_from_record,
        limit,
    ))
}

async fn job_ids_for_range(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    from: &str,
    to: &str,
    limit: u32,
) -> Result<Vec<String>, String> {
    let snapshot = if store.range_snapshot_exists(app_id, "jobs", from, to) {
        store.index_existing_range_snapshot(app_id, "jobs", from, to)?;
        store.load_range_snapshot(app_id, "jobs", from, to)?
    } else {
        let data = client
            .list_jobs(app_id, Some(from), Some(to), None)
            .await
            .map_err(|error| error.to_string())?;
        store.store_range_snapshot(app_id, "jobs", from, to, data, false)?;
        store.load_range_snapshot(app_id, "jobs", from, to)?
    };
    Ok(ids_from_records(
        &crate::archive::diff::extract_jobs_array_for_export(&snapshot.data),
        job_id_from_record,
        limit,
    ))
}

async fn store_listed_traces(
    client: &Client,
    store: &mut ArchiveStore,
    app_id: u64,
    listing: &Value,
    force: bool,
    report: &mut TracePullReport,
) -> Result<(), String> {
    for trace_id in trace_ids_from_listing(listing) {
        match pull_trace_by_id(client, store, app_id, trace_id, force).await? {
            StoreAction::Created | StoreAction::Merged => report.created += 1,
            StoreAction::Skipped => report.skipped += 1,
        }
    }
    Ok(())
}

fn ids_from_records(
    records: &[Value],
    extract: fn(&Value) -> Option<String>,
    limit: u32,
) -> Vec<String> {
    records
        .iter()
        .filter_map(extract)
        .take(scan_limit(limit))
        .collect()
}

pub fn endpoint_id_from_record(endpoint: &Value) -> Option<String> {
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

pub fn job_id_from_record(job: &Value) -> Option<String> {
    if let Some(job_id) = job.get("job_id").and_then(Value::as_str) {
        if !job_id.is_empty() {
            return Some(job_id.to_string());
        }
    }
    if let Some(link) = job.get("link").and_then(Value::as_str) {
        if let Some((_, id)) = link.split_once("/jobs/") {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
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
    use serde_json::json;

    #[test]
    fn scan_limit_zero_means_all() {
        assert_eq!(scan_limit(0), usize::MAX);
        assert_eq!(scan_limit(50), 50);
    }

    #[test]
    fn endpoint_id_reads_link_suffix() {
        let endpoint = json!({"link": "/apps/1/endpoints/HomeController", "name": "Home#index"});
        assert_eq!(
            endpoint_id_from_record(&endpoint).as_deref(),
            Some("HomeController")
        );
    }

    #[test]
    fn job_id_prefers_job_id_field() {
        let job = json!({"job_id": "YWJj", "full_name": "default/Dummy"});
        assert_eq!(job_id_from_record(&job).as_deref(), Some("YWJj"));
    }
}
