//! Archive pull resource names and ScoutAPM window caps.

use serde::{Deserialize, Serialize};

pub const REQUEST_SPAN_SECS: i64 = 14 * 24 * 3600;
pub const LOOKBACK_SLACK_SECS: i64 = 3600;
pub const MAX_PULL_SECS: i64 = 30 * 24 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullResource {
    App,
    Metrics,
    Endpoints,
    Jobs,
    Errors,
    Anomalies,
    Insights,
    EndpointMetrics,
    JobMetrics,
    Traces,
}

pub const DEFAULT_METRICS: [&str; 6] = [
    "apdex",
    "response_time",
    "response_time_95th",
    "errors",
    "throughput",
    "queue_time",
];

pub struct ResourceWindowPolicy {
    pub max_lookback_secs: Option<i64>,
    pub max_span_secs: i64,
}

#[derive(Debug, Clone)]
pub struct PullOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub range: Option<String>,
    pub resources: Vec<PullResource>,
    pub metrics: Vec<String>,
    pub trace_ids: Vec<u64>,
    pub trace_endpoint_limit: u32,
    pub force: bool,
    pub incremental: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRefusal {
    pub resource: String,
    pub from: String,
    pub to: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullReport {
    pub app_id: u64,
    pub from: String,
    pub to: String,
    pub chunks: u64,
    pub created: u64,
    pub skipped: u64,
    pub metric_points_added: u64,
    pub metric_points_skipped: u64,
    pub traces_created: u64,
    pub traces_skipped: u64,
    pub resources: Vec<String>,
    #[serde(default)]
    pub refusals: Vec<PullRefusal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullPlan {
    pub app_id: u64,
    pub archive_home: String,
    pub from: String,
    pub to: String,
    pub chunk_count: u64,
    pub resources: Vec<String>,
    pub metrics: Vec<String>,
    pub trace_ids: Vec<u64>,
    pub incremental: bool,
    pub force: bool,
    pub dry_run: bool,
}

impl PullResource {
    pub fn as_str(self) -> &'static str {
        match self {
            PullResource::App => "app",
            PullResource::Metrics => "metrics",
            PullResource::Endpoints => "endpoints",
            PullResource::Jobs => "jobs",
            PullResource::Errors => "errors",
            PullResource::Anomalies => "anomalies",
            PullResource::Insights => "insights",
            PullResource::EndpointMetrics => "endpoint_metrics",
            PullResource::JobMetrics => "job_metrics",
            PullResource::Traces => "traces",
        }
    }

    pub fn window_policy(self) -> ResourceWindowPolicy {
        match self {
            PullResource::App => ResourceWindowPolicy {
                max_lookback_secs: None,
                max_span_secs: REQUEST_SPAN_SECS,
            },
            PullResource::Metrics
            | PullResource::Endpoints
            | PullResource::Jobs
            | PullResource::EndpointMetrics
            | PullResource::JobMetrics
            | PullResource::Insights => ResourceWindowPolicy {
                max_lookback_secs: None,
                max_span_secs: REQUEST_SPAN_SECS,
            },
            PullResource::Errors | PullResource::Anomalies => ResourceWindowPolicy {
                max_lookback_secs: Some(MAX_PULL_SECS - LOOKBACK_SLACK_SECS),
                max_span_secs: REQUEST_SPAN_SECS,
            },
            PullResource::Traces => ResourceWindowPolicy {
                max_lookback_secs: Some(7 * 24 * 3600 - LOOKBACK_SLACK_SECS),
                max_span_secs: 7 * 24 * 3600 - LOOKBACK_SLACK_SECS,
            },
        }
    }

    pub fn parse_list(values: &[String]) -> Result<Vec<PullResource>, String> {
        if values.is_empty() {
            return Ok(vec![
                PullResource::App,
                PullResource::Metrics,
                PullResource::Endpoints,
                PullResource::Jobs,
                PullResource::EndpointMetrics,
                PullResource::JobMetrics,
                PullResource::Errors,
                PullResource::Anomalies,
                PullResource::Insights,
            ]);
        }
        let mut resources = Vec::new();
        for value in values {
            let resource = match value.as_str() {
                "app" => PullResource::App,
                "metrics" => PullResource::Metrics,
                "endpoints" => PullResource::Endpoints,
                "jobs" => PullResource::Jobs,
                "errors" => PullResource::Errors,
                "anomalies" => PullResource::Anomalies,
                "insights" => PullResource::Insights,
                "endpoint_metrics" | "endpoint-metrics" => PullResource::EndpointMetrics,
                "job_metrics" | "job-metrics" => PullResource::JobMetrics,
                "traces" => PullResource::Traces,
                other => {
                    return Err(format!(
                        "unknown archive resource '{other}'. Expected: app, metrics, endpoints, jobs, errors, anomalies, insights, endpoint_metrics, job_metrics, traces"
                    ));
                }
            };
            if !resources.contains(&resource) {
                resources.push(resource);
            }
        }
        Ok(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn pull_resources_default_list_omits_traces() {
        let resources = PullResource::parse_list(&[]).unwrap();
        let expected: HashSet<PullResource> = [
            PullResource::App,
            PullResource::Metrics,
            PullResource::Endpoints,
            PullResource::Jobs,
            PullResource::EndpointMetrics,
            PullResource::JobMetrics,
            PullResource::Errors,
            PullResource::Anomalies,
            PullResource::Insights,
        ]
        .into_iter()
        .collect();
        assert_eq!(resources.into_iter().collect::<HashSet<_>>(), expected);
        assert!(!expected.contains(&PullResource::Traces));
    }

    #[test]
    fn parse_list_accepts_kebab_series_names() {
        let resources =
            PullResource::parse_list(&["endpoint-metrics".to_string(), "job_metrics".to_string()])
                .unwrap();
        assert_eq!(
            resources,
            vec![PullResource::EndpointMetrics, PullResource::JobMetrics]
        );
    }
}
