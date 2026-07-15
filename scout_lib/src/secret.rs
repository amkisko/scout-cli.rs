//! Secret backends for reading the Scout APM API key.

use std::process::Command;

/// Result of attempting to read a configured secret backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendAttempt {
    NotConfigured,
    Success,
    Failed(String),
}

/// Read secret from a subprocess; stderr is discarded to avoid leaking into output.
fn run_cmd(args: &[&str]) -> Option<String> {
    run_cmd_with_env(args, &[])
}

/// Run a command with extra env vars (e.g. pass SCOUT_BW_SESSION as BW_SESSION for `bw`).
fn run_cmd_with_env(args: &[&str], env_extra: &[(&str, &str)]) -> Option<String> {
    let (binary, rest) = args.split_first()?;
    let mut command = Command::new(binary);
    command
        .args(rest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in env_extra {
        command.env(key, value);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn run_cmd_with_diagnostics(
    backend: &str,
    args: &[&str],
    env_extra: &[(&str, &str)],
) -> BackendAttempt {
    let (binary, rest) = match args.split_first() {
        Some(parts) => parts,
        None => return BackendAttempt::NotConfigured,
    };
    let mut command = Command::new(binary);
    command
        .args(rest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in env_extra {
        command.env(key, value);
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            return BackendAttempt::Failed(format!("{backend}: failed to run ({error})"));
        }
    };
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .trim()
            .lines()
            .next()
            .unwrap_or("command failed")
            .to_string();
        return BackendAttempt::Failed(format!("{backend}: {detail}"));
    }
    let secret = String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    match secret {
        Some(_) => BackendAttempt::Success,
        None => BackendAttempt::Failed(format!("{backend}: returned an empty secret")),
    }
}

pub fn one_password_configured() -> bool {
    std::env::var("SCOUT_OP_ENTRY_PATH")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        || (std::env::var("SCOUT_OP_VAULT")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
            && std::env::var("SCOUT_OP_ITEM")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false))
}

pub fn bitwarden_configured() -> bool {
    std::env::var("SCOUT_BW_ITEM_ID")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub fn keepassxc_configured() -> bool {
    std::env::var("SCOUT_KPXC_DB")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        && std::env::var("SCOUT_KPXC_ENTRY")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
}

fn one_password_entry_path_includes_field(entry_path: &str) -> bool {
    let rest = entry_path
        .strip_prefix("op://")
        .unwrap_or(entry_path);
    rest.split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .count()
        >= 3
}

fn one_password_uri_from_entry_path(entry_path: &str, field: &str) -> String {
    let base = entry_path.trim().trim_end_matches('/');
    if one_password_entry_path_includes_field(base) {
        base.to_string()
    } else {
        format!("{base}/{field}")
    }
}

/// 1Password CLI (`op read`).
pub fn one_password() -> Option<String> {
    let field = std::env::var("SCOUT_OP_FIELD").unwrap_or_else(|_| "API_KEY".to_string());
    let field = field.trim();
    if field.is_empty() {
        return None;
    }

    if let Ok(path) = std::env::var("SCOUT_OP_ENTRY_PATH") {
        let path = path.trim();
        if path.is_empty() {
            return None;
        }
        let uri = one_password_uri_from_entry_path(path, field);
        return run_cmd(&["op", "read", &uri]);
    }

    let vault = std::env::var("SCOUT_OP_VAULT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let item = std::env::var("SCOUT_OP_ITEM")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let uri = format!("op://{vault}/{item}/{field}");
    run_cmd(&["op", "read", &uri])
}

pub fn one_password_attempt() -> BackendAttempt {
    if !one_password_configured() {
        return BackendAttempt::NotConfigured;
    }
    let field = std::env::var("SCOUT_OP_FIELD").unwrap_or_else(|_| "API_KEY".to_string());
    let field = field.trim();
    if field.is_empty() {
        return BackendAttempt::NotConfigured;
    }
    if let Ok(path) = std::env::var("SCOUT_OP_ENTRY_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            let uri = one_password_uri_from_entry_path(path, field);
            return run_cmd_with_diagnostics("1Password", &["op", "read", &uri], &[]);
        }
    }
    let vault = match std::env::var("SCOUT_OP_VAULT") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return BackendAttempt::NotConfigured,
    };
    let item = match std::env::var("SCOUT_OP_ITEM") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return BackendAttempt::NotConfigured,
    };
    let uri = format!("op://{vault}/{item}/{field}");
    run_cmd_with_diagnostics("1Password", &["op", "read", &uri], &[])
}

