//! Resolve Scout resource identifiers for jobs and anomaly endpoints.

use crate::helpers::decode_endpoint_id;
use base64::Engine;

/// Encode `queue/JobName` or accept an already-encoded job id.
///
/// Matches Ruby `Base64.urlsafe_encode64` (URL-safe alphabet with padding).
pub fn resolve_job_id(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "invalid job \"\" — expected a job id from 'scout jobs --json' (job_id), or a full name like default/MyWorker"
                .to_string(),
        );
    }
    if trimmed.contains('/') {
        return Ok(base64::engine::general_purpose::URL_SAFE.encode(trimmed.as_bytes()));
    }
    match decode_endpoint_id(trimmed) {
        Ok(decoded) if decoded.contains('/') => Ok(trimmed.to_string()),
        _ => Err(format!(
            "invalid job {trimmed:?} — expected a job id from 'scout jobs --json' (job_id), or a full name like default/MyWorker"
        )),
    }
}

/// Human-facing job label for titles and URLs.
pub fn job_display_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains('/') {
        return trimmed.to_string();
    }
    decode_endpoint_id(trimmed).unwrap_or_else(|_| trimmed.to_string())
}

/// Resolve anomaly `--endpoint` to the scoped metric name the API matches.
///
/// Accepts `Controller/users/index`, a plain name (`users/index` → `Controller/…`),
/// or a Base64 endpoint id from `scout endpoints`.
pub fn resolve_anomaly_endpoint(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "invalid --endpoint \"\" — expected a name from anomaly events (e.g. Controller/users/index) or a Base64 endpoint id"
                .to_string(),
        );
    }
    let name = if trimmed.contains('/') {
        trimmed.to_string()
    } else {
        decode_endpoint_id(trimmed).map_err(|_| {
            format!(
                "invalid --endpoint {trimmed:?} — expected a name from anomaly events (e.g. Controller/users/index) or a Base64 endpoint id"
            )
        })?
    };
    if has_metric_scope(&name) {
        Ok(name)
    } else {
        Ok(format!("Controller/{name}"))
    }
}

fn has_metric_scope(name: &str) -> bool {
    name.starts_with("Controller/")
        || name.starts_with("Job/")
        || name.starts_with("Middleware/")
        || name.starts_with("Custom/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_queue_job_name() {
        let id = resolve_job_id("default/MyWorker").unwrap();
        assert_eq!(
            id,
            base64::engine::general_purpose::URL_SAFE.encode(b"default/MyWorker")
        );
    }

    #[test]
    fn accept_encoded_job_id() {
        let encoded = base64::engine::general_purpose::URL_SAFE.encode(b"default/MyWorker");
        assert_eq!(resolve_job_id(&encoded).unwrap(), encoded);
    }

    #[test]
    fn reject_bare_class_name() {
        let err = resolve_job_id("MyWorker").unwrap_err();
        assert!(err.contains("full name"));
    }

    #[test]
    fn reject_blank_job() {
        assert!(resolve_job_id("  ").is_err());
    }

    #[test]
    fn anomaly_scoped_name_passthrough() {
        assert_eq!(
            resolve_anomaly_endpoint("Controller/users/index").unwrap(),
            "Controller/users/index"
        );
    }

    #[test]
    fn anomaly_plain_name_gets_controller_prefix() {
        assert_eq!(
            resolve_anomaly_endpoint("users/index").unwrap(),
            "Controller/users/index"
        );
    }

    #[test]
    fn anomaly_base64_endpoint_id() {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"users/index");
        assert_eq!(
            resolve_anomaly_endpoint(&encoded).unwrap(),
            "Controller/users/index"
        );
    }

    #[test]
    fn anomaly_rejects_blank() {
        assert!(resolve_anomaly_endpoint("").is_err());
    }
}
