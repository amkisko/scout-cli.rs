//! ScoutAPM API client library.
//!
//! Provides a typed client for the ScoutAPM REST API: apps, metrics, endpoints,
//! jobs, traces, errors, insights, and anomaly events.

pub mod app_resolve;
pub mod archive;
pub mod client;
pub mod config;
pub mod error;
pub mod helpers;
pub mod secret;

pub use app_resolve::{app_ref_needs_list, resolve_app_id_from_list, resolve_app_target};
pub use archive::{
    archive_home, diff_endpoints, diff_errors, diff_jobs, diff_metric_buckets, export_archive,
    format_pull_summary, plan_pull, pull_app, pull_app_with_progress, pull_trace_by_id,
    ArchiveLayout, ArchiveStore, DiffChange, DiffReport, DiffResource, DiffSide, ExportFormat,
    ExportReport, ExportRequest, ExportResource, MetricBucket, PullOptions, PullPlan, PullReport,
    PullResource, RangeSnapshotFile, StoreAction, MANIFEST_VERSION,
};
pub use client::Client;
pub use config::{
    config_file_path, ensure_home_config_loaded, friendly_config_key, get_config_entry,
    list_config_entries, load_home_config, resolve_config_key, scout_home, set_config_entry,
    unset_config_entry, ConfigEntry, ConfigSource,
};
pub use error::{ApiError, AuthError, Error};
pub use helpers::{
    build_app_url, build_endpoint_trace_url, build_endpoint_url, build_error_group_url,
    build_job_trace_url, build_job_url, build_scout_url, build_trace_url, format_timestamp_display,
    get_api_key, parse_scout_url, scout_web_origin, ApiKeySource, ParsedScoutUrl, ScoutUrlType,
};
pub use secret::{
    bitwarden, bitwarden_attempt, bitwarden_configured, keepassxc, keepassxc_attempt,
    keepassxc_configured, one_password, one_password_attempt, one_password_configured,
    BackendAttempt,
};

/// Library version for User-Agent and diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
