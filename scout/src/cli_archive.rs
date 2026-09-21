//! Archive and diff clap subcommands.

use super::AppIdArgs;
use clap::Subcommand;

#[derive(Clone, Subcommand)]
pub enum DiffCommands {
    /// Diff endpoint snapshots for two stored time ranges
    Endpoints {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        left_from: String,
        #[arg(long)]
        left_to: String,
        #[arg(long)]
        right_from: String,
        #[arg(long)]
        right_to: String,
        #[arg(long)]
        left_label: Option<String>,
        #[arg(long)]
        right_label: Option<String>,
    },
    /// Diff daily metric buckets for two dates
    #[command(allow_missing_positional = true)]
    Metrics {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(value_parser = ["apdex", "response_time", "response_time_95th", "errors", "throughput", "queue_time"])]
        metric_type: String,
        #[arg(long)]
        left_date: String,
        #[arg(long)]
        right_date: String,
        #[arg(long)]
        left_label: Option<String>,
        #[arg(long)]
        right_label: Option<String>,
    },
    /// Diff error group snapshots for two stored time ranges
    Errors {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        left_from: String,
        #[arg(long)]
        left_to: String,
        #[arg(long)]
        right_from: String,
        #[arg(long)]
        right_to: String,
        #[arg(long)]
        left_label: Option<String>,
        #[arg(long)]
        right_label: Option<String>,
    },
    /// Diff background job snapshots for two stored time ranges
    Jobs {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        left_from: String,
        #[arg(long)]
        left_to: String,
        #[arg(long)]
        right_from: String,
        #[arg(long)]
        right_to: String,
        #[arg(long)]
        left_label: Option<String>,
        #[arg(long)]
        right_label: Option<String>,
    },
}

#[derive(Clone, Subcommand)]
pub enum ArchiveCommands {
    /// Print local archive directory
    Path,
    /// Show stored ranges and metric buckets
    Status {
        /// Application ID (omit for all apps)
        #[arg(value_name = "APP_ID")]
        app_id: Option<u64>,
    },
    /// Fetch aggregates from ScoutAPM and store them locally
    #[command(
        allow_missing_positional = true,
        after_help = "Default resources: app, metrics, endpoints, jobs, endpoint_metrics, job_metrics, errors, anomalies, insights.\n\
          Traces are samples, not a compare unit; use `archive trace` or `--trace-id`.\n\
          `--range max` is 30 days, then each resource clamps to its Scout cap.\n\
          Caps: errors and anomalies just under 30 days; traces just under 7 days.\n\
          Endpoints, jobs, and app metrics can look back further in 14-day request chunks.\n\
          One refused window is recorded; other resources in the pull keep going.\n\n\
          Examples:\n  \
          scout archive pull 123 --range 1day\n  \
          scout archive pull 123 --range max --dry-run\n  \
          scout archive pull 123 --range 30days --resource endpoints --resource metrics\n  \
          scout archive pull 123 --resource traces --range 6days --trace-endpoint-limit 0"
    )]
    Pull {
        #[command(flatten)]
        app: AppIdArgs,
        /// Inclusive start time (ISO 8601). Ignored when --range is set.
        #[arg(long)]
        from: Option<String>,
        /// Inclusive end time (ISO 8601). Defaults to now with --range.
        #[arg(long)]
        to: Option<String>,
        /// Window ending at --to or now: 30min, 1day, 7days, 30days, or max.
        #[arg(long)]
        range: Option<String>,
        /// Repeatable resource. Default omits traces. Also: endpoint_metrics, job_metrics, traces.
        #[arg(long = "resource")]
        resource: Vec<String>,
        /// Repeatable app metric type. Default: apdex, response_time, response_time_95th, errors, throughput, queue_time.
        #[arg(long = "metric")]
        metric: Vec<String>,
        #[arg(long, help = "Re-fetch even when a snapshot already exists")]
        force: bool,
        #[arg(long, help = "Continue from the last successful pull (1 hour overlap)")]
        incremental: bool,
        #[arg(long = "trace-id", help = "Fetch specific trace IDs (repeatable)")]
        trace_id: Vec<u64>,
        #[arg(
            long,
            default_value = "50",
            help = "Max endpoints/jobs to scan for traces or per-item series (0 = all)"
        )]
        trace_endpoint_limit: u32,
        #[arg(short = 'n', long, help = "Preview pull plan without calling the API")]
        dry_run: bool,
    },
    /// Fetch and store one trace by ID (idempotent)
    #[command(allow_missing_positional = true)]
    Trace {
        #[command(flatten)]
        app: AppIdArgs,
        trace_id: u64,
        #[arg(long, help = "Re-fetch even when the trace is already stored")]
        force: bool,
    },
    /// Export archived data for other systems (csv, prometheus, ndjson, parquet)
    Export {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        resource: String,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        metric: Option<String>,
        #[arg(long)]
        date: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(short, long, help = "Write to file instead of stdout")]
        output: Option<String>,
    },
}
