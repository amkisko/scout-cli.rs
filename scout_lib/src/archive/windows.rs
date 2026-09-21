//! Per-resource ScoutAPM time windows for archive pulls.

use crate::archive::resource::PullResource;
use crate::archive::store::ArchiveStore;
use crate::helpers::{calculate_range, format_time, parse_time};
use chrono::{Duration, Utc};

const INCREMENTAL_OVERLAP_SECS: i64 = 3600;

pub fn resolve_requested_window(
    store: &ArchiveStore,
    app_id: u64,
    from: Option<&str>,
    to: Option<&str>,
    range: Option<&str>,
    incremental: bool,
) -> Result<(String, String), String> {
    if let Some(range) = range {
        let range = if range.eq_ignore_ascii_case("max") {
            "30days"
        } else {
            range
        };
        return calculate_range(range, to);
    }
    if let (Some(from), Some(to)) = (from, to) {
        return Ok((from.to_string(), to.to_string()));
    }
    if incremental {
        if let Some(last_to) = store
            .app_manifest(app_id)
            .and_then(|manifest| manifest.last_pull_to.clone())
        {
            let end = to
                .map(str::to_string)
                .unwrap_or_else(|| format_time(Utc::now()));
            let last_to_time = parse_time(&last_to)?;
            let start_time = last_to_time - Duration::seconds(INCREMENTAL_OVERLAP_SECS);
            return Ok((format_time(start_time), end));
        }
    }
    calculate_range("1day", to)
}

pub fn split_range(
    from: &str,
    to: &str,
    max_span_secs: i64,
) -> Result<Vec<(String, String)>, String> {
    let from_time = parse_time(from)?;
    let to_time = parse_time(to)?;
    if from_time >= to_time {
        return Err("from_time must be before to_time".to_string());
    }
    let mut chunks = Vec::new();
    let mut chunk_start = from_time;
    while chunk_start < to_time {
        let mut chunk_end = chunk_start + Duration::seconds(max_span_secs);
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

pub fn clamp_resource_window(
    resource: PullResource,
    from: &str,
    to: &str,
    now: chrono::DateTime<Utc>,
) -> Result<Option<(String, String)>, String> {
    let from_time = parse_time(from)?;
    let to_time = parse_time(to)?;
    if from_time >= to_time {
        return Ok(None);
    }
    let policy = resource.window_policy();
    let mut start = from_time;
    let end = to_time;
    if let Some(max_lookback_secs) = policy.max_lookback_secs {
        let earliest = now - Duration::seconds(max_lookback_secs);
        if end <= earliest {
            return Ok(None);
        }
        if start < earliest {
            start = earliest;
        }
    }
    if (end - start).num_seconds() > policy.max_span_secs {
        start = end - Duration::seconds(policy.max_span_secs);
    }
    if start >= end {
        return Ok(None);
    }
    Ok(Some((format_time(start), format_time(end))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::resource::{LOOKBACK_SLACK_SECS, MAX_PULL_SECS, REQUEST_SPAN_SECS};
    use chrono::TimeZone;

    fn noon() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 21, 12, 0, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn split_range_chunks_long_windows() {
        let from = "2025-01-01T00:00:00Z";
        let to = "2025-01-20T00:00:00Z";
        let chunks = split_range(from, to, REQUEST_SPAN_SECS).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, from);
        assert_eq!(chunks[1].1, to);
    }

    #[test]
    fn errors_clamp_shortens_windows_older_than_thirty_days() {
        let now = noon();
        let from = "2026-08-22T12:00:00Z";
        let to = "2026-09-05T12:00:00Z";
        let clamped = clamp_resource_window(PullResource::Errors, from, to, now)
            .unwrap()
            .expect("partial overlap");
        let start = parse_time(&clamped.0).unwrap();
        assert!(now - start <= Duration::seconds(MAX_PULL_SECS - LOOKBACK_SLACK_SECS));
        assert_eq!(clamped.1, to);
    }

    #[test]
    fn traces_skip_chunks_older_than_seven_days() {
        let now = noon();
        let skipped = clamp_resource_window(
            PullResource::Traces,
            "2026-08-22T12:00:00Z",
            "2026-09-05T12:00:00Z",
            now,
        )
        .unwrap();
        assert!(skipped.is_none());
    }

    #[test]
    fn traces_seven_day_range_is_under_the_api_cap() {
        let now = noon();
        let from = format_time(now - Duration::seconds(7 * 86400));
        let to = format_time(now);
        let clamped = clamp_resource_window(PullResource::Traces, &from, &to, now)
            .unwrap()
            .expect("traces window");
        let start = parse_time(&clamped.0).unwrap();
        assert!(now - start < Duration::seconds(7 * 86400));
        assert!(now - start >= Duration::seconds(7 * 86400 - LOOKBACK_SLACK_SECS - 1));
    }

    #[test]
    fn thirty_day_errors_chunks_stay_under_lookback_cap() {
        let now = noon();
        let from = format_time(now - Duration::seconds(30 * 86400));
        let to = format_time(now);
        let chunks = split_range(&from, &to, REQUEST_SPAN_SECS).unwrap();
        assert!(chunks.len() >= 2);
        let mut kept = 0u32;
        for (chunk_from, chunk_to) in chunks {
            let clamped =
                clamp_resource_window(PullResource::Errors, &chunk_from, &chunk_to, now).unwrap();
            let Some((start, end)) = clamped else {
                continue;
            };
            kept += 1;
            let start_time = parse_time(&start).unwrap();
            assert!(now - start_time <= Duration::seconds(MAX_PULL_SECS - LOOKBACK_SLACK_SECS));
            assert!(parse_time(&end).unwrap() > start_time);
        }
        assert!(kept >= 2);
    }

    #[test]
    fn metrics_keep_requested_window() {
        let now = noon();
        let from = "2026-08-22T12:00:00Z";
        let to = "2026-09-05T12:00:00Z";
        let clamped = clamp_resource_window(PullResource::Metrics, from, to, now)
            .unwrap()
            .unwrap();
        assert_eq!(clamped.0, from);
        assert_eq!(clamped.1, to);
    }

    #[test]
    fn max_range_resolves_to_thirty_days() {
        let (from, to) = calculate_range("30days", Some("2026-09-21T12:00:00Z")).unwrap();
        assert_eq!(to, "2026-09-21T12:00:00Z");
        assert_eq!(from, "2026-08-22T12:00:00Z");
    }
}
