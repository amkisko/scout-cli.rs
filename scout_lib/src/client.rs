//! HTTP client for ScoutAPM REST API.

use crate::error::{ApiError, AuthError, Error};
use crate::helpers::{calculate_range, format_time, parse_time};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::time::Duration;

const API_BASE: &str = "https://scoutapm.com/api/v0";
const VALID_METRICS: [&str; 6] = [
    "apdex",
    "response_time",
    "response_time_95th",
    "errors",
    "throughput",
    "queue_time",
];
const VALID_INSIGHTS: [&str; 3] = ["n_plus_one", "memory_bloat", "slow_query"];
const MAX_RANGE_SECS: i64 = 14 * 24 * 3600; // 14 days

/// ScoutAPM API client.
#[derive(Clone)]
pub struct Client {
    api_key: String,
    api_base: String,
    user_agent: String,
    http: HttpClient,
}

impl Client {
    /// Create a new client with the given API key.
    pub fn new(api_key: String) -> Self {
        let user_agent = format!("scout-cli/{}", crate::VERSION);
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client");
        Self {
            api_key,
            api_base: API_BASE.to_string(),
            user_agent,
            http,
        }
    }

    /// List applications accessible with the API key.
    pub async fn list_apps(&self, active_since: Option<&str>) -> Result<Vec<Value>, Error> {
        let url = format!("{}/apps", self.api_base);
        let mut req = self.http.get(&url);
        req = self.auth(req);
        let res: Value = self.send(req).await?;
        let apps: Vec<Value> = res
            .get("results")
            .and_then(|r| r.get("apps"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        if let Some(since) = active_since {
            let since_t = parse_time(since).map_err(Error::Other)?;
            let filtered: Vec<Value> = apps
                .into_iter()
                .filter(|app| {
                    app.get("last_reported_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| parse_time(s).ok())
                        .map(|t| t >= since_t)
                        .unwrap_or(false)
                })
                .collect();
            return Ok(filtered);
        }
        Ok(apps)
    }

    /// Get a single application by ID.
    pub async fn get_app(&self, app_id: u64) -> Result<Value, Error> {
        let url = format!("{}/apps/{}", self.api_base, app_id);
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res
            .get("results")
            .and_then(|r| r.get("app"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// List available metric types for an app.
    pub async fn list_metrics(&self, app_id: u64) -> Result<Vec<String>, Error> {
        let url = format!("{}/apps/{}/metrics", self.api_base, app_id);
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        let arr = res
            .get("results")
            .and_then(|r| r.get("availableMetrics"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let list: Vec<String> = arr
            .into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        Ok(list)
    }

    /// Get time-series metric data.
    pub async fn get_metric(
        &self,
        app_id: u64,
        metric_type: &str,
        from: Option<&str>,
        to: Option<&str>,
        range: Option<&str>,
    ) -> Result<Value, Error> {
        if !VALID_METRICS.contains(&metric_type) {
            return Err(Error::Other(format!(
                "Invalid metric_type. Must be one of: {}",
                VALID_METRICS.join(", ")
            )));
        }
        let (from, to) = if let Some(r) = range {
            let (f, t) = calculate_range(r, to).map_err(Error::Other)?;
            (Some(f), Some(t))
        } else {
            (from.map(String::from), to.map(String::from))
        };
        if let (Some(ref f), Some(ref t)) = (&from, &to) {
            validate_time_range(f, t)?;
        }
        let mut url = format!("{}/apps/{}/metrics/{}", self.api_base, app_id, metric_type);
        if from.is_some() || to.is_some() {
            let mut params = vec![];
            if let Some(ref f) = from {
                params.push(format!("from={}", urlencoding::encode(f)));
            }
            if let Some(ref t) = to {
                params.push(format!("to={}", urlencoding::encode(t)));
            }
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res
            .get("results")
            .and_then(|r| r.get("series"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// List endpoints for an app.
    pub async fn list_endpoints(
        &self,
        app_id: u64,
        from: Option<&str>,
        to: Option<&str>,
        range: Option<&str>,
    ) -> Result<Value, Error> {
        let (from_str, to_str) = if let Some(r) = range {
            calculate_range(r, to).map_err(Error::Other)?
        } else if from.is_none() && to.is_none() {
            calculate_range("7days", None).map_err(Error::Other)?
        } else {
            let to_s = to
                .map(String::from)
                .unwrap_or_else(|| format_time(Utc::now()));
            let from_s = from.map(String::from).unwrap_or_else(|| {
                let (f, _) = calculate_range("7days", Some(&to_s)).unwrap();
                f
            });
            (from_s, to_s)
        };
        validate_time_range(&from_str, &to_str)?;
        let url = format!(
            "{}/apps/{}/endpoints?from={}&to={}",
            self.api_base,
            app_id,
            urlencoding::encode(&from_str),
            urlencoding::encode(&to_str)
        );
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Get metric data for a specific endpoint.
    pub async fn get_endpoint_metrics(
        &self,
        app_id: u64,
        endpoint_id: &str,
        metric_type: &str,
        from: Option<&str>,
        to: Option<&str>,
        range: Option<&str>,
    ) -> Result<Value, Error> {
        if !VALID_METRICS.contains(&metric_type) {
            return Err(Error::Other(format!(
                "Invalid metric_type. Must be one of: {}",
                VALID_METRICS.join(", ")
            )));
        }
        let (from, to) = if let Some(r) = range {
            let (f, t) = calculate_range(r, to).map_err(Error::Other)?;
            (Some(f), Some(t))
        } else {
            (from.map(String::from), to.map(String::from))
        };
        if let (Some(ref f), Some(ref t)) = (&from, &to) {
            validate_time_range(f, t)?;
        }
        let mut url = format!(
            "{}/apps/{}/endpoints/{}/metrics/{}",
            self.api_base, app_id, endpoint_id, metric_type
        );
        if from.is_some() || to.is_some() {
            let mut params = vec![];
            if let Some(ref f) = from {
                params.push(format!("from={}", urlencoding::encode(f)));
            }
            if let Some(ref t) = to {
                params.push(format!("to={}", urlencoding::encode(t)));
            }
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res
            .get("results")
            .and_then(|r| r.get("series"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// List traces for a specific endpoint (max 100, within 7 days).
    pub async fn list_endpoint_traces(
        &self,
        app_id: u64,
        endpoint_id: &str,
        from: Option<&str>,
        to: Option<&str>,
        range: Option<&str>,
    ) -> Result<Value, Error> {
        let (from_str, to_str) = if let Some(r) = range {
            calculate_range(r, to).map_err(Error::Other)?
        } else if from.is_none() && to.is_none() {
            calculate_range("7days", None).map_err(Error::Other)?
        } else {
            let to_s = to
                .map(String::from)
                .unwrap_or_else(|| format_time(Utc::now()));
            let from_s = from.map(String::from).unwrap_or_else(|| {
                let (f, _) = calculate_range("7days", Some(&to_s)).unwrap();
                f
            });
            (from_s, to_s)
        };
        validate_time_range(&from_str, &to_str)?;
        let url = format!(
            "{}/apps/{}/endpoints/{}/traces?from={}&to={}",
            self.api_base,
            app_id,
            endpoint_id,
            urlencoding::encode(&from_str),
            urlencoding::encode(&to_str)
        );
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Fetch a single trace by app and trace ID.
    pub async fn fetch_trace(&self, app_id: u64, trace_id: u64) -> Result<Value, Error> {
        let url = format!("{}/apps/{}/traces/{}", self.api_base, app_id, trace_id);
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res
            .get("results")
            .and_then(|r| r.get("trace"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// List error groups for an app.
    pub async fn list_error_groups(
        &self,
        app_id: u64,
        from: Option<&str>,
        to: Option<&str>,
        endpoint: Option<&str>,
    ) -> Result<Vec<Value>, Error> {
        if let (Some(f), Some(t)) = (from, to) {
            validate_time_range(f, t)?;
        }
        let mut url = format!("{}/apps/{}/error_groups", self.api_base, app_id);
        let mut params = vec![];
        if let Some(f) = from {
            params.push(format!("from={}", urlencoding::encode(f)));
        }
        if let Some(t) = to {
            params.push(format!("to={}", urlencoding::encode(t)));
        }
        if let Some(e) = endpoint {
            params.push(format!("endpoint={}", urlencoding::encode(e)));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        let list = res
            .get("results")
            .and_then(|r| r.get("error_groups"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(list)
    }

    /// Get a single error group.
    pub async fn get_error_group(&self, app_id: u64, error_id: u64) -> Result<Value, Error> {
        let url = format!(
            "{}/apps/{}/error_groups/{}",
            self.api_base, app_id, error_id
        );
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res
            .get("results")
            .and_then(|r| r.get("error_group"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Get individual errors within an error group (max 100).
    pub async fn get_error_group_errors(
        &self,
        app_id: u64,
        error_id: u64,
    ) -> Result<Vec<Value>, Error> {
        let url = format!(
            "{}/apps/{}/error_groups/{}/errors",
            self.api_base, app_id, error_id
        );
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        let list = res
            .get("results")
            .and_then(|r| r.get("errors"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(list)
    }

    /// Get all insights for an app.
    pub async fn get_all_insights(&self, app_id: u64, limit: Option<u32>) -> Result<Value, Error> {
        let mut url = format!("{}/apps/{}/insights", self.api_base, app_id);
        if let Some(l) = limit {
            url.push_str(&format!("?limit={}", l));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Get insight by type.
    pub async fn get_insight_by_type(
        &self,
        app_id: u64,
        insight_type: &str,
        limit: Option<u32>,
    ) -> Result<Value, Error> {
        if !VALID_INSIGHTS.contains(&insight_type) {
            return Err(Error::Other(format!(
                "Invalid insight_type. Must be one of: {}",
                VALID_INSIGHTS.join(", ")
            )));
        }
        let mut url = format!(
            "{}/apps/{}/insights/{}",
            self.api_base, app_id, insight_type
        );
        if let Some(l) = limit {
            url.push_str(&format!("?limit={}", l));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Get historical insights with cursor-based pagination.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_insights_history(
        &self,
        app_id: u64,
        from: Option<&str>,
        to: Option<&str>,
        limit: Option<u32>,
        pagination_cursor: Option<u64>,
        pagination_direction: Option<&str>,
        pagination_page: Option<u32>,
    ) -> Result<Value, Error> {
        let mut url = format!("{}/apps/{}/insights/history", self.api_base, app_id);
        let mut params = vec![];
        if let Some(f) = from {
            params.push(format!("from={}", urlencoding::encode(f)));
        }
        if let Some(t) = to {
            params.push(format!("to={}", urlencoding::encode(t)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(c) = pagination_cursor {
            params.push(format!("pagination_cursor={}", c));
        }
        if let Some(d) = pagination_direction {
            params.push(format!("pagination_direction={}", urlencoding::encode(d)));
        }
        if let Some(p) = pagination_page {
            params.push(format!("pagination_page={}", p));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Get historical insights by type with cursor-based pagination.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_insights_history_by_type(
        &self,
        app_id: u64,
        insight_type: &str,
        from: Option<&str>,
        to: Option<&str>,
        limit: Option<u32>,
        pagination_cursor: Option<u64>,
        pagination_direction: Option<&str>,
        pagination_page: Option<u32>,
    ) -> Result<Value, Error> {
        if !VALID_INSIGHTS.contains(&insight_type) {
            return Err(Error::Other(format!(
                "Invalid insight_type. Must be one of: {}",
                VALID_INSIGHTS.join(", ")
            )));
        }
        let mut url = format!(
            "{}/apps/{}/insights/history/{}",
            self.api_base, app_id, insight_type
        );
        let mut params = vec![];
        if let Some(f) = from {
            params.push(format!("from={}", urlencoding::encode(f)));
        }
        if let Some(t) = to {
            params.push(format!("to={}", urlencoding::encode(t)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(c) = pagination_cursor {
            params.push(format!("pagination_cursor={}", c));
        }
        if let Some(d) = pagination_direction {
            params.push(format!("pagination_direction={}", urlencoding::encode(d)));
        }
        if let Some(p) = pagination_page {
            params.push(format!("pagination_page={}", p));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-SCOUT-API",
            HeaderValue::from_str(&self.api_key).expect("api key header"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent).expect("user agent"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        req.headers(headers)
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<Value, Error> {
        let res = req.send().await.map_err(|e| Error::Other(e.to_string()))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| Error::Other(e.to_string()))?;
        let data: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        if status.is_client_error() && status.as_u16() == 401 {
            return Err(Error::Auth(AuthError {
                message: "Authentication failed. Check your API key.".to_string(),
            }));
        }
        if !status.is_success() {
            let msg = data
                .get("header")
                .and_then(|h| h.get("status"))
                .and_then(|s| s.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("API request failed");
            return Err(Error::Api(ApiError::new(
                msg,
                Some(status.as_u16()),
                Some(data.clone()),
            )));
        }
        if let Some(code) = data
            .get("header")
            .and_then(|h| h.get("status"))
            .and_then(|s| s.get("code"))
            .and_then(|c| c.as_u64())
        {
            if code >= 400 {
                let msg = data
                    .get("header")
                    .and_then(|h| h.get("status"))
                    .and_then(|s| s.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown API error");
                return Err(Error::Api(ApiError::new(
                    msg,
                    Some(code as u16),
                    Some(data.clone()),
                )));
            }
        }
        Ok(data)
    }
}

fn validate_time_range(from: &str, to: &str) -> Result<(), Error> {
    let from_t = parse_time(from).map_err(Error::Other)?;
    let to_t = parse_time(to).map_err(Error::Other)?;
    if from_t >= to_t {
        return Err(Error::Other("from_time must be before to_time".to_string()));
    }
    if (to_t - from_t).num_seconds() > MAX_RANGE_SECS {
        return Err(Error::Other("Time range cannot exceed 2 weeks".to_string()));
    }
    Ok(())
}
