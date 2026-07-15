//! Local archive commands: pull, status, path, trace, export.

use crate::cli::{AppIdArgs, ArchiveCommands};
use crate::output::{self, OutputMode};
use crate::util;
use scout_lib::{
    archive_home, export_archive, format_pull_summary, plan_pull, pull_app_with_progress,
    pull_trace_by_id, ArchiveStore, Client, ExportFormat, ExportRequest, ExportResource,
    PullOptions, PullResource,
};
use serde_json::json;

pub struct ArchiveContext {
    pub mode: OutputMode,
    pub quiet: bool,
}

pub struct ArchivePullRequest {
    pub app: AppIdArgs,
    pub from: Option<String>,
    pub to: Option<String>,
    pub range: Option<String>,
    pub resource: Vec<String>,
    pub metric: Vec<String>,
    pub trace_id: Vec<u64>,
    pub trace_endpoint_limit: u32,
    pub force: bool,
    pub incremental: bool,
    pub dry_run: bool,
}

pub async fn archive_command_value(
    client: Option<&Client>,
    command: ArchiveCommands,
    context: &ArchiveContext,
) -> Result<serde_json::Value, String> {
    match command {
        ArchiveCommands::Path => archive_path_value(),
        ArchiveCommands::Status { app_id } => archive_status_value(app_id),
        ArchiveCommands::Export {
            app,
            resource,
            format,
            metric,
            date,
            from,
            to,
            output,
        } => archive_export_value(
            app,
            resource,
            format,
            metric,
            date,
            from,
            to,
            output,
        ),
        ArchiveCommands::Pull {
            app,
            from,
            to,
            range,
            resource,
            metric,
            trace_id,
            trace_endpoint_limit,
            force,
            incremental,
            dry_run: true,
        } => {
            let request = ArchivePullRequest {
                app,
                from,
                to,
                range,
                resource,
                metric,
                trace_id,
                trace_endpoint_limit,
                force,
                incremental,
                dry_run: true,
            };
            archive_pull_plan_value(&request)
        }
        ArchiveCommands::Pull {
            app,
            from,
            to,
            range,
            resource,
            metric,
            trace_id,
            trace_endpoint_limit,
            force,
            incremental,
            dry_run: false,
        } => {
            let client = client.ok_or("archive pull requires ScoutAPM API access")?;
            let request = ArchivePullRequest {
                app,
                from,
                to,
                range,
                resource,
                metric,
                trace_id,
                trace_endpoint_limit,
                force,
                incremental,
                dry_run: false,
            };
            archive_pull_value(client, request, context).await
        }
        ArchiveCommands::Trace {
            app,
            trace_id,
            force,
        } => {
            let client = client.ok_or("archive trace requires ScoutAPM API access")?;
            archive_trace_value(client, app, trace_id, force, context).await
        }
    }
}

fn archive_path_value() -> Result<serde_json::Value, String> {
    Ok(json!({
        "archive_home": archive_home().display().to_string(),
    }))
}

fn archive_status_value(app_id: Option<u64>) -> Result<serde_json::Value, String> {
    let store = ArchiveStore::from_env()?;
    let payload = if let Some(app_id) = app_id {
        let manifest = store.manifest();
        json!({
            "archive_home": store.layout().root().display().to_string(),
            "app_id": app_id,
            "manifest": manifest.apps.get(&app_id.to_string()),
        })
    } else {
        json!({
            "archive_home": store.layout().root().display().to_string(),
            "manifest": store.manifest(),
        })
    };
    serde_json::to_value(payload).map_err(|error| error.to_string())
}

fn archive_pull_plan_value(request: &ArchivePullRequest) -> Result<serde_json::Value, String> {
    let app_id = request.app.resolve()?;
    let store = ArchiveStore::from_env()?;
    let options = build_pull_options(request)?;
    let plan = plan_pull(&store, app_id, &options)?;
    serde_json::to_value(plan).map_err(|error| error.to_string())
}

