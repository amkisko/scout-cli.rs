//! Organization usage and billing period from `/api/v0/usage`.

use crate::client::Client;
use crate::error::Error;
use serde_json::Value;

impl Client {
    /// Fetch organization usage for the current billing period.
    pub async fn get_org_usage(&self) -> Result<Value, Error> {
        let url = format!("{}/usage", self.api_base());
        let res: Value = self.send(self.auth(self.http.get(&url))).await?;
        Ok(res.get("results").cloned().unwrap_or(Value::Null))
    }

    /// Fetch one app and fill `last_reported_at` from the list payload when missing.
    pub async fn get_app_enriched(&self, app_id: u64) -> Result<Value, Error> {
        let mut app = self.get_app(app_id).await?;
        let missing = app
            .get("last_reported_at")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if !missing {
            return Ok(app);
        }
        if let Some(listed) = self
            .list_apps(None)
            .await?
            .into_iter()
            .find(|row| row.get("id").and_then(|v| v.as_u64()) == Some(app_id))
        {
            if let Some(last) = listed.get("last_reported_at").cloned() {
                if let Some(object) = app.as_object_mut() {
                    object.insert("last_reported_at".to_string(), last);
                }
            }
        }
        Ok(app)
    }
}

/// Rename latency job-metric series `total` → `execution_time_total` when both exist.
pub fn normalize_job_latency_series(metric_type: &str, mut data: Value) -> Value {
    if metric_type != "latency" {
        return data;
    }
    let Some(series) = data.get_mut("series").and_then(|s| s.as_object_mut()) else {
        // Some responses nest under results; also try top-level object of named series.
        return rename_total_in_object(&mut data);
    };
    if series.contains_key("Latency") && series.contains_key("total") {
        if let Some(total) = series.remove("total") {
            series.insert("execution_time_total".to_string(), total);
        }
    }
    data
}

fn rename_total_in_object(data: &mut Value) -> Value {
    if let Some(object) = data.as_object_mut() {
        if object.contains_key("Latency") && object.contains_key("total") {
            if let Some(total) = object.remove("total") {
                object.insert("execution_time_total".to_string(), total);
            }
        }
    }
    data.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renames_latency_total_series() {
        let data = json!({
            "series": {
                "Latency": [["2026-01-01T00:00:00Z", 1.0]],
                "total": [["2026-01-01T00:00:00Z", 2.0]]
            }
        });
        let out = normalize_job_latency_series("latency", data);
        assert!(out["series"].get("execution_time_total").is_some());
        assert!(out["series"].get("total").is_none());
    }

    #[test]
    fn leaves_other_metrics_alone() {
        let data = json!({"series": {"total": [[ "t", 1.0 ]]}});
        let out = normalize_job_latency_series("execution_time", data.clone());
        assert_eq!(out, data);
    }
}
