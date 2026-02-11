//! Secret backends for reading the Scout APM API key.
//!
//! Resolution is via secret backends only (1Password, Bitwarden, KeePassXC).
//! Plain-text API keys (e.g. env vars or explicit keys) are intentionally not supported;
//! see README and CLI help for the recommended secret-backend setup.

use std::process::Command;

/// Read secret from a subprocess; stderr is discarded to avoid leaking into output.
fn run_cmd(args: &[&str]) -> Option<String> {
    run_cmd_with_env(args, &[])
}

/// Run a command with extra env vars (e.g. pass SCOUT_BW_SESSION as BW_SESSION for `bw`).
fn run_cmd_with_env(args: &[&str], env_extra: &[(&str, &str)]) -> Option<String> {
    let (bin, rest) = args.split_first()?;
    let mut cmd = Command::new(bin);
    cmd.args(rest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    for (k, v) in env_extra {
        cmd.env(k, v);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// 1Password CLI (`op read`).
///
/// Configure via:
/// - `SCOUT_OP_ENTRY_PATH`: `op://Vault/Item` (field name from `SCOUT_OP_FIELD`, default `API_KEY`)
/// - Or `SCOUT_OP_VAULT` + `SCOUT_OP_ITEM` + optional `SCOUT_OP_FIELD` (default `API_KEY`)
pub fn one_password() -> Option<String> {
    let field = std::env::var("SCOUT_OP_FIELD").unwrap_or_else(|_| "API_KEY".to_string());
    let field = field.trim();
    if field.is_empty() {
        return None;
    }

    if let Ok(ref path) = std::env::var("SCOUT_OP_ENTRY_PATH") {
        let path = path.trim();
        if path.is_empty() {
            return None;
        }
        // op://Vault/Item -> op read "op://Vault/Item/Field"
        let base = path.trim_end_matches('/');
        let uri = format!("{}/{}", base, field);
        return run_cmd(&["op", "read", &uri]).filter(|s| !s.is_empty());
    }

    let vault = std::env::var("SCOUT_OP_VAULT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let item = std::env::var("SCOUT_OP_ITEM")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let uri = format!("op://{}/{}/{}", vault, item, field);
    run_cmd(&["op", "read", &uri]).filter(|s| !s.is_empty())
}

/// Bitwarden CLI (`bw get password`).
///
/// Configure via:
/// - `SCOUT_BW_ITEM_ID`: UUID of the login item (from `bw list items`)
/// - `SCOUT_BW_SESSION`: optional session key (from `bw unlock --raw`) if vault is locked
pub fn bitwarden() -> Option<String> {
    let id = std::env::var("SCOUT_BW_ITEM_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let env_extra: Vec<(String, String)> = std::env::var("SCOUT_BW_SESSION")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| ("BW_SESSION".to_string(), s))
        .into_iter()
        .collect();
    let env_refs: Vec<(&str, &str)> = env_extra
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    run_cmd_with_env(&["bw", "get", "password", &id], &env_refs).filter(|s| !s.is_empty())
}

/// KeePassXC CLI (`keepassxc-cli show`).
///
/// Configure via:
/// - `SCOUT_KPXC_DB`: path to the .kdbx database file
/// - `SCOUT_KPXC_ENTRY`: entry title or path (e.g. "Scout APM" or "Web/Scout APM")
/// - `SCOUT_KPXC_ATTRIBUTE`: attribute name (default `Password`)
pub fn keepassxc() -> Option<String> {
    let db = std::env::var("SCOUT_KPXC_DB")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let entry = std::env::var("SCOUT_KPXC_ENTRY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let attr = std::env::var("SCOUT_KPXC_ATTRIBUTE").unwrap_or_else(|_| "Password".to_string());
    let attr = attr.trim();
    if attr.is_empty() {
        return None;
    }
    run_cmd(&["keepassxc-cli", "show", "-a", attr, &db, &entry]).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_password_not_configured() {
        // No env set -> None
        assert!(one_password().is_none());
    }

    #[test]
    fn bitwarden_not_configured() {
        assert!(bitwarden().is_none());
    }

    #[test]
    fn keepassxc_not_configured() {
        assert!(keepassxc().is_none());
    }
}
