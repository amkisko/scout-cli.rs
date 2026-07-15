//! Clap CLI definition.

use clap::{Args, Parser, Subcommand, ValueEnum};

pub const LONG_ABOUT: &str = "\
Query ScoutAPM apps, endpoints, traces, metrics, and errors from the terminal.

Examples:
  scout apps
  scout endpoints 123 --range 1day
  scout metric 123 response_time --range 7days
  scout parse-url \"https://scoutapm.com/apps/123\"
  echo '[{\"args\":[\"archive\",\"path\"]}]' | scout batch

Documentation: https://github.com/amkisko/scout-cli.rs
API reference: https://github.com/amkisko/scout-cli.rs/blob/main/doc/openapi.yaml
Report issues: https://github.com/amkisko/scout-cli.rs/issues";

pub const AFTER_HELP: &str = "\
Output:
  -o plain          Human-readable tables (default)
  --plain           Script-stable tab-separated records
  -o json           Pretty JSON (backward compatible)
  --json            Compact JSON for scripts

Batch (`scout batch`): stdout is always a JSON report; use --json-pretty for indented output.

Interactive TUI (no subcommand): --app, --tab, --refresh, --utc

Exit codes: 0 success, 1 general error, 2 usage, 3 auth, 4 API, 5 I/O

Run `scout help <command>` for command-specific examples.";

