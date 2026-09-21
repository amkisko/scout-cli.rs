//! Idempotent merge of metric time series into daily buckets.

use crate::helpers::{format_time, parse_time};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricBucket {
    pub metric_type: String,
    pub date: String,
    pub series: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricMergeStats {
    pub added_points: u64,
    pub skipped_points: u64,
    pub buckets_touched: u64,
}

/// Split incoming series points into daily buckets and merge with existing bucket data.
/// Existing timestamps are never overwritten (Scout historical data is immutable).
pub fn merge_series_into_buckets(
    metric_type: &str,
    incoming_series: &Value,
    existing_buckets: &HashMap<String, MetricBucket>,
) -> (HashMap<String, MetricBucket>, MetricMergeStats) {
    let mut buckets = existing_buckets.clone();
    let mut stats = MetricMergeStats::default();
    let wrapped;
    let series_object = if let Some(object) = incoming_series.as_object() {
        object
    } else if incoming_series.as_array().is_some() {
        wrapped = json!({ metric_type: incoming_series });
        wrapped.as_object().expect("wrapped series object")
    } else {
        return (buckets, stats);
    };

    for (series_name, points_value) in series_object {
        let Some(points) = points_value.as_array() else {
            continue;
        };
        for point in points {
            let Some(normalized) = normalize_point(point) else {
                continue;
            };
            let timestamp = normalized
                .get("timestamp")
                .and_then(Value::as_str)
                .expect("normalized point timestamp");
            let Ok(parsed_time) = parse_time(timestamp) else {
                continue;
            };
            let date = parsed_time.format("%Y-%m-%d").to_string();
            let bucket = buckets.entry(date.clone()).or_insert_with(|| MetricBucket {
                metric_type: metric_type.to_string(),
                date: date.clone(),
                series: Value::Object(Map::new()),
            });
            let bucket_series = bucket.series.as_object_mut().expect("bucket series object");
            let series_points = bucket_series
                .entry(series_name.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            let series_array = series_points.as_array_mut().expect("series array");
            if series_array.iter().any(|existing| {
                existing
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .is_some_and(|existing_timestamp| existing_timestamp == timestamp)
            }) {
                stats.skipped_points += 1;
                continue;
            }
            series_array.push(normalized);
            stats.added_points += 1;
        }
    }

    stats.buckets_touched = buckets.len() as u64;
    sort_bucket_series(&mut buckets);
    (buckets, stats)
}

pub fn point_value(point: &Value) -> Option<f64> {
    if let Some(value) = point.get("value") {
        return numeric_value(value);
    }
    point.get(1).and_then(numeric_value)
}

fn normalize_point(point: &Value) -> Option<Value> {
    if let Some(timestamp) = point_timestamp(point) {
        let value = point_value(point)?;
        return Some(json!({ "timestamp": timestamp, "value": value }));
    }
    None
}

fn point_timestamp(point: &Value) -> Option<String> {
    if let Some(timestamp) = point
        .get("timestamp")
        .or_else(|| point.get("time"))
        .and_then(json_timestamp)
    {
        return Some(timestamp);
    }
    point.get(0).and_then(json_timestamp)
}

fn json_timestamp(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return parse_time(text).ok().map(format_time);
    }
    if let Some(seconds) = value.as_i64() {
        return Utc.timestamp_opt(seconds, 0).single().map(format_time);
    }
    if let Some(seconds) = value.as_f64() {
        return Utc
            .timestamp_opt(seconds as i64, 0)
            .single()
            .map(format_time);
    }
    None
}

fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|number| number as f64))
        .or_else(|| value.as_i64().map(|number| number as f64))
}

