//! Resolve ScoutAPM application id from id, name, or config defaults.

use serde_json::Value;

/// Match an application id from a `list_apps` payload.
///
/// Numeric refs match `id`. Non-numeric refs match exact `name` (case-insensitive).
/// Zero or multiple name matches return an error string.
pub fn resolve_app_id_from_list(apps: &[Value], app_ref: &str) -> Result<u64, String> {
    let app_ref = app_ref.trim();
    if app_ref.is_empty() {
        return Err("provide APP (id or name), --app-id, SCOUT_APP_ID, or SCOUT_APP".to_string());
    }

    if let Ok(id) = app_ref.parse::<u64>() {
        if apps
            .iter()
            .any(|app| app.get("id").and_then(|value| value.as_u64()) == Some(id))
        {
            return Ok(id);
        }
        // Allow ids that are not in the current list (API may still accept them).
        return Ok(id);
    }

    let lower = app_ref.to_lowercase();
    let matches: Vec<u64> = apps
        .iter()
        .filter_map(|app| {
            let name = app.get("name").and_then(|value| value.as_str())?;
            if name.to_lowercase() == lower {
                app.get("id").and_then(|value| value.as_u64())
            } else {
                None
            }
        })
        .collect();

    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(format!("app not found: {app_ref}")),
        many => Err(format!(
            "app name {app_ref} matches {} applications; use a numeric id instead",
            many.len()
        )),
    }
}

/// Resolve the effective app id from positional APP, --app-id, and env defaults.
///
/// `apps` is required when `app_ref` is a non-numeric name. Pass `None` only when
/// the ref is numeric or when only `flag_id` is set.
pub fn resolve_app_target(
    app_ref: Option<&str>,
    flag_id: Option<u64>,
    apps: Option<&[Value]>,
) -> Result<u64, String> {
    let trimmed = app_ref.map(str::trim).filter(|value| !value.is_empty());

    match (trimmed, flag_id) {
        (Some(positional), Some(flag)) => {
            if let Ok(positional_id) = positional.parse::<u64>() {
                if positional_id != flag {
                    return Err("provide only one of APP or --app-id".to_string());
                }
                return Ok(flag);
            }
            Err("provide only one of APP or --app-id".to_string())
        }
        (None, Some(flag)) => Ok(flag),
        (Some(positional), None) => {
            if let Ok(id) = positional.parse::<u64>() {
                return Ok(id);
            }
            let list = apps.ok_or_else(|| {
                "app name resolution requires loading applications from the API".to_string()
            })?;
            resolve_app_id_from_list(list, positional)
        }
        (None, None) => {
            Err("provide APP (id or name), --app-id, SCOUT_APP_ID, or SCOUT_APP".to_string())
        }
    }
}

/// True when resolving this target needs a `list_apps` call.
pub fn app_ref_needs_list(app_ref: Option<&str>, flag_id: Option<u64>) -> bool {
    if flag_id.is_some() {
        return false;
    }
    match app_ref.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<u64>().is_err(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_apps() -> Vec<Value> {
        vec![
            json!({"id": 1, "name": "Alpha"}),
            json!({"id": 2, "name": "Beta"}),
            json!({"id": 3, "name": "Alpha"}),
        ]
    }

    #[test]
    fn resolves_numeric_id() {
        assert_eq!(resolve_app_id_from_list(&sample_apps(), "2").unwrap(), 2);
    }

    #[test]
    fn resolves_unique_name_case_insensitive() {
        assert_eq!(resolve_app_id_from_list(&sample_apps(), "beta").unwrap(), 2);
    }

    #[test]
    fn rejects_ambiguous_name() {
        assert!(resolve_app_id_from_list(&sample_apps(), "Alpha").is_err());
    }

    #[test]
    fn flag_wins_when_positional_absent() {
        assert_eq!(resolve_app_target(None, Some(9), None).unwrap(), 9);
    }

    #[test]
    fn needs_list_only_for_names() {
        assert!(app_ref_needs_list(Some("Beta"), None));
        assert!(!app_ref_needs_list(Some("12"), None));
        assert!(!app_ref_needs_list(None, Some(12)));
    }
}
