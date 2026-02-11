//! Helpers for API key resolution and ScoutAPM URL parsing.

use base64::Engine;
use chrono::{DateTime, Local, Utc};
use url::Url;

/// Source from which the API key was obtained (for diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeySource {
    OnePassword,
    Bitwarden,
    Keepassxc,
}

/// Get API key from a secret backend only (1Password, Bitwarden, KeePassXC).
///
/// Plain-text API keys (env vars or CLI) are not supported for security reasons.
/// Configure one backend via its env vars (see [secret] module):
/// - 1Password: `SCOUT_OP_ENTRY_PATH` (op://Vault/Item) or `SCOUT_OP_VAULT` + `SCOUT_OP_ITEM`; optional `SCOUT_OP_FIELD` (default API_KEY).
/// - Bitwarden: `SCOUT_BW_ITEM_ID` (login item UUID); optional `SCOUT_BW_SESSION`.
/// - KeePassXC: `SCOUT_KPXC_DB`, `SCOUT_KPXC_ENTRY`; optional `SCOUT_KPXC_ATTRIBUTE` (default Password).
pub fn get_api_key() -> Result<(String, ApiKeySource), String> {
    if let Some(k) = crate::secret::one_password() {
        if !k.is_empty() {
            return Ok((k, ApiKeySource::OnePassword));
        }
    }
    if let Some(k) = crate::secret::bitwarden() {
        if !k.is_empty() {
            return Ok((k, ApiKeySource::Bitwarden));
        }
    }
    if let Some(k) = crate::secret::keepassxc() {
        if !k.is_empty() {
            return Ok((k, ApiKeySource::Keepassxc));
        }
    }
    Err(
        "API key not found. Configure a secret backend: SCOUT_OP_ENTRY_PATH (1Password), \
         SCOUT_BW_ITEM_ID (Bitwarden), or SCOUT_KPXC_DB+SCOUT_KPXC_ENTRY (KeePassXC). Plain-text keys are not supported."
            .to_string(),
    )
}

/// Parsed ScoutAPM URL resource type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoutUrlType {
    App,
    Endpoint,
    Trace,
    ErrorGroup,
    Insight,
    Unknown,
}

/// Result of parsing a ScoutAPM URL.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ParsedScoutUrl {
    pub url_type: ScoutUrlType,
    pub app_id: Option<u64>,
    pub endpoint_id: Option<String>,
    pub trace_id: Option<u64>,
    pub error_id: Option<u64>,
    pub insight_type: Option<String>,
    pub decoded_endpoint: Option<String>,
}

/// Parse a ScoutAPM URL and extract resource identifiers.
pub fn parse_scout_url(url: &str) -> Result<ParsedScoutUrl, String> {
    let parsed = Url::parse(url).map_err(|e| e.to_string())?;
    let path = parsed.path();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    let app_index = segments.iter().position(|s| *s == "apps");
    let app_id = app_index.and_then(|i| segments.get(i + 1).and_then(|s| s.parse::<u64>().ok()));

    let url_type = if segments.contains(&"trace") {
        ScoutUrlType::Trace
    } else if segments.contains(&"endpoints") {
        ScoutUrlType::Endpoint
    } else if segments.contains(&"error_groups") {
        ScoutUrlType::ErrorGroup
    } else if segments.contains(&"insights") {
        ScoutUrlType::Insight
    } else if app_index.is_some() && segments.len() >= 2 && segments[0] == "apps" {
        ScoutUrlType::App
    } else {
        ScoutUrlType::Unknown
    };

    let endpoint_id = segments
        .iter()
        .position(|s| *s == "endpoints")
        .and_then(|i| segments.get(i + 1).map(|s| (*s).to_string()));
    let trace_id = segments
        .iter()
        .position(|s| *s == "trace")
        .and_then(|i| segments.get(i + 1).and_then(|s| s.parse::<u64>().ok()));
    let error_id = segments
        .iter()
        .position(|s| *s == "error_groups")
        .and_then(|i| segments.get(i + 1).and_then(|s| s.parse::<u64>().ok()));
    let insight_type = segments
        .iter()
        .position(|s| *s == "insights")
        .and_then(|i| segments.get(i + 1).map(|s| (*s).to_string()));

    let decoded_endpoint = endpoint_id
        .as_ref()
        .and_then(|id| decode_endpoint_id(id).ok());

    Ok(ParsedScoutUrl {
        url_type,
        app_id,
        endpoint_id,
        trace_id,
        error_id,
        insight_type,
        decoded_endpoint,
    })
}

/// Decode base64url endpoint ID to a readable string when possible.
pub fn decode_endpoint_id(endpoint_id: &str) -> Result<String, String> {
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(endpoint_id.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(endpoint_id.as_bytes()))
        .map_err(|e| e.to_string())?;
    String::from_utf8(decoded).map_err(|e| e.to_string())
}

/// Format time as ISO 8601 for the API.
pub fn format_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Format an ISO 8601 timestamp for display. If `use_utc` is true, shows UTC; otherwise converts to local timezone.
/// On parse failure returns the original string unchanged.
pub fn format_timestamp_display(ts: &str, use_utc: bool) -> String {
    let dt = match parse_time(ts) {
        Ok(d) => d,
        _ => return ts.to_string(),
    };
    if use_utc {
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    } else {
        dt.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S %:z")
            .to_string()
    }
}

/// Parse ISO 8601 time string.
pub fn parse_time(s: &str) -> Result<DateTime<Utc>, String> {
    let s = s.trim().trim_end_matches('Z').trim_end_matches('z');
    let parsed = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", s))
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
        .map_err(|e| e.to_string())?;
    Ok(parsed.with_timezone(&Utc))
}

/// Parse range string (e.g. "30min", "1day", "7days") into seconds.
pub fn parse_range(range_str: &str) -> Result<u64, String> {
    let s = range_str.trim().to_lowercase();
    let s = s.replace(" ", "");
    let mut num_end = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            num_end += 1;
        } else {
            break;
        }
    }
    let num: u64 = s[..num_end]
        .parse()
        .map_err(|_| format!("Invalid range: {}", range_str))?;
    let unit = s[num_end..].trim();
    let secs = match unit {
        u if u.starts_with("min") => num * 60,
        u if u.starts_with("hr") || u.starts_with("hour") => num * 3600,
        u if u.starts_with("day") => num * 86400,
        _ => return Err(format!("Unknown time unit in range: {}", range_str)),
    };
    Ok(secs)
}

/// Compute (from, to) ISO 8601 strings for a range ending at `to` (or now).
pub fn calculate_range(range: &str, to: Option<&str>) -> Result<(String, String), String> {
    let end_time = match to {
        Some(t) => parse_time(t)?,
        None => Utc::now(),
    };
    let secs = parse_range(range)?;
    let start_time = end_time - chrono::Duration::seconds(secs as i64);
    Ok((format_time(start_time), format_time(end_time)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("30min").unwrap(), 30 * 60);
        assert_eq!(parse_range("1day").unwrap(), 86400);
        assert_eq!(parse_range("7days").unwrap(), 7 * 86400);
    }

    #[test]
    fn test_parse_scout_url_trace() {
        let u = "https://scoutapm.com/apps/123/endpoints/abc/trace/456";
        let p = parse_scout_url(u).unwrap();
        assert_eq!(p.url_type, ScoutUrlType::Trace);
        assert_eq!(p.app_id, Some(123));
        assert_eq!(p.trace_id, Some(456));
    }
}
