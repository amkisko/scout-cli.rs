//! Diff archived ScoutAPM snapshots from local storage.

use crate::cli::{AppIdArgs, DiffCommands};
use crate::output::{self, OutputMode};
use scout_lib::{
    diff_endpoints, diff_errors, diff_jobs, diff_metric_buckets, ArchiveStore, DiffChange,
    DiffReport, MetricBucket, RangeSnapshotFile,
};

pub struct DiffContext {
    pub mode: OutputMode,
}

pub fn run_diff_command(command: DiffCommands, context: &DiffContext) -> Result<(), String> {
    let report = diff_command_report(command)?;
    emit_diff_report(context, &report)
}

pub fn diff_command_value(
    command: DiffCommands,
    _context: &DiffContext,
) -> Result<serde_json::Value, String> {
    let report = diff_command_report(command)?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

fn diff_command_report(command: DiffCommands) -> Result<DiffReport, String> {
    match command {
        DiffCommands::Endpoints {
            app,
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
        } => range_diff_report(
            app,
            "endpoints",
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
            diff_endpoints,
        ),
        DiffCommands::Errors {
            app,
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
        } => range_diff_report(
            app,
            "errors",
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
            diff_errors,
        ),
        DiffCommands::Jobs {
            app,
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
        } => range_diff_report(
            app,
            "jobs",
            left_from,
            left_to,
            right_from,
            right_to,
            left_label,
            right_label,
            diff_jobs,
        ),
        DiffCommands::Metrics {
            app,
            metric_type,
            left_date,
            right_date,
            left_label,
            right_label,
        } => metric_diff_report(
            app,
            &metric_type,
            left_date,
            right_date,
            left_label,
            right_label,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn range_diff_report(
    app: AppIdArgs,
    resource: &str,
    left_from: String,
    left_to: String,
    right_from: String,
    right_to: String,
    left_label: Option<String>,
    right_label: Option<String>,
    diff_fn: fn(&RangeSnapshotFile, &RangeSnapshotFile, &str, &str) -> DiffReport,
) -> Result<DiffReport, String> {
    let app_id = app.resolve()?;
    let store = ArchiveStore::from_env()?;
    let left =
        load_range_snapshot_with_hint(&store, app_id, resource, &left_from, &left_to, &left_label)?;
    let right = load_range_snapshot_with_hint(
        &store,
        app_id,
        resource,
        &right_from,
        &right_to,
        &right_label,
    )?;
    let left_name = left_label.unwrap_or_else(|| "left".to_string());
    let right_name = right_label.unwrap_or_else(|| "right".to_string());
    let report = diff_fn(&left, &right, &left_name, &right_name);
    Ok(report)
}

fn metric_diff_report(
    app: AppIdArgs,
    metric_type: &str,
    left_date: String,
    right_date: String,
    left_label: Option<String>,
    right_label: Option<String>,
) -> Result<DiffReport, String> {
    let app_id = app.resolve()?;
    let store = ArchiveStore::from_env()?;
    let left_bucket =
        load_metric_bucket_with_hint(&store, app_id, metric_type, &left_date, &left_label)?;
    let right_bucket =
        load_metric_bucket_with_hint(&store, app_id, metric_type, &right_date, &right_label)?;
    let left_name = left_label.unwrap_or_else(|| left_date.clone());
    let right_name = right_label.unwrap_or_else(|| right_date.clone());
    Ok(diff_metric_buckets(
        metric_type,
        &left_bucket,
        &right_bucket,
        &left_name,
        &right_name,
    ))
}

fn load_range_snapshot_with_hint(
    store: &ArchiveStore,
    app_id: u64,
    resource: &str,
    from: &str,
    to: &str,
    label: &Option<String>,
) -> Result<RangeSnapshotFile, String> {
    store
        .load_range_snapshot(app_id, resource, from, to)
        .map_err(|error| {
            archive_load_hint(
                error,
                app_id,
                resource,
                &format!("{from} .. {to}"),
                label.as_deref(),
            )
        })
}

fn load_metric_bucket_with_hint(
    store: &ArchiveStore,
    app_id: u64,
    metric_type: &str,
    date: &str,
    label: &Option<String>,
) -> Result<MetricBucket, String> {
    store
        .load_metric_bucket(app_id, metric_type, date)
        .map_err(|error| {
            archive_load_hint(
                error,
                app_id,
                &format!("metrics/{metric_type}"),
                date,
                label.as_deref(),
            )
        })
}

fn archive_load_hint(
    error: String,
    app_id: u64,
    resource: &str,
    window: &str,
    label: Option<&str>,
) -> String {
    if !looks_like_missing_archive(&error) {
        return error;
    }
    let side = label.map(|name| format!(" ({name})")).unwrap_or_default();
    format!(
        "{error}\nHint: no archived {resource} data for app {app_id}{side} ({window}). \
         Run `scout archive pull {app_id} --range 1day` or `scout archive status {app_id}`."
    )
}

fn looks_like_missing_archive(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("no such file")
        || lower.contains("not found")
        || lower.contains("os error 2")
        || lower.contains("could not read")
}

fn emit_diff_report(context: &DiffContext, report: &DiffReport) -> Result<(), String> {
    match context.mode {
        OutputMode::HumanPlain => {
            let formatted = format_diff_human(report);
            output::emit_text(&formatted, true).map_err(|error| error.to_string())
        }
        other => {
            let value = serde_json::to_value(report).map_err(|error| error.to_string())?;
            output::emit_value(other, &value).map_err(|error| error.to_string())
        }
    }
}

pub fn format_diff_human(report: &DiffReport) -> String {
    let mut output = format!(
        "{} diff: {} ({} .. {}) vs {} ({} .. {})\n",
        report.resource,
        report.left.label,
        report.left.from,
        report.left.to,
        report.right.label,
        report.right.from,
        report.right.to,
    );

    if report.changes.is_empty() {
        output.push_str("\nNo changes.\n");
        return output;
    }

    output.push_str(&format_diff_table(&report.changes));
    output
}

fn format_diff_table(changes: &[DiffChange]) -> String {
    let mut lines = vec![format!(
        "{:<24} {:<18} {:>10} {:>10} {:>10} {}",
        "key", "field", "left", "right", "delta", "status"
    )];
    for change in changes {
        lines.push(format!(
            "{:<24} {:<18} {:>10} {:>10} {:>10} {}",
            truncate_cell(&change.key, 24),
            truncate_cell(&change.field, 18),
            format_optional_number(change.left),
            format_optional_number(change.right),
            format_optional_delta(change.delta),
            change.status,
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn truncate_cell(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let end = text
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        format!("{}…", &text[..end])
    }
}

fn format_optional_number(value: Option<f64>) -> String {
    value
        .map(|number| format!("{number:.2}"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_optional_delta(value: Option<f64>) -> String {
    value
        .map(|delta| {
            if delta > 0.0 {
                format!("+{delta:.2}")
            } else {
                format!("{delta:.2}")
            }
        })
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scout_lib::DiffSide;

    #[test]
    fn format_diff_human_renders_table_header() {
        let report = DiffReport {
            resource: "endpoints".to_string(),
            left: DiffSide {
                label: "week1".to_string(),
                from: "2025-01-01T00:00:00Z".to_string(),
                to: "2025-01-02T00:00:00Z".to_string(),
            },
            right: DiffSide {
                label: "week2".to_string(),
                from: "2025-01-08T00:00:00Z".to_string(),
                to: "2025-01-09T00:00:00Z".to_string(),
            },
            changes: vec![DiffChange {
                key: "HomeController#index".to_string(),
                field: "response_time".to_string(),
                left: Some(100.0),
                right: Some(130.0),
                delta: Some(30.0),
                status: "worsened".to_string(),
            }],
        };
        let rendered = format_diff_human(&report);
        assert!(rendered.contains("endpoints diff"));
        assert!(rendered.contains("response_time"));
        assert!(rendered.contains("+30.00"));
        assert!(rendered.contains("worsened"));
    }

    #[test]
    fn archive_load_hint_adds_recovery_steps() {
        let message = archive_load_hint(
            "No such file or directory (os error 2)".to_string(),
            42,
            "endpoints",
            "2025-01-01 .. 2025-01-02",
            Some("left"),
        );
        assert!(message.contains("archive pull 42"));
        assert!(message.contains("archive status 42"));
    }
}