fn sort_bucket_series(buckets: &mut HashMap<String, MetricBucket>) {
    for bucket in buckets.values_mut() {
        let Some(series_object) = bucket.series.as_object_mut() else {
            continue;
        };
        for points in series_object.values_mut() {
            let Some(series_array) = points.as_array_mut() else {
                continue;
            };
            series_array.sort_by(|left, right| {
                let left_time = left
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(|value| parse_time(value).ok());
                let right_time = right
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(|value| parse_time(value).ok());
                left_time.cmp(&right_time)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_series() -> Value {
        json!({
            "avg": [
                {"timestamp": "2025-01-01T10:00:00Z", "value": 100.0},
                {"timestamp": "2025-01-01T11:00:00Z", "value": 110.0},
                {"timestamp": "2025-01-02T09:00:00Z", "value": 90.0}
            ]
        })
    }

    fn live_tuple_series() -> Value {
        json!({
            "throughput": [
                ["2026-09-20T12:10:00Z", 3.3],
                ["2026-09-20T12:20:00Z", 4.4],
                ["2026-09-21T09:00:00Z", 7.0]
            ]
        })
    }

    #[test]
    fn merge_splits_points_into_daily_buckets() {
        let (buckets, stats) =
            merge_series_into_buckets("response_time", &sample_series(), &HashMap::new());
        assert_eq!(stats.added_points, 3);
        assert_eq!(stats.skipped_points, 0);
        assert_eq!(buckets.len(), 2);
        assert!(buckets.contains_key("2025-01-01"));
        assert!(buckets.contains_key("2025-01-02"));
    }

    #[test]
    fn merge_is_idempotent_for_existing_timestamps() {
        let (first_buckets, first_stats) =
            merge_series_into_buckets("response_time", &sample_series(), &HashMap::new());
        assert_eq!(first_stats.added_points, 3);

        let (second_buckets, second_stats) =
            merge_series_into_buckets("response_time", &sample_series(), &first_buckets);
        assert_eq!(second_stats.added_points, 0);
        assert_eq!(second_stats.skipped_points, 3);
        assert_eq!(first_buckets, second_buckets);
    }

    #[test]
    fn merge_adds_only_new_timestamps() {
        let (existing_buckets, _) =
            merge_series_into_buckets("response_time", &sample_series(), &HashMap::new());
        let updated_series = json!({
            "avg": [
                {"timestamp": "2025-01-01T10:00:00Z", "value": 999.0},
                {"timestamp": "2025-01-03T08:00:00Z", "value": 80.0}
            ]
        });
        let (buckets, stats) =
            merge_series_into_buckets("response_time", &updated_series, &existing_buckets);
        assert_eq!(stats.added_points, 1);
        assert_eq!(stats.skipped_points, 1);
        let jan_first = buckets.get("2025-01-01").unwrap();
        let first_point = jan_first.series["avg"][0]["value"].as_f64().unwrap();
        assert_eq!(first_point, 100.0);
        assert!(buckets.contains_key("2025-01-03"));
    }

    #[test]
    fn merge_accepts_bare_tuple_array() {
        let series = json!([["2026-09-20T12:10:00Z", 3.3], ["2026-09-20T12:20:00Z", 4.4]]);
        let (buckets, stats) = merge_series_into_buckets("throughput", &series, &HashMap::new());
        assert_eq!(stats.added_points, 2);
        assert_eq!(buckets["2026-09-20"].series["throughput"][0]["value"], 3.3);
    }

    #[test]
    fn merge_accepts_live_tuple_series() {
        let (buckets, stats) =
            merge_series_into_buckets("throughput", &live_tuple_series(), &HashMap::new());
        assert_eq!(stats.added_points, 3);
        assert_eq!(buckets.len(), 2);
        let first = &buckets["2026-09-20"].series["throughput"][0];
        assert_eq!(first["timestamp"], "2026-09-20T12:10:00Z");
        assert_eq!(first["value"], 3.3);
    }

    #[test]
    fn merge_tuple_series_is_idempotent_after_normalize() {
        let (first, _) =
            merge_series_into_buckets("throughput", &live_tuple_series(), &HashMap::new());
        let (_second, stats) =
            merge_series_into_buckets("throughput", &live_tuple_series(), &first);
        assert_eq!(stats.added_points, 0);
        assert_eq!(stats.skipped_points, 3);
    }

    #[test]
    fn merge_ten_thousand_tuple_points_stays_linear() {
        let origin = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let points: Vec<Value> = (0..10_000)
            .map(|index| {
                let timestamp = origin + chrono::Duration::minutes(index);
                json!([format_time(timestamp), index as f64])
            })
            .collect();
        let series = json!({ "throughput": points });
        let started = std::time::Instant::now();
        let (buckets, stats) = merge_series_into_buckets("throughput", &series, &HashMap::new());
        let elapsed = started.elapsed();
        assert_eq!(stats.added_points, 10_000);
        assert!(buckets.len() > 1);
        assert!(
            elapsed.as_millis() < 2_000,
            "merge of 10000 points took {elapsed:?}"
        );
    }
}
