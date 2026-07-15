//! Compare archived ScoutAPM snapshots from local storage.

use crate::archive::store::RangeSnapshotFile;
use crate::archive::metrics::MetricBucket;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffResource {
    Endpoints,
    Metrics,
    Errors,
    Jobs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffSide {
    pub label: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffChange {
    pub key: String,
    pub field: String,
    pub left: Option<f64>,
    pub right: Option<f64>,
    pub delta: Option<f64>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    pub resource: String,
    pub left: DiffSide,
    pub right: DiffSide,
    pub changes: Vec<DiffChange>,
}

pub fn diff_endpoints(
    left: &RangeSnapshotFile,
    right: &RangeSnapshotFile,
    left_label: &str,
    right_label: &str,
) -> DiffReport {
    diff_range_records(
        "endpoints",
        left,
        right,
        left_label,
        right_label,
        extract_endpoint_array,
        |record| {
            record
                .get("name")
                .or_else(|| record.get("formatted_method_name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        },
        &["response_time", "throughput", "error_rate", "95th_percentile"],
    )
}

pub fn diff_errors(
    left: &RangeSnapshotFile,
    right: &RangeSnapshotFile,
    left_label: &str,
    right_label: &str,
) -> DiffReport {
    diff_range_records(
        "errors",
        left,
        right,
        left_label,
        right_label,
        extract_error_groups,
        error_group_key,
        &["errors_count"],
    )
}

pub fn diff_jobs(
    left: &RangeSnapshotFile,
    right: &RangeSnapshotFile,
    left_label: &str,
    right_label: &str,
) -> DiffReport {
    diff_range_records(
        "jobs",
        left,
        right,
        left_label,
        right_label,
        extract_jobs_array,
        |record| {
            record
                .get("full_name")
                .or_else(|| record.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        },
        &["throughput", "execution_time", "latency", "time_consumed"],
    )
}

pub fn diff_metric_buckets(
    metric_type: &str,
    left: &MetricBucket,
    right: &MetricBucket,
    left_label: &str,
    right_label: &str,
) -> DiffReport {
    let left_map = metric_series_map(&left.series);
    let right_map = metric_series_map(&right.series);
    let mut keys: Vec<String> = left_map.keys().chain(right_map.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    let mut changes = Vec::new();
    for key in keys {
        let left_average = left_map.get(&key).copied();
        let right_average = right_map.get(&key).copied();
        if left_average == right_average {
            continue;
        }
        changes.push(DiffChange {
            key: key.clone(),
            field: metric_type.to_string(),
            left: left_average,
            right: right_average,
            delta: match (left_average, right_average) {
                (Some(left_number), Some(right_number)) => Some(right_number - left_number),
                _ => None,
            },
            status: match (left_average, right_average) {
                (Some(_), Some(_)) => "changed".to_string(),
                (Some(_), None) => "removed".to_string(),
                (None, Some(_)) => "added".to_string(),
                (None, None) => "changed".to_string(),
            },
        });
    }

    DiffReport {
        resource: format!("metrics/{metric_type}"),
        left: DiffSide {
            label: left_label.to_string(),
            from: left.date.clone(),
            to: left.date.clone(),
        },
        right: DiffSide {
            label: right_label.to_string(),
            from: right.date.clone(),
            to: right.date.clone(),
        },
        changes,
    }
}

pub fn extract_endpoint_array_for_export(data: &Value) -> Vec<Value> {
    extract_endpoint_array(data)
}

pub fn extract_error_groups_for_export(data: &Value) -> Vec<Value> {
    extract_error_groups(data)
}

pub fn extract_jobs_array_for_export(data: &Value) -> Vec<Value> {
    extract_jobs_array(data)
}

#[allow(clippy::too_many_arguments)]
fn diff_range_records(
    resource: &str,
    left: &RangeSnapshotFile,
    right: &RangeSnapshotFile,
    left_label: &str,
    right_label: &str,
    extract_records: fn(&Value) -> Vec<Value>,
    record_key: fn(&Value) -> String,
    numeric_fields: &[&str],
) -> DiffReport {
    let left_map = records_map(&extract_records(&left.data), record_key);
    let right_map = records_map(&extract_records(&right.data), record_key);
    let mut keys: Vec<String> = left_map.keys().chain(right_map.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    let mut changes = Vec::new();
    for key in keys {
        match (left_map.get(&key), right_map.get(&key)) {
            (Some(left_record), Some(right_record)) => {
                for field in numeric_fields {
                    let left_value = numeric_field(left_record, field);
                    let right_value = numeric_field(right_record, field);
                    if left_value == right_value {
                        continue;
                    }
                    changes.push(DiffChange {
                        key: key.clone(),
                        field: (*field).to_string(),
                        left: left_value,
                        right: right_value,
                        delta: match (left_value, right_value) {
                            (Some(left_number), Some(right_number)) => {
                                Some(right_number - left_number)
                            }
                            _ => None,
                        },
                        status: "changed".to_string(),
                    });
                }
            }
            (Some(_), None) => changes.push(DiffChange {
                key: key.clone(),
                field: resource.to_string(),
                left: Some(1.0),
                right: None,
                delta: None,
                status: "removed".to_string(),
            }),
            (None, Some(_)) => changes.push(DiffChange {
                key: key.clone(),
                field: resource.to_string(),
                left: None,
                right: Some(1.0),
                delta: None,
                status: "added".to_string(),
            }),
            (None, None) => {}
        }
    }

    DiffReport {
        resource: resource.to_string(),
        left: DiffSide {
            label: left_label.to_string(),
            from: left.from.clone(),
            to: left.to.clone(),
        },
        right: DiffSide {
            label: right_label.to_string(),
            from: right.from.clone(),
            to: right.to.clone(),
        },
        changes,
    }
}

fn records_map(
    records: &[Value],
    record_key: fn(&Value) -> String,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for record in records {
        map.insert(record_key(record), record.clone());
    }
    map
}

fn error_group_key(record: &Value) -> String {
    let name = record
        .get("name")
        .or_else(|| record.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if let Some(id) = record.get("id").and_then(Value::as_u64) {
        format!("{id}:{name}")
    } else {
        name.to_string()
    }
}

fn extract_endpoint_array(data: &Value) -> Vec<Value> {
    extract_named_array(data, &["endpoints"])
}

fn extract_error_groups(data: &Value) -> Vec<Value> {
    extract_named_array(data, &["error_groups"])
}

fn extract_jobs_array(data: &Value) -> Vec<Value> {
    extract_named_array(data, &["jobs"])
}

fn extract_named_array(data: &Value, names: &[&str]) -> Vec<Value> {
    if let Some(array) = data.as_array() {
        return array.clone();
    }
    for name in names {
        if let Some(array) = data.get(*name).and_then(Value::as_array) {
            return array.clone();
        }
    }
    if let Some(results) = data.get("results") {
        if let Some(array) = results.as_array() {
            return array.clone();
        }
        for name in names {
            if let Some(array) = results.get(*name).and_then(Value::as_array) {
                return array.clone();
            }
        }
    }
    Vec::new()
}

fn numeric_field(record: &Value, field: &str) -> Option<f64> {
    if field == "95th_percentile" {
        return record.get("95th_percentile").and_then(Value::as_f64);
    }
    record
        .get(field)
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|number| number as f64)))
}

fn metric_series_map(series: &Value) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    let Some(series_object) = series.as_object() else {
        return map;
    };
    for (series_name, points_value) in series_object {
        let Some(points) = points_value.as_array() else {
            continue;
        };
        let values: Vec<f64> = points
            .iter()
            .filter_map(|point| point.get("value").and_then(Value::as_f64))
            .collect();
        if values.is_empty() {
            continue;
        }
        let average = values.iter().sum::<f64>() / values.len() as f64;
        map.insert(series_name.clone(), average);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diff_endpoints_reports_metric_deltas() {
        let left = RangeSnapshotFile {
            resource: "endpoints".to_string(),
            from: "2025-01-01T00:00:00Z".to_string(),
            to: "2025-01-02T00:00:00Z".to_string(),
            fetched_at: "2025-01-02T01:00:00Z".to_string(),
            data: json!([
                {"name": "Home#index", "response_time": 100.0, "throughput": 10.0, "error_rate": 0.0}
            ]),
        };
        let right = RangeSnapshotFile {
            resource: "endpoints".to_string(),
            from: "2025-01-08T00:00:00Z".to_string(),
            to: "2025-01-09T00:00:00Z".to_string(),
            fetched_at: "2025-01-09T01:00:00Z".to_string(),
            data: json!([
                {"name": "Home#index", "response_time": 120.0, "throughput": 12.0, "error_rate": 0.0}
            ]),
        };
        let report = diff_endpoints(&left, &right, "week-ago", "today");
        assert_eq!(report.changes.len(), 2);
        let response_change = report
            .changes
            .iter()
            .find(|change| change.field == "response_time")
            .unwrap();
        assert_eq!(response_change.delta, Some(20.0));
    }

    #[test]
    fn diff_errors_reports_count_changes() {
        let left = RangeSnapshotFile {
            resource: "errors".to_string(),
            from: "2025-01-01T00:00:00Z".to_string(),
            to: "2025-01-02T00:00:00Z".to_string(),
            fetched_at: "2025-01-02T01:00:00Z".to_string(),
            data: json!({
                "error_groups": [
                    {"id": 1, "name": "NoMethodError", "errors_count": 5}
                ]
            }),
        };
        let right = RangeSnapshotFile {
            resource: "errors".to_string(),
            from: "2025-01-08T00:00:00Z".to_string(),
            to: "2025-01-09T00:00:00Z".to_string(),
            fetched_at: "2025-01-09T01:00:00Z".to_string(),
            data: json!({
                "error_groups": [
                    {"id": 1, "name": "NoMethodError", "errors_count": 12},
                    {"id": 2, "name": "Timeout", "errors_count": 1}
                ]
            }),
        };
        let report = diff_errors(&left, &right, "left", "right");
        assert!(
            report
                .changes
                .iter()
                .any(|change| change.key.contains("NoMethodError") && change.delta == Some(7.0))
        );
        assert!(
            report
                .changes
                .iter()
                .any(|change| change.status == "added" && change.key.contains("Timeout"))
        );
    }

    #[test]
    fn diff_jobs_reports_execution_time_delta() {
        let left = RangeSnapshotFile {
            resource: "jobs".to_string(),
            from: "2025-01-01T00:00:00Z".to_string(),
            to: "2025-01-02T00:00:00Z".to_string(),
            fetched_at: "2025-01-02T01:00:00Z".to_string(),
            data: json!([
                {"full_name": "default/Worker", "execution_time": 100.0, "throughput": 10.0}
            ]),
        };
        let right = RangeSnapshotFile {
            resource: "jobs".to_string(),
            from: "2025-01-08T00:00:00Z".to_string(),
            to: "2025-01-09T00:00:00Z".to_string(),
            fetched_at: "2025-01-09T01:00:00Z".to_string(),
            data: json!([
                {"full_name": "default/Worker", "execution_time": 150.0, "throughput": 8.0}
            ]),
        };
        let report = diff_jobs(&left, &right, "left", "right");
        let execution_change = report
            .changes
            .iter()
            .find(|change| change.field == "execution_time")
            .unwrap();
        assert_eq!(execution_change.delta, Some(50.0));
    }
}
