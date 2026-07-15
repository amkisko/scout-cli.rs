//! Command execution for API subcommands.

use crate::cli::Commands;
use crate::cli_error::ErrorContext;
use crate::output::{self, OutputMode};
use crate::util::{read_stdin_line, status_message, suggest_next_command};
use scout_lib::{parse_scout_url, Client, Error};
use serde_json::Value;

pub struct RunContext {
    pub mode: OutputMode,
    pub quiet: bool,
    pub app_id_override: Option<u64>,
    pub error_context: ErrorContext,
}

fn resolve_command_app_id(
    app: &crate::cli::AppIdArgs,
    override_id: Option<u64>,
) -> Result<u64, String> {
    crate::cli::resolve_app_id(app.app_id, override_id)
}

pub async fn run_api_command(
    client: &Client,
    command: Commands,
    context: &RunContext,
) -> Result<(), Error> {
    let value = api_command_value(client, command, context).await?;
    output::emit_value(context.mode, &value).map_err(Error::Other)?;
    Ok(())
}

pub async fn api_command_value(
    client: &Client,
    command: Commands,
    context: &RunContext,
) -> Result<Value, Error> {
    match command {
        Commands::Apps { active_since } => {
            status_message(context.quiet, "Fetching applications…");
            let apps = client.list_apps(active_since.as_deref()).await?;
            if let Some(first) = apps
                .first()
                .and_then(|app| app.get("id").and_then(|v| v.as_u64()))
            {
                suggest_next_command(
                    context.quiet,
                    &format!("Try: scout endpoints {first} --range 1day"),
                );
            }
            Ok(serde_json::to_value(&apps).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::App { app } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching application…");
            let app_value = client.get_app(app_id).await?;
            Ok(app_value)
        }
        Commands::Metrics { app } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching metric types…");
            let list = client.list_metrics(app_id).await?;
            Ok(serde_json::to_value(&list).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::Metric {
            app,
            metric_type,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching metric data…");
            let data = client
                .get_metric(
                    app_id,
                    &metric_type,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await?;
            Ok(data)
        }
        Commands::Endpoints {
            app,
            from,
            to,
            range,
            sort_by,
            limit,
            offset,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching endpoints…");
            let data = client
                .list_endpoints(
                    app_id,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                    sort_by.as_deref(),
                    limit,
                    offset,
                )
                .await?;
            suggest_next_command(
                context.quiet,
                &format!(
                    "Try: scout archive pull {app_id} --range 1day  (then scout diff endpoints {app_id} …)"
                ),
            );
            Ok(data)
        }
        Commands::EndpointMetric {
            app,
            endpoint_id,
            metric_type,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching endpoint metric…");
            let data = client
                .get_endpoint_metrics(
                    app_id,
                    &endpoint_id,
                    &metric_type,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await?;
            Ok(data)
        }
        Commands::EndpointTraces {
            app,
            endpoint_id,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            validate_trace_window(from.as_deref(), to.as_deref(), range.as_deref())?;
            status_message(context.quiet, "Fetching endpoint traces…");
            let data = client
                .list_endpoint_traces(
                    app_id,
                    &endpoint_id,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await?;
            Ok(data)
        }
        Commands::Jobs {
            app,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching jobs…");
            let data = client
                .list_jobs(app_id, from.as_deref(), to.as_deref(), range.as_deref())
                .await?;
            Ok(data)
        }
        Commands::JobMetrics { app, job_id } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching job metrics…");
            let list = client.list_job_metrics(app_id, &job_id).await?;
            Ok(serde_json::to_value(&list).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::JobMetric {
            app,
            job_id,
            metric_type,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching job metric…");
            let data = client
                .get_job_metrics(
                    app_id,
                    &job_id,
                    &metric_type,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await?;
            Ok(data)
        }
        Commands::JobTraces {
            app,
            job_id,
            from,
            to,
            range,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            validate_trace_window(from.as_deref(), to.as_deref(), range.as_deref())?;
            status_message(context.quiet, "Fetching job traces…");
            let data = client
                .list_job_traces(
                    app_id,
                    &job_id,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await?;
            Ok(data)
        }
        Commands::Trace { app, trace_id } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching trace…");
            let trace = client.fetch_trace(app_id, trace_id).await?;
            Ok(trace)
        }
        Commands::AnomalyEvents {
            app,
            from,
            to,
            range,
            state,
            metric,
            endpoint,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching anomaly events…");
            let list = client
                .list_anomaly_events(
                    app_id,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                    state.as_deref(),
                    metric.as_deref(),
                    endpoint.as_deref(),
                )
                .await?;
            Ok(serde_json::to_value(&list).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::AnomalyEvent {
            app,
            anomaly_event_id,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching anomaly event…");
            let event = client.get_anomaly_event(app_id, anomaly_event_id).await?;
            Ok(event)
        }
        Commands::Errors {
            app,
            from,
            to,
            endpoint,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching error groups…");
            let list = client
                .list_error_groups(app_id, from.as_deref(), to.as_deref(), endpoint.as_deref())
                .await?;
            Ok(serde_json::to_value(&list).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::Error { app, error_id } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching error group…");
            let err = client.get_error_group(app_id, error_id).await?;
            Ok(err)
        }
        Commands::ErrorGroupErrors { app, error_id } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching errors in group…");
            let list = client.get_error_group_errors(app_id, error_id).await?;
            Ok(serde_json::to_value(&list).map_err(|e| Error::Other(e.to_string()))?)
        }
        Commands::Insights { app, limit } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching insights…");
            let data = client.get_all_insights(app_id, limit).await?;
            Ok(data)
        }
        Commands::Insight {
            app,
            insight_type,
            limit,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching insight…");
            let data = client
                .get_insight_by_type(app_id, &insight_type, limit)
                .await?;
            Ok(data)
        }
        Commands::InsightsHistory {
            app,
            from,
            to,
            limit,
            pagination_cursor,
            pagination_direction,
            pagination_page,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching insights history…");
            let data = client
                .get_insights_history(
                    app_id,
                    from.as_deref(),
                    to.as_deref(),
                    limit,
                    pagination_cursor,
                    pagination_direction.as_deref(),
                    pagination_page,
                )
                .await?;
            Ok(data)
        }
        Commands::InsightsHistoryByType {
            app,
            insight_type,
            from,
            to,
            limit,
            pagination_cursor,
            pagination_direction,
            pagination_page,
        } => {
            let app_id =
                resolve_command_app_id(&app, context.app_id_override).map_err(Error::Other)?;
            status_message(context.quiet, "Fetching insights history…");
            let data = client
                .get_insights_history_by_type(
                    app_id,
                    &insight_type,
                    from.as_deref(),
                    to.as_deref(),
                    limit,
                    pagination_cursor,
                    pagination_direction.as_deref(),
                    pagination_page,
                )
                .await?;
            Ok(data)
        }
        Commands::ParseUrl { url } => {
            let url = if url == "-" {
                read_stdin_line().map_err(Error::Other)?
            } else {
                url.clone()
            };
            let parsed = parse_scout_url(&url).map_err(Error::Other)?;
            Ok(serde_json::to_value(&parsed).map_err(|error| Error::Other(error.to_string()))?)
        }
        Commands::Config { .. }
        | Commands::Completions { .. }
        | Commands::Diff { .. }
        | Commands::Archive { .. }
        | Commands::Batch { .. }
        | Commands::Man
        | Commands::Version => Err(Error::Other(
            "command is handled outside api_command_value".to_string(),
        )),
    }
}

fn validate_trace_window(
    from: Option<&str>,
    to: Option<&str>,
    range: Option<&str>,
) -> Result<(), Error> {
    if range.is_some() || (from.is_some() && to.is_some()) {
        return Ok(());
    }
    Err(Error::Other(
        "trace queries require --range or both --from and --to (max 7 days)".to_string(),
    ))
}