#[derive(Parser)]
#[command(
    name = "scout",
    version,
    author,
    about = "ScoutAPM CLI — query apps, endpoints, traces, and metrics",
    long_about = LONG_ABOUT,
    after_help = AFTER_HELP,
    subcommand_required = false,
    arg_required_else_help = false,
    disable_help_subcommand = false
)]
pub struct Cli {
    /// Output format: plain (human-readable) or json (pretty JSON).
    #[arg(
        short,
        long,
        global = true,
        default_value = "plain",
        value_enum,
        env = "SCOUT_OUTPUT"
    )]
    pub output: OutputFormatArg,

    /// Emit compact JSON (scripts). Overrides --output.
    #[arg(long, global = true)]
    pub json: bool,

    /// Emit script-stable plain text (one record per line).
    #[arg(long, global = true, conflicts_with = "json")]
    pub plain: bool,

    /// Emit pretty JSON explicitly.
    #[arg(long, global = true, conflicts_with_all = ["json", "plain"])]
    pub json_pretty: bool,

    /// Suppress non-essential stderr output.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Show debug details on errors.
    #[arg(short, long, global = true, env = "SCOUT_DEBUG")]
    pub debug: bool,

    /// Show extra error context (status codes, response hints).
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Disable color in supported output.
    #[arg(long, global = true, env = "SCOUT_NO_COLOR")]
    pub no_color: bool,

    /// Disable interactive prompts and the default TUI.
    #[arg(long, global = true)]
    pub no_input: bool,

    /// HTTP timeout in seconds (default: 15).
    #[arg(long, global = true, env = "SCOUT_TIMEOUT")]
    pub timeout: Option<u64>,

    /// Override the ScoutAPM API base URL.
    #[arg(long, global = true, env = "SCOUT_API_BASE")]
    pub api_base: Option<String>,

    /// Override the application ID for subcommands that take APP_ID.
    #[arg(long = "app-id", global = true, value_name = "APP_ID")]
    pub app_id: Option<u64>,

    /// Start with this app selected in the interactive TUI.
    #[arg(long, global = true, hide = true)]
    pub app: Option<String>,

    /// Initial tab when opening an app in the interactive TUI.
    #[arg(
        long,
        default_value = "endpoints",
        value_enum,
        global = true,
        hide = true
    )]
    pub tab: TuiTabArg,

    /// Auto-refresh interval in seconds in the interactive TUI (0 = off).
    #[arg(long, default_value = "0", global = true, hide = true)]
    pub refresh: u64,

    /// Show timestamps in UTC only in the interactive TUI.
    #[arg(long, global = true, hide = true)]
    pub utc: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TuiTabArg {
    Endpoints,
    Insights,
    Metrics,
    Errors,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormatArg {
    Plain,
    Json,
}

#[derive(Clone, Args)]
pub struct AppIdArgs {
    /// Application ID (or pass `--app-id`).
    #[arg(value_name = "APP_ID")]
    pub app_id: u64,
}

impl AppIdArgs {
    pub fn resolve(&self) -> Result<u64, String> {
        Ok(self.app_id)
    }
}

pub fn resolve_app_id(positional: u64, flag: Option<u64>) -> Result<u64, String> {
    match flag {
        Some(flag_id) if flag_id != positional => {
            Err("provide only one of APP_ID or --app-id".to_string())
        }
        _ => Ok(positional),
    }
}

#[derive(Clone, Subcommand)]
pub enum Commands {
    /// List applications
    #[command(
        visible_alias = "ls",
        after_help = "Examples:\n  scout apps\n  scout apps --active-since 2025-01-01T00:00:00Z"
    )]
    #[command(next_help_heading = "Apps")]
    Apps {
        #[arg(long)]
        active_since: Option<String>,
    },

    /// Show one application
    #[command(arg_required_else_help = true, next_help_heading = "Apps")]
    App {
        #[command(flatten)]
        app: AppIdArgs,
    },

    /// List available metric types
    #[command(arg_required_else_help = true, next_help_heading = "Metrics")]
    Metrics {
        #[command(flatten)]
        app: AppIdArgs,
    },

    /// Get time-series metric data
    #[command(arg_required_else_help = true, next_help_heading = "Metrics")]
    Metric {
        #[command(flatten)]
        app: AppIdArgs,
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
    #[command(arg_required_else_help = true, next_help_heading = "Endpoints")]
    Endpoints {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
        #[arg(long, value_parser = ["time_consumed", "response_time", "throughput", "error_rate"])]
        sort_by: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
    },

    /// Get metric data for a specific endpoint
    #[command(arg_required_else_help = true, next_help_heading = "Metrics")]
    EndpointMetric {
        #[command(flatten)]
        app: AppIdArgs,
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
    #[command(arg_required_else_help = true, next_help_heading = "Traces")]
    EndpointTraces {
        #[command(flatten)]
        app: AppIdArgs,
        endpoint_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },

    /// List background jobs
    #[command(arg_required_else_help = true, next_help_heading = "Jobs")]
    Jobs {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },

    /// List available job metrics
    #[command(arg_required_else_help = true, next_help_heading = "Jobs")]
    JobMetrics {
        #[command(flatten)]
        app: AppIdArgs,
        job_id: String,
    },

    /// Get job metrics
    #[command(arg_required_else_help = true, next_help_heading = "Jobs")]
    JobMetric {
        #[command(flatten)]
        app: AppIdArgs,
        job_id: String,
        #[arg(value_parser = ["throughput", "execution_time", "latency", "errors", "allocations"])]
        metric_type: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },

    /// List traces for a job (max 100, within 7 days)
    #[command(arg_required_else_help = true, next_help_heading = "Traces")]
    JobTraces {
        #[command(flatten)]
        app: AppIdArgs,
        job_id: String,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
    },

    /// Fetch a trace
    #[command(arg_required_else_help = true, next_help_heading = "Traces")]
    Trace {
        #[command(flatten)]
        app: AppIdArgs,
        trace_id: u64,
    },

    /// List anomaly events (max 100, within 30 days)
    #[command(arg_required_else_help = true, next_help_heading = "Anomalies")]
    AnomalyEvents {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
        #[arg(long, value_parser = ["open", "closed", "all"])]
        state: Option<String>,
        #[arg(long)]
        metric: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
    },

    /// Show one anomaly event
    #[command(arg_required_else_help = true, next_help_heading = "Anomalies")]
    AnomalyEvent {
        #[command(flatten)]
        app: AppIdArgs,
        anomaly_event_id: u64,
    },

    /// List error groups
    #[command(arg_required_else_help = true, next_help_heading = "Errors")]
    Errors {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
    },

    /// Show one error group
    #[command(arg_required_else_help = true, next_help_heading = "Errors")]
    Error {
        #[command(flatten)]
        app: AppIdArgs,
        error_id: u64,
    },

    /// List individual errors in an error group (max 100)
    #[command(arg_required_else_help = true, next_help_heading = "Errors")]
    ErrorGroupErrors {
        #[command(flatten)]
        app: AppIdArgs,
        error_id: u64,
    },

    /// Get all insights
    #[command(arg_required_else_help = true, next_help_heading = "Insights")]
    Insights {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Get insight by type
    #[command(arg_required_else_help = true, next_help_heading = "Insights")]
    Insight {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(value_parser = ["n_plus_one", "memory_bloat", "slow_query"])]
        insight_type: String,
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Get insights history (cursor-based pagination)
    #[command(arg_required_else_help = true, next_help_heading = "Insights")]
    InsightsHistory {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        pagination_cursor: Option<u64>,
        #[arg(long, value_parser = ["forward", "backward"])]
        pagination_direction: Option<String>,
        #[arg(long)]
        pagination_page: Option<u32>,
    },

    /// Get insights history by type
    #[command(arg_required_else_help = true, next_help_heading = "Insights")]
    InsightsHistoryByType {
        #[command(flatten)]
        app: AppIdArgs,
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
        #[arg(long, value_parser = ["forward", "backward"])]
        pagination_direction: Option<String>,
        #[arg(long)]
        pagination_page: Option<u32>,
    },

    /// Parse a ScoutAPM URL and print extracted IDs (no API key required)
    #[command(
        arg_required_else_help = true,
        after_help = "Examples:\n  \
          scout parse-url \"https://scoutapm.com/apps/123\"\n  \
          echo 'https://scoutapm.com/apps/123' | scout parse-url -",
        next_help_heading = "Utilities"
    )]
    ParseUrl {
        /// URL to parse, or `-` to read from stdin
        url: String,
    },

    /// Manage Scout config (`scout config path` shows the directory)
    #[command(next_help_heading = "Configuration")]
    Config {
        #[command(flatten)]
        options: ConfigOptions,
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Generate shell completions
    #[command(next_help_heading = "Utilities")]
    Completions {
        shell: crate::completions::CompletionShell,
    },

    /// Print the man page to stdout
    #[command(next_help_heading = "Utilities")]
    Man,

    /// Show version
    #[command(next_help_heading = "Utilities")]
    Version,

    /// Compare archived snapshots from local storage (no API calls)
    #[command(
        after_help = "Examples:\n  \
          scout diff endpoints 123 --left-from 2025-01-01T00:00:00Z --left-to 2025-01-02T00:00:00Z \
          --right-from 2025-01-08T00:00:00Z --right-to 2025-01-09T00:00:00Z\n  \
          scout diff metrics 123 response_time --left-date 2025-01-01 --right-date 2025-01-08",
        next_help_heading = "Archive"
    )]
    Diff {
        #[command(subcommand)]
        command: DiffCommands,
    },

    /// Pull and store ScoutAPM data locally for long-term analysis
    #[command(
        after_help = "Examples:\n  \
          scout archive pull 123 --range 1day\n  \
          scout archive pull 123 --incremental\n  \
          scout archive status 123",
        next_help_heading = "Archive"
    )]
    Archive {
        #[command(subcommand)]
        command: ArchiveCommands,
    },

    /// Run multiple scout operations from a JSON plan
    #[command(
        after_help = "Input format:\n  \
          [{\"id\":\"optional-label\",\"args\":[\"archive\",\"path\"]}]\n  \
          or {\"operations\":[...]}\n\n\
          Output:\n  \
          Always a JSON report on stdout with ok, data, and error per operation.\n  \
          Use --json-pretty for indented output.\n\n\
          Not allowed inside batch: nested batch, completions, man, version, config set/unset.\n\n\
          Examples:\n  \
          echo '[{\"args\":[\"archive\",\"path\"]}]' | scout batch\n  \
          scout batch --file plan.json\n  \
          scout batch --file - < plan.json\n  \
          scout batch --fail-fast --file plan.json",
        next_help_heading = "Utilities"
    )]
    Batch {
        /// JSON plan file (`-` reads stdin; piped stdin is used when --file is omitted)
        #[arg(long, value_name = "FILE")]
        file: Option<String>,

        /// Stop after the first failed operation
        #[arg(long)]
        fail_fast: bool,
    },
}

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
    /// Fetch from ScoutAPM and store idempotently
    Pull {
        #[command(flatten)]
        app: AppIdArgs,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        range: Option<String>,
        #[arg(long = "resource")]
        resource: Vec<String>,
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
            help = "Max endpoints to scan when pulling traces"
        )]
        trace_endpoint_limit: u32,
        #[arg(short = 'n', long, help = "Preview pull plan without calling the API")]
        dry_run: bool,
    },
    /// Fetch and store one trace by ID (idempotent)
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

#[derive(Clone, Args)]
pub struct ConfigOptions {
    /// Preview config changes without writing files.
    #[arg(short = 'n', long)]
    pub dry_run: bool,
}

#[derive(Clone, Subcommand)]
pub enum ConfigCommands {
    /// List config keys and effective values
    List,
    /// Print one config value
    Get { key: String },
    /// Set a config value in config.env
    Set { key: String, value: String },
    /// Remove a config value from config.env
    Unset { key: String },
    /// Print config directory and file paths
    Path,
}