/// Bitwarden CLI (`bw get password`).
pub fn bitwarden() -> Option<String> {
    let id = std::env::var("SCOUT_BW_ITEM_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let env_extra: Vec<(String, String)> = std::env::var("SCOUT_BW_SESSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| ("BW_SESSION".to_string(), value))
        .into_iter()
        .collect();
    let env_refs: Vec<(&str, &str)> = env_extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    run_cmd_with_env(&["bw", "get", "password", &id], &env_refs)
}

pub fn bitwarden_attempt() -> BackendAttempt {
    if !bitwarden_configured() {
        return BackendAttempt::NotConfigured;
    }
    let id = std::env::var("SCOUT_BW_ITEM_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap();
    let env_extra: Vec<(String, String)> = std::env::var("SCOUT_BW_SESSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| ("BW_SESSION".to_string(), value))
        .into_iter()
        .collect();
    let env_refs: Vec<(&str, &str)> = env_extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    run_cmd_with_diagnostics("Bitwarden", &["bw", "get", "password", &id], &env_refs)
}

/// KeePassXC CLI (`keepassxc-cli show`).
pub fn keepassxc() -> Option<String> {
    let database = std::env::var("SCOUT_KPXC_DB")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let entry = std::env::var("SCOUT_KPXC_ENTRY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let attribute =
        std::env::var("SCOUT_KPXC_ATTRIBUTE").unwrap_or_else(|_| "Password".to_string());
    let attribute = attribute.trim();
    if attribute.is_empty() {
        return None;
    }
    run_cmd(&["keepassxc-cli", "show", "-a", attribute, &database, &entry])
}

pub fn keepassxc_attempt() -> BackendAttempt {
    if !keepassxc_configured() {
        return BackendAttempt::NotConfigured;
    }
    let database = std::env::var("SCOUT_KPXC_DB")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap();
    let entry = std::env::var("SCOUT_KPXC_ENTRY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap();
    let attribute =
        std::env::var("SCOUT_KPXC_ATTRIBUTE").unwrap_or_else(|_| "Password".to_string());
    let attribute = attribute.trim();
    if attribute.is_empty() {
        return BackendAttempt::NotConfigured;
    }
    run_cmd_with_diagnostics(
        "KeePassXC",
        &[
            "keepassxc-cli",
            "show",
            "-a",
            attribute,
            &database,
            &entry,
        ],
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_password_not_configured() {
        assert!(one_password().is_none());
        assert_eq!(one_password_attempt(), BackendAttempt::NotConfigured);
    }

    #[test]
    fn one_password_uri_from_entry_path_appends_field_when_missing() {
        assert_eq!(
            one_password_uri_from_entry_path("op://Vault/Scout APM", "API_KEY"),
            "op://Vault/Scout APM/API_KEY"
        );
        assert_eq!(
            one_password_uri_from_entry_path("op://Vault/Scout APM", "password"),
            "op://Vault/Scout APM/password"
        );
    }

    #[test]
    fn one_password_uri_from_entry_path_respects_existing_field() {
        assert_eq!(
            one_password_uri_from_entry_path("op://Employee/Scout APM/API_KEY", "API_KEY"),
            "op://Employee/Scout APM/API_KEY"
        );
        assert_eq!(
            one_password_uri_from_entry_path("op://Employee/Scout APM/credential", "API_KEY"),
            "op://Employee/Scout APM/credential"
        );
    }

    #[test]
    fn bitwarden_not_configured() {
        assert!(bitwarden().is_none());
        assert_eq!(bitwarden_attempt(), BackendAttempt::NotConfigured);
    }

    #[test]
    fn keepassxc_not_configured() {
        assert!(keepassxc().is_none());
        assert_eq!(keepassxc_attempt(), BackendAttempt::NotConfigured);
    }
}
