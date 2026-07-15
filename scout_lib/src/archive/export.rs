//! Export archived data for import into other systems.

use crate::archive::metrics::MetricBucket;
use crate::archive::store::{ArchiveStore, RangeSnapshotFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    Ndjson,
    Csv,
    Prometheus,
    Parquet,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_lowercase().as_str() {
            "json" => Ok(ExportFormat::Json),
            "ndjson" | "jsonl" => Ok(ExportFormat::Ndjson),
            "csv" => Ok(ExportFormat::Csv),
            "prometheus" | "prom" => Ok(ExportFormat::Prometheus),
            "parquet" => Ok(ExportFormat::Parquet),
            other => Err(format!(
                "unknown export format '{other}'. Expected: json, ndjson, csv, prometheus, parquet"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportResource {
    Metrics,
    Endpoints,
    Jobs,
    Errors,
}

impl ExportResource {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_lowercase().as_str() {
            "metrics" => Ok(ExportResource::Metrics),
            "endpoints" => Ok(ExportResource::Endpoints),
            "jobs" => Ok(ExportResource::Jobs),
            "errors" => Ok(ExportResource::Errors),
            other => Err(format!(
                "unknown export resource '{other}'. Expected: metrics, endpoints, jobs, errors"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            ExportResource::Metrics => "metrics",
            ExportResource::Endpoints => "endpoints",
            ExportResource::Jobs => "jobs",
            ExportResource::Errors => "errors",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub app_id: u64,
    pub resource: ExportResource,
    pub format: ExportFormat,
    pub metric_type: Option<String>,
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportReport {
    pub app_id: u64,
    pub resource: String,
    pub format: String,
    pub records: u64,
    pub output: String,
}

pub fn export_archive(store: &ArchiveStore, request: &ExportRequest) -> Result<ExportReport, String> {
    validate_export_request(request)?;
    let payload = load_export_payload(store, request)?;
    let rendered = render_export(request, &payload)?;
    let output = write_export_output(request, &rendered)?;
    Ok(ExportReport {
        app_id: request.app_id,
        resource: request.resource.as_str().to_string(),
        format: format_name(request.format),
        records: count_records(&payload),
        output,
    })
}

enum ExportPayload {
    MetricBucket {
        metric_type: String,
        bucket: MetricBucket,
    },
    RangeSnapshot(RangeSnapshotFile),
}

fn validate_export_request(request: &ExportRequest) -> Result<(), String> {
    if request.format == ExportFormat::Prometheus && request.resource != ExportResource::Metrics {
        return Err("prometheus export supports metrics only".to_string());
    }
    Ok(())
}

fn load_export_payload(store: &ArchiveStore, request: &ExportRequest) -> Result<ExportPayload, String> {
    match request.resource {
        ExportResource::Metrics => {
            let metric_type = request
                .metric_type
                .clone()
                .ok_or_else(|| "metric type is required for metrics export".to_string())?;
            let date = request
                .date
                .clone()
                .ok_or_else(|| "date is required for metrics export (YYYY-MM-DD)".to_string())?;
            let bucket = store.load_metric_bucket(request.app_id, &metric_type, &date)?;
            Ok(ExportPayload::MetricBucket { metric_type, bucket })
        }
        ExportResource::Endpoints | ExportResource::Jobs | ExportResource::Errors => {
            let from = request
                .from
                .clone()
                .ok_or_else(|| "from is required for range export".to_string())?;
            let to = request
                .to
                .clone()
                .ok_or_else(|| "to is required for range export".to_string())?;
            let snapshot = store.load_range_snapshot(
                request.app_id,
                request.resource.as_str(),
                &from,
                &to,
            )?;
            Ok(ExportPayload::RangeSnapshot(snapshot))
        }
    }
}

fn count_records(payload: &ExportPayload) -> u64 {
    match payload {
        ExportPayload::MetricBucket { bucket, .. } => metric_points(&bucket.series).len() as u64,
        ExportPayload::RangeSnapshot(snapshot) => extract_records_for_resource(snapshot).len() as u64,
    }
}

fn render_export(request: &ExportRequest, payload: &ExportPayload) -> Result<Vec<u8>, String> {
    match request.format {
        ExportFormat::Json => render_json(payload),
        ExportFormat::Ndjson => render_ndjson(payload),
        ExportFormat::Csv => render_csv(request.resource, payload),
        ExportFormat::Prometheus => render_prometheus(request, payload),
        ExportFormat::Parquet => render_parquet(request, payload),
    }
}

fn render_json(payload: &ExportPayload) -> Result<Vec<u8>, String> {
    let value = payload_to_json(payload)?;
    serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())
}

fn render_ndjson(payload: &ExportPayload) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    for record in payload_records(payload)? {
        serde_json::to_writer(&mut output, &record).map_err(|error| error.to_string())?;
        output.push(b'\n');
    }
    Ok(output)
}

fn render_csv(resource: ExportResource, payload: &ExportPayload) -> Result<Vec<u8>, String> {
    let records = payload_records(payload)?;
    let columns = csv_columns(resource, &records);
    let mut output = Vec::new();
    writeln_csv_row(&mut output, &columns)?;
    for record in records {
        let row = columns
            .iter()
            .map(|column| csv_cell(&record, column))
            .collect::<Vec<_>>();
        writeln_csv_row(&mut output, &row)?;
    }
    Ok(output)
}

fn render_prometheus(request: &ExportRequest, payload: &ExportPayload) -> Result<Vec<u8>, String> {
    let ExportPayload::MetricBucket { metric_type, bucket } = payload else {
        return Err("prometheus export supports metrics only".to_string());
    };
    let mut output = String::new();
    let Some(series_object) = bucket.series.as_object() else {
        return Ok(output.into_bytes());
    };
    for (series_name, points_value) in series_object {
        let Some(points) = points_value.as_array() else {
            continue;
        };
        let metric_name = prometheus_metric_name(request.app_id, metric_type, series_name);
        for point in points {
            let Some(timestamp) = point.get("timestamp").and_then(Value::as_str) else {
                continue;
            };
            let Some(value) = point.get("value").and_then(Value::as_f64) else {
                continue;
            };
            let millis = prometheus_timestamp_millis(timestamp)?;
            output.push_str(&format!(
                "# TYPE {metric_name} gauge\n{metric_name}{{app_id=\"{}\",series=\"{}\",date=\"{}\"}} {value} {millis}\n",
                request.app_id, series_name, bucket.date
            ));
        }
    }
    Ok(output.into_bytes())
}

fn render_parquet(request: &ExportRequest, payload: &ExportPayload) -> Result<Vec<u8>, String> {
    #[cfg(feature = "export-parquet")]
    {
        parquet_export::render(request, payload)
    }
    #[cfg(not(feature = "export-parquet"))]
    {
        let _ = (request, payload);
        Err("parquet export is disabled in this build. Rebuild with --features export-parquet, or use ndjson/csv".to_string())
    }
}

fn payload_to_json(payload: &ExportPayload) -> Result<Value, String> {
    match payload {
        ExportPayload::MetricBucket { metric_type, bucket } => Ok(serde_json::json!({
            "metric_type": metric_type,
            "date": bucket.date,
            "series": bucket.series,
        })),
        ExportPayload::RangeSnapshot(snapshot) => Ok(serde_json::json!({
            "resource": snapshot.resource,
            "from": snapshot.from,
            "to": snapshot.to,
            "data": snapshot.data,
        })),
    }
}

fn payload_records(payload: &ExportPayload) -> Result<Vec<Value>, String> {
    match payload {
        ExportPayload::MetricBucket { metric_type, bucket } => Ok(metric_points(&bucket.series)
            .into_iter()
            .map(|(series_name, timestamp, value)| {
                serde_json::json!({
                    "metric_type": metric_type,
                    "date": bucket.date,
                    "series": series_name,
                    "timestamp": timestamp,
                    "value": value,
                })
            })
            .collect()),
        ExportPayload::RangeSnapshot(snapshot) => Ok(extract_records_for_resource(snapshot)),
    }
}

fn extract_records_for_resource(snapshot: &RangeSnapshotFile) -> Vec<Value> {
    match snapshot.resource.as_str() {
        "endpoints" => crate::archive::diff::extract_endpoint_array_for_export(&snapshot.data),
        "errors" => crate::archive::diff::extract_error_groups_for_export(&snapshot.data),
        "jobs" => crate::archive::diff::extract_jobs_array_for_export(&snapshot.data),
        _ => Vec::new(),
    }
}

fn metric_points(series: &Value) -> Vec<(String, String, f64)> {
    let mut points = Vec::new();
    let Some(series_object) = series.as_object() else {
        return points;
    };
    for (series_name, points_value) in series_object {
        let Some(series_points) = points_value.as_array() else {
            continue;
        };
        for point in series_points {
            let Some(timestamp) = point.get("timestamp").and_then(Value::as_str) else {
                continue;
            };
            let Some(value) = point.get("value").and_then(Value::as_f64) else {
                continue;
            };
            points.push((series_name.clone(), timestamp.to_string(), value));
        }
    }
    points
}

fn csv_columns(resource: ExportResource, records: &[Value]) -> Vec<String> {
    let mut columns = Vec::new();
    for record in records {
        let Some(map) = record.as_object() else {
            continue;
        };
        for key in map.keys() {
            if !columns.iter().any(|existing| existing == key) {
                columns.push(key.clone());
            }
        }
    }
    if columns.is_empty() {
        return match resource {
            ExportResource::Metrics => {
                vec![
                    "metric_type".to_string(),
                    "date".to_string(),
                    "series".to_string(),
                    "timestamp".to_string(),
                    "value".to_string(),
                ]
            }
            ExportResource::Endpoints => vec![
                "name".to_string(),
                "response_time".to_string(),
                "throughput".to_string(),
                "error_rate".to_string(),
            ],
            ExportResource::Jobs => vec![
                "full_name".to_string(),
                "throughput".to_string(),
                "execution_time".to_string(),
                "latency".to_string(),
            ],
            ExportResource::Errors => vec![
                "id".to_string(),
                "name".to_string(),
                "errors_count".to_string(),
            ],
        };
    }
    columns.sort();
    columns
}

fn csv_cell(record: &Value, column: &str) -> String {
    record
        .get(column)
        .map(value_to_csv_cell)
        .unwrap_or_default()
}

fn value_to_csv_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => escape_csv(text),
        other => escape_csv(&other.to_string()),
    }
}

fn escape_csv(text: &str) -> String {
    if text.contains(',') || text.contains('"') || text.contains('\n') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn writeln_csv_row(output: &mut Vec<u8>, cells: &[String]) -> Result<(), String> {
    let line = cells.join(",");
    output
        .write_all(line.as_bytes())
        .and_then(|_| output.write_all(b"\n"))
        .map_err(|error| error.to_string())
}

fn prometheus_metric_name(_app_id: u64, metric_type: &str, series_name: &str) -> String {
    let sanitized_series = series_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("scout_{metric_type}_{sanitized_series}")
}

fn prometheus_timestamp_millis(timestamp: &str) -> Result<u64, String> {
    let parsed = crate::helpers::parse_time(timestamp)?;
    Ok(parsed.timestamp_millis() as u64)
}

fn write_export_output(request: &ExportRequest, bytes: &[u8]) -> Result<String, String> {
    if let Some(path) = &request.output_path {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
        }
        fs::write(path, bytes).map_err(|error| error.to_string())?;
        Ok(path.clone())
    } else {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle
            .write_all(bytes)
            .map_err(|error| error.to_string())?;
        if !bytes.ends_with(b"\n") && request.format != ExportFormat::Parquet {
            handle.write_all(b"\n").map_err(|error| error.to_string())?;
        }
        Ok("stdout".to_string())
    }
}

fn format_name(format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => "json".to_string(),
        ExportFormat::Ndjson => "ndjson".to_string(),
        ExportFormat::Csv => "csv".to_string(),
        ExportFormat::Prometheus => "prometheus".to_string(),
        ExportFormat::Parquet => "parquet".to_string(),
    }
}

#[cfg(feature = "export-parquet")]
mod parquet_export {
    use super::*;
    use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray, UInt64Array};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::ArrowWriter;
    use parquet::basic::Compression;
    use parquet::file::properties::WriterProperties;
    use std::sync::Arc;

    pub fn render(request: &ExportRequest, payload: &ExportPayload) -> Result<Vec<u8>, String> {
        let records = payload_records(payload)?;
        let schema = Arc::new(Schema::new(vec![
            Field::new("app_id", DataType::UInt64, false),
            Field::new("resource", DataType::Utf8, false),
            Field::new("key", DataType::Utf8, true),
            Field::new("field", DataType::Utf8, true),
            Field::new("timestamp", DataType::Utf8, true),
            Field::new("value", DataType::Float64, true),
        ]));
        let mut app_ids = Vec::new();
        let mut resources = Vec::new();
        let mut keys: Vec<Option<String>> = Vec::new();
        let mut fields: Vec<Option<String>> = Vec::new();
        let mut timestamps: Vec<Option<String>> = Vec::new();
        let mut values: Vec<Option<f64>> = Vec::new();

        for record in records {
            if let Some((series, timestamp, value)) = metric_record_parts(&record) {
                app_ids.push(request.app_id);
                resources.push(request.resource.as_str().to_string());
                keys.push(Some(series));
                fields.push(Some(
                    request
                        .metric_type
                        .clone()
                        .unwrap_or_else(|| "metric".to_string()),
                ));
                timestamps.push(Some(timestamp));
                values.push(Some(value));
                continue;
            }
            flatten_record(
                request.app_id,
                request.resource.as_str(),
                &record,
                &mut app_ids,
                &mut resources,
                &mut keys,
                &mut fields,
                &mut timestamps,
                &mut values,
            );
        }

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(UInt64Array::from(app_ids)) as ArrayRef,
                Arc::new(StringArray::from(resources)),
                Arc::new(StringArray::from(keys)) as ArrayRef,
                Arc::new(StringArray::from(fields)) as ArrayRef,
                Arc::new(StringArray::from(timestamps)) as ArrayRef,
                Arc::new(Float64Array::from(values)) as ArrayRef,
            ],
        )
        .map_err(|error| error.to_string())?;

        let mut buffer = Vec::new();
        let properties = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();
        let mut writer = ArrowWriter::try_new(&mut buffer, schema, Some(properties))
            .map_err(|error| error.to_string())?;
        writer
            .write(&batch)
            .and_then(|_| writer.close())
            .map_err(|error| error.to_string())?;
        Ok(buffer)
    }

    fn metric_record_parts(record: &Value) -> Option<(String, String, f64)> {
        let series = record.get("series")?.as_str()?.to_string();
        let timestamp = record.get("timestamp")?.as_str()?.to_string();
        let value = record.get("value")?.as_f64()?;
        Some((series, timestamp, value))
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_record(
        app_id: u64,
        resource: &str,
        record: &Value,
        app_ids: &mut Vec<u64>,
        resources: &mut Vec<String>,
        keys: &mut Vec<Option<String>>,
        fields: &mut Vec<Option<String>>,
        timestamps: &mut Vec<Option<String>>,
        values: &mut Vec<Option<f64>>,
    ) {
        let key = record
            .get("name")
            .or_else(|| record.get("full_name"))
            .or_else(|| record.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string);
        for (field, value) in [
            ("response_time", record.get("response_time")),
            ("throughput", record.get("throughput")),
            ("error_rate", record.get("error_rate")),
            ("execution_time", record.get("execution_time")),
            ("latency", record.get("latency")),
            ("errors_count", record.get("errors_count")),
        ] {
            let Some(number) = value.and_then(|entry| entry.as_f64().or_else(|| entry.as_i64().map(|v| v as f64)))
            else {
                continue;
            };
            app_ids.push(app_id);
            resources.push(resource.to_string());
            keys.push(key.clone());
            fields.push(Some(field.to_string()));
            timestamps.push(None);
            values.push(Some(number));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::layout::ArchiveLayout;
    use crate::archive::store::ArchiveStore;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_store() -> ArchiveStore {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("scout-export-test-{nanos}"));
        ArchiveStore::open(ArchiveLayout::new(root)).unwrap()
    }

    #[test]
    fn csv_export_writes_metric_rows() {
        let mut store = temp_store();
        let series = json!({
            "avg": [{"timestamp": "2025-01-01T10:00:00Z", "value": 42.0}]
        });
        store
            .merge_metric_series(1, "response_time", &series, false)
            .unwrap();
        store.save_manifest().unwrap();
        let output_path = store
            .layout()
            .root()
            .join("export.csv")
            .to_string_lossy()
            .to_string();

        let report = export_archive(
            &store,
            &ExportRequest {
                app_id: 1,
                resource: ExportResource::Metrics,
                format: ExportFormat::Csv,
                metric_type: Some("response_time".to_string()),
                date: Some("2025-01-01".to_string()),
                from: None,
                to: None,
                output_path: Some(output_path.clone()),
            },
        )
        .unwrap();
        assert_eq!(report.records, 1);
        assert_eq!(report.format, "csv");
        let csv = std::fs::read_to_string(output_path).unwrap();
        assert!(csv.contains("42"));
        let _ = std::fs::remove_dir_all(store.layout().root());
    }

    #[test]
    fn prometheus_export_requires_metrics() {
        let store = temp_store();
        let err = export_archive(
            &store,
            &ExportRequest {
                app_id: 1,
                resource: ExportResource::Endpoints,
                format: ExportFormat::Prometheus,
                metric_type: None,
                date: None,
                from: Some("2025-01-01T00:00:00Z".to_string()),
                to: Some("2025-01-02T00:00:00Z".to_string()),
                output_path: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("prometheus export supports metrics only"));
    }
}