async fn archive_pull_value(
    client: &Client,
    request: ArchivePullRequest,
    _context: &ArchiveContext,
) -> Result<serde_json::Value, String> {
    let app_id = request.app.resolve()?;
    let mut store = ArchiveStore::from_env()?;
    let options = build_pull_options(&request)?;
    let report = run_cancellable(pull_app_with_progress(
        client,
        &mut store,
        app_id,
        &options,
        |_| {},
    ))
    .await?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

async fn archive_trace_value(
    client: &Client,
    app: AppIdArgs,
    trace_id: u64,
    force: bool,
    _context: &ArchiveContext,
) -> Result<serde_json::Value, String> {
    let app_id = app.resolve()?;
    let mut store = ArchiveStore::from_env()?;
    let action = run_cancellable(pull_trace_by_id(
        client,
        &mut store,
        app_id,
        trace_id,
        force,
    ))
    .await?;
    store.save_manifest()?;
    serde_json::to_value(json!({
        "app_id": app_id,
        "trace_id": trace_id,
        "action": format!("{action:?}").to_lowercase(),
    }))
    .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
fn archive_export_value(
    app: AppIdArgs,
    resource: String,
    format: String,
    metric: Option<String>,
    date: Option<String>,
    from: Option<String>,
    to: Option<String>,
    output_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = ArchiveStore::from_env()?;
    let request = ExportRequest {
        app_id: app.resolve()?,
        resource: ExportResource::parse(&resource)?,
        format: ExportFormat::parse(&format)?,
        metric_type: metric,
        date,
        from,
        to,
        output_path,
    };
    let report = export_archive(&store, &request)?;
    if report.output == "stdout" && matches!(request.format, ExportFormat::Parquet) {
        return Ok(json!({ "output": "stdout", "format": "parquet" }));
    }
    serde_json::to_value(report).map_err(|error| error.to_string())
}

pub fn run_archive_local_command(
    command: ArchiveCommands,
    context: &ArchiveContext,
) -> Result<(), String> {
    match command {
        ArchiveCommands::Path => run_path(context),
        ArchiveCommands::Status { app_id } => run_status(app_id, context),
        ArchiveCommands::Export {
            app,
            resource,
            format,
            metric,
            date,
            from,
            to,
            output,
        } => run_export(app, resource, format, metric, date, from, to, output, context),
        ArchiveCommands::Pull {
            app,
            from,
            to,
            range,
            resource,
            metric,
            trace_id,
            trace_endpoint_limit,
            force,
            incremental,
            dry_run,
        } if dry_run => run_archive_pull_plan(
            ArchivePullRequest {
                app,
                from,
                to,
                range,
                resource,
                metric,
                trace_id,
                trace_endpoint_limit,
                force,
                incremental,
                dry_run: true,
            },
            context,
        ),
        ArchiveCommands::Pull { .. } | ArchiveCommands::Trace { .. } => {
            Err("archive pull/trace requires API access; use the async entry point".to_string())
        }
    }
}

pub fn run_archive_pull_plan(
    request: ArchivePullRequest,
    context: &ArchiveContext,
) -> Result<(), String> {
    let value = archive_pull_plan_value(&request)?;
    output::emit_value(context.mode, &value).map_err(|error| error.to_string())
}

pub async fn run_archive_pull(
    client: &Client,
    request: ArchivePullRequest,
    context: &ArchiveContext,
) -> Result<(), String> {
    if request.dry_run {
        return run_archive_pull_plan(request, context);
    }

    let app_id = request.app.resolve()?;
    let mut store = ArchiveStore::from_env()?;
    let options = build_pull_options(&request)?;
    if !context.quiet {
        eprintln!("Pulling app {app_id} into {}", archive_home().display());
    }

    let quiet = context.quiet;
    let report = run_cancellable(pull_app_with_progress(
        client,
        &mut store,
        app_id,
        &options,
        move |message| util::progress_message(quiet, message),
    ))
    .await?;

    if !context.quiet {
        eprintln!("{}", format_pull_summary(&report));
        util::suggest_next_command(
            context.quiet,
            &format!("Try: scout archive status {app_id}"),
        );
    }

    let value = serde_json::to_value(report).map_err(|error| error.to_string())?;
    output::emit_value(context.mode, &value).map_err(|error| error.to_string())
}

pub async fn run_archive_trace(
    client: &Client,
    app: AppIdArgs,
    trace_id: u64,
    force: bool,
    context: &ArchiveContext,
) -> Result<(), String> {
    let app_id = app.resolve()?;
    let mut store = ArchiveStore::from_env()?;
    if !context.quiet {
        eprintln!("Archiving trace {trace_id} for app {app_id}");
    }
    let action = run_cancellable(pull_trace_by_id(
        client,
        &mut store,
        app_id,
        trace_id,
        force,
    ))
    .await?;
    store.save_manifest()?;
    if !context.quiet {
        eprintln!("Trace archive complete: {action:?}");
    }
    let value = serde_json::to_value(json!({
        "app_id": app_id,
        "trace_id": trace_id,
        "action": format!("{action:?}").to_lowercase(),
    }))
    .map_err(|error| error.to_string())?;
    output::emit_value(context.mode, &value).map_err(|error| error.to_string())
}

async fn run_cancellable<T>(future: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    tokio::select! {
        result = future => result,
        _ = tokio::signal::ctrl_c() => Err("Interrupted.".to_string()),
    }
}

fn build_pull_options(request: &ArchivePullRequest) -> Result<PullOptions, String> {
    Ok(PullOptions {
        from: request.from.clone(),
        to: request.to.clone(),
        range: request.range.clone(),
        resources: PullResource::parse_list(&request.resource)?,
        metrics: request.metric.clone(),
        trace_ids: request.trace_id.clone(),
        trace_endpoint_limit: request.trace_endpoint_limit,
        force: request.force,
        incremental: request.incremental,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_export(
    app: AppIdArgs,
    resource: String,
    format: String,
    metric: Option<String>,
    date: Option<String>,
    from: Option<String>,
    to: Option<String>,
    output_path: Option<String>,
    context: &ArchiveContext,
) -> Result<(), String> {
    let store = ArchiveStore::from_env()?;
    let request = ExportRequest {
        app_id: app.resolve()?,
        resource: ExportResource::parse(&resource)?,
        format: ExportFormat::parse(&format)?,
        metric_type: metric,
        date,
        from,
        to,
        output_path,
    };
    let report = export_archive(&store, &request)?;
    if report.output == "stdout" && matches!(request.format, ExportFormat::Parquet) {
        return Ok(());
    }
    if report.output != "stdout" {
        let value = serde_json::to_value(report).map_err(|error| error.to_string())?;
        output::emit_value(context.mode, &value).map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

fn run_path(context: &ArchiveContext) -> Result<(), String> {
    let payload = archive_path_value()?;
    output::emit_value(context.mode, &payload).map_err(|error| error.to_string())
}

fn run_status(app_id: Option<u64>, context: &ArchiveContext) -> Result<(), String> {
    let value = archive_status_value(app_id)?;
    output::emit_value(context.mode, &value).map_err(|error| error.to_string())
}
