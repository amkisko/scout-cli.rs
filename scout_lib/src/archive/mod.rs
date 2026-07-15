//! Local archive for ScoutAPM data: idempotent storage, scheduled pulls, and diff.

mod diff;
mod export;
mod layout;
mod metrics;
mod pull;
mod store;

pub use metrics::MetricBucket;

pub use diff::{
    diff_endpoints, diff_errors, diff_jobs, diff_metric_buckets, DiffChange, DiffReport,
    DiffResource, DiffSide,
};
pub use export::{export_archive, ExportFormat, ExportReport, ExportRequest, ExportResource};
pub use layout::{archive_home, range_key, ArchiveLayout, RangeSnapshotMeta, MANIFEST_VERSION};
pub use pull::{
    format_pull_summary, plan_pull, pull_app, pull_app_with_progress, pull_trace_by_id,
    PullOptions, PullPlan, PullReport, PullResource, DEFAULT_METRICS,
};
pub use store::{ArchiveStore, MetricMergeReport, RangeSnapshotFile, StoreAction};
