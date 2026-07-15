//! Idempotent merge of metric time series into daily buckets.

use crate::helpers::parse_time;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
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
    let Some(series_object) = incoming_series.as_object() else {
        return (buckets, stats);
    };

    for (series_name, points_value) in series_object {
        let Some(points) = points_value.as_array() else {
            continue;
        };
        for point in points {
            let Some(timestamp) = point.get("timestamp").and_then(Value::as_str) else {
                continue;
            };
            let Ok(parsed_time) = parse_time(timestamp) else {
                continue;
            };
            let date = parsed_time.format("%Y-%m-%d").to_string();
            let bucket = buckets
                .entry(date.clone())
                .or_insert_with(|| MetricBucket {
                    metric_type: metric_type.to_string(),
                    date: date.clone(),
                    series: Value::Object(Map::new()),
                });
            let bucket_series = bucket
                .series
                .as_object_mut()
                .expect("bucket series object");
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
            series_array.push(point.clone());
            stats.added_points += 1;
        }
    }

    stats.buckets_touched = buckets.len() as u64;
    sort_bucket_series(&mut buckets);
    (buckets, stats)
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
}
