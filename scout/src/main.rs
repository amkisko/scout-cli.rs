//! ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors from the terminal.

mod output;
mod tui;

use clap::{Parser, Subcommand, ValueEnum};
use scout_lib::{get_api_key, parse_scout_url, Client};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "scout")]
#[command(about = "ScoutAPM CLI — query apps, endpoints, traces, and metrics", long_about = None)]
#[command(subcommand_required = false)]
struct Cli {
    /// Output format: plain (human-readable), json (structured). Ignored for TUI.
    #[arg(short, long, default_value = "plain", value_enum)]
    output: OutputFormatArg,

    /// [TUI] Start with this app selected: numeric id or app name (case-insensitive).
    #[arg(long)]
    app: Option<String>,

    /// [TUI] Initial tab when opening an app: Endpoints, Insights, Metrics, or Errors.
    #[arg(long, default_value = "endpoints", value_enum)]
    tab: TuiTabArg,

    /// [TUI] Auto-refresh interval in seconds (0 = off). Re-fetches data for live view.
    #[arg(long, default_value = "0")]
    refresh: u64,

    /// [TUI] Show timestamps in UTC only. By default timestamps are shown in local timezone.
    #[arg(long)]
    utc: bool,

    /// When no subcommand is given, the interactive TUI is started.
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Copy, ValueEnum)]
enum TuiTabArg {
    Endpoints,
    Insights,
    Metrics,
    Errors,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Plain,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// List applications
    Apps {
        /// Filter apps active since (ISO 8601)
        #[arg(long)]
        active_since: Option<String>,
    },
    /// Show one application
    App { app_id: u64 },
    /// List available metric types
    Metrics { app_id: u64 },
    /// Get time-series metric data
    Metric {
        app_id: u64,
        #[arg(value_parser = ["apdex", "response_time", "response_time_95th", "errors", "throughput", "queue_time"])]
        metric_type: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },
    /// List endpoints
    Endpoints {
        app_id: u64,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },
    /// Get metric data for a specific endpoint
    EndpointMetric {
        app_id: u64,
        endpoint_id: String,
        #[arg(value_parser = ["apdex", "response_time", "response_time_95th", "errors", "throughput", "queue_time"])]
        metric_type: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },
    /// List traces for an endpoint (max 100, within 7 days)
    EndpointTraces {
        app_id: u64,
        endpoint_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },
    /// Fetch a trace
    Trace { app_id: u64, trace_id: u64 },
    /// List error groups
    Errors {
        app_id: u64,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Show one error group
    Error { app_id: u64, error_id: u64 },
    /// List individual errors in an error group (max 100)
    ErrorGroupErrors { app_id: u64, error_id: u64 },
    /// Get all insights
    Insights {
        app_id: u64,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get insight by type (n_plus_one, memory_bloat, slow_query)
    Insight {
        app_id: u64,
        #[arg(value_parser = ["n_plus_one", "memory_bloat", "slow_query"])]
        insight_type: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get insights history (cursor-based pagination)
    InsightsHistory {
        app_id: u64,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        pagination_cursor: Option<u64>,
        #[arg(long)]
        pagination_direction: Option<String>,
        #[arg(long)]
        pagination_page: Option<u32>,
    },
    /// Get insights history by type (cursor-based pagination)
    InsightsHistoryByType {
        app_id: u64,
        #[arg(value_parser = ["n_plus_one", "memory_bloat", "slow_query"])]
        insight_type: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        pagination_cursor: Option<u64>,
        #[arg(long)]
        pagination_direction: Option<String>,
        #[arg(long)]
        pagination_page: Option<u32>,
    },
    /// Parse a ScoutAPM URL and print extracted IDs
    ParseUrl { url: String },
    /// Show version
    Version,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    if matches!(cli.command, Some(Commands::Version)) {
        println!("scout {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let (api_key, _source) = match get_api_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let client = Client::new(api_key);
    let format = match cli.output {
        OutputFormatArg::Plain => output::OutputFormat::Plain,
        OutputFormatArg::Json => output::OutputFormat::Json,
    };

    // No subcommand → run interactive TUI
    if cli.command.is_none() {
        let tui_opts = tui::Options {
            app: cli.app.clone(),
            tab: match cli.tab {
                TuiTabArg::Endpoints => tui::Tab::Endpoints,
                TuiTabArg::Insights => tui::Tab::Insights,
                TuiTabArg::Metrics => tui::Tab::Metrics,
                TuiTabArg::Errors => tui::Tab::Errors,
            },
            refresh_secs: cli.refresh,
            use_utc: cli.utc,
        };
        return match tui::run(&client, tui_opts).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Error: {}", e);
                ExitCode::FAILURE
            }
        };
    }

    match run(&client, cli.command.unwrap(), format).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn run(client: &Client, cmd: Commands, format: output::OutputFormat) -> Result<(), String> {
    let print_value = |v: &serde_json::Value| match format {
        output::OutputFormat::Plain => println!("{}", output::format_plain(v)),
        output::OutputFormat::Json => println!("{}", output::format_json(v).unwrap()),
    };

    match cmd {
        Commands::Apps { active_since } => {
            let apps = client
                .list_apps(active_since.as_deref())
                .await
                .map_err(|e| e.to_string())?;
            print_value(&serde_json::to_value(&apps).unwrap());
        }
        Commands::App { app_id } => {
            let app = client.get_app(app_id).await.map_err(|e| e.to_string())?;
            print_value(&app);
        }
        Commands::Metrics { app_id } => {
            let list = client
                .list_metrics(app_id)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&serde_json::to_value(&list).unwrap());
        }
        Commands::Metric {
            app_id,
            metric_type,
            from,
            to,
            range,
        } => {
            let data = client
                .get_metric(
                    app_id,
                    &metric_type,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::Endpoints {
            app_id,
            from,
            to,
            range,
        } => {
            let data = client
                .list_endpoints(app_id, from.as_deref(), to.as_deref(), range.as_deref())
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::EndpointMetric {
            app_id,
            endpoint_id,
            metric_type,
            from,
            to,
            range,
        } => {
            let data = client
                .get_endpoint_metrics(
                    app_id,
                    &endpoint_id,
                    &metric_type,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::EndpointTraces {
            app_id,
            endpoint_id,
            from,
            to,
            range,
        } => {
            let data = client
                .list_endpoint_traces(
                    app_id,
                    &endpoint_id,
                    from.as_deref(),
                    to.as_deref(),
                    range.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::Trace { app_id, trace_id } => {
            let trace = client
                .fetch_trace(app_id, trace_id)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&trace);
        }
        Commands::Errors {
            app_id,
            from,
            to,
            endpoint,
        } => {
            let list = client
                .list_error_groups(app_id, from.as_deref(), to.as_deref(), endpoint.as_deref())
                .await
                .map_err(|e| e.to_string())?;
            print_value(&serde_json::to_value(&list).unwrap());
        }
        Commands::Error { app_id, error_id } => {
            let err = client
                .get_error_group(app_id, error_id)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&err);
        }
        Commands::ErrorGroupErrors { app_id, error_id } => {
            let list = client
                .get_error_group_errors(app_id, error_id)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&serde_json::to_value(&list).unwrap());
        }
        Commands::Insights { app_id, limit } => {
            let data = client
                .get_all_insights(app_id, limit)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::Insight {
            app_id,
            insight_type,
            limit,
        } => {
            let data = client
                .get_insight_by_type(app_id, &insight_type, limit)
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::InsightsHistory {
            app_id,
            from,
            to,
            limit,
            pagination_cursor,
            pagination_direction,
            pagination_page,
        } => {
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
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::InsightsHistoryByType {
            app_id,
            insight_type,
            from,
            to,
            limit,
            pagination_cursor,
            pagination_direction,
            pagination_page,
        } => {
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
                .await
                .map_err(|e| e.to_string())?;
            print_value(&data);
        }
        Commands::ParseUrl { url } => {
            let parsed = parse_scout_url(&url).map_err(|e| e.to_string())?;
            print_value(&serde_json::to_value(&parsed).unwrap());
        }
        Commands::Version => {}
    }
    Ok(())
}
