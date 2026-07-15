//! Home-directory configuration for secret-backend env vars.
//!
//! Reads `$SCOUT_HOME/config.env` and `$SCOUT_HOME/config.local.env` (default
//! `~/.config/scout`, with legacy `~/.scout` fallback). Project `.scout.env` or
//! `.env` in the working directory is also loaded. Process environment variables
//! always win. Plain-text API keys are not loaded.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

const CONFIG_FILE: &str = "config.env";
const LOCAL_CONFIG_FILE: &str = "config.local.env";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    Env,
    LocalFile,
    File,
    ProjectFile,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: Option<String>,
    pub source: Option<ConfigSource>,
}

struct ConfigKeyDef {
    friendly: &'static str,
    env: &'static str,
}

const CONFIG_KEY_DEFS: &[ConfigKeyDef] = &[
    ConfigKeyDef {
        friendly: "op.entry_path",
        env: "SCOUT_OP_ENTRY_PATH",
    },
    ConfigKeyDef {
        friendly: "op.vault",
        env: "SCOUT_OP_VAULT",
    },
    ConfigKeyDef {
        friendly: "op.item",
        env: "SCOUT_OP_ITEM",
    },
    ConfigKeyDef {
        friendly: "op.field",
        env: "SCOUT_OP_FIELD",
    },
    ConfigKeyDef {
        friendly: "bw.item_id",
        env: "SCOUT_BW_ITEM_ID",
    },
    ConfigKeyDef {
        friendly: "bw.session",
        env: "SCOUT_BW_SESSION",
    },
    ConfigKeyDef {
        friendly: "kpxc.db",
        env: "SCOUT_KPXC_DB",
    },
    ConfigKeyDef {
        friendly: "kpxc.entry",
        env: "SCOUT_KPXC_ENTRY",
    },
    ConfigKeyDef {
        friendly: "kpxc.attribute",
        env: "SCOUT_KPXC_ATTRIBUTE",
    },
];

const ALLOWED_CONFIG_KEYS: &[&str] = &[
    "SCOUT_OP_ENTRY_PATH",
    "SCOUT_OP_VAULT",
    "SCOUT_OP_ITEM",
    "SCOUT_OP_FIELD",
    "SCOUT_BW_ITEM_ID",
    "SCOUT_BW_SESSION",
    "SCOUT_KPXC_DB",
    "SCOUT_KPXC_ENTRY",
    "SCOUT_KPXC_ATTRIBUTE",
];

/// Resolve the Scout config directory.
///
/// Precedence: `SCOUT_HOME`, then legacy `~/.scout` when it exists and XDG path does not,
/// otherwise `$XDG_CONFIG_HOME/scout` (default `~/.config/scout`).
pub fn scout_home() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SCOUT_HOME") {
        let path = path.trim();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    let home = home_directory()?;
    let xdg_home = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| home.join(".config"));
    let xdg_scout = xdg_home.join("scout");
    let legacy = home.join(".scout");
    if legacy.exists() && !xdg_scout.exists() {
        Some(legacy)
    } else {
        Some(xdg_scout)
    }
}

static HOME_CONFIG_ONCE: Once = Once::new();

/// Path to the main config file (`config.env`).
pub fn config_file_path() -> Result<PathBuf, String> {
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    Ok(home.join(CONFIG_FILE))
}

/// Resolve a friendly key (`op.entry_path`) or env key (`SCOUT_OP_ENTRY_PATH`).
pub fn resolve_config_key(input: &str) -> Result<String, String> {
    let input = input.trim();
    CONFIG_KEY_DEFS
        .iter()
        .find(|def| def.friendly == input || def.env == input)
        .map(|def| def.env.to_string())
        .ok_or_else(|| format!("unknown config key: {input}"))
}

fn resolve_config_key_def(input: &str) -> Result<&'static ConfigKeyDef, String> {
    let input = input.trim();
    CONFIG_KEY_DEFS
        .iter()
        .find(|def| def.friendly == input || def.env == input)
        .ok_or_else(|| format!("unknown config key: {input}"))
}

pub fn friendly_config_key(input: &str) -> Result<String, String> {
    resolve_config_key_def(input).map(|definition| definition.friendly.to_string())
}

/// List all known config keys with effective values and sources.
pub fn list_config_entries() -> Result<Vec<ConfigEntry>, String> {
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    Ok(CONFIG_KEY_DEFS
        .iter()
        .map(|def| entry_for_key(def, &home))
        .collect())
}

/// Read one config key's effective value.
pub fn get_config_entry(input: &str) -> Result<ConfigEntry, String> {
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    let def = resolve_config_key_def(input)?;
    Ok(entry_for_key(def, &home))
}

/// Write a config value to `config.env` (creates `SCOUT_HOME` when needed).
pub fn set_config_entry(input: &str, value: &str) -> Result<ConfigEntry, String> {
    let def = resolve_config_key_def(input)?;
    let value = value.trim();
    if value.is_empty() {
        return Err("config value must not be empty".to_string());
    }
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    fs::create_dir_all(&home).map_err(|error| format!("create {}: {error}", home.display()))?;
    let path = home.join(CONFIG_FILE);
    let content = if path.is_file() {
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?
    } else {
        String::new()
    };
    let next = upsert_config_line(&content, def.env, value);
    fs::write(&path, next).map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(entry_for_key(def, &home))
}

/// Remove a config value from `config.env`.
pub fn unset_config_entry(input: &str) -> Result<(), String> {
    let def = resolve_config_key_def(input)?;
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    let path = home.join(CONFIG_FILE);
    if !path.is_file() {
        return Ok(());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let next = remove_config_line(&content, def.env);
    fs::write(&path, next).map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(())
}

/// Load home config once per process before reading secret-backend settings.
pub fn ensure_home_config_loaded() {
    HOME_CONFIG_ONCE.call_once(|| {
        let _ = load_home_config();
    });
}

/// Load config files from [`scout_home`] into the process environment.
pub fn load_home_config() -> Result<(), String> {
    let Some(home) = scout_home() else {
        return Ok(());
    };
    load_config_directory(&home)
}

/// Parse `KEY=VALUE` lines from a dotenv-style config file.
pub fn parse_config_content(content: &str) -> HashMap<String, String> {
    let mut entries = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = parse_config_line(line) {
            if is_allowed_config_key(&key) {
                entries.insert(key, value);
            }
        }
    }
    entries
}

fn load_config_directory(home: &Path) -> Result<(), String> {
    let original_keys: HashSet<String> = std::env::vars().map(|(key, _)| key).collect();
    let mut merged = HashMap::new();

    for file_name in project_config_file_names() {
        if let Some(path) = project_config_path(file_name) {
            if !path.is_file() {
                continue;
            }
            let content = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            merged.extend(parse_config_content(&content));
        }
    }

    for file_name in [CONFIG_FILE, LOCAL_CONFIG_FILE] {
        let path = home.join(file_name);
        if !path.is_file() {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        merged.extend(parse_config_content(&content));
    }

    for (key, value) in merged {
        if original_keys.contains(&key) {
            continue;
        }
        // SAFETY: called during startup before other threads read these vars.
        unsafe { std::env::set_var(&key, &value) };
    }

    Ok(())
}

fn project_config_file_names() -> [&'static str; 2] {
    [".scout.env", ".env"]
}

fn project_config_path(file_name: &str) -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|directory| directory.join(file_name))
}

fn parse_config_line(line: &str) -> Option<(String, String)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    let value = parse_config_value(rest.trim());
    Some((key.to_string(), value))
}

fn parse_config_value(raw: &str) -> String {
    if raw.len() >= 2 {
        let bytes = raw.as_bytes();
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[raw.len() - 1] == quote {
            return raw[1..raw.len() - 1].to_string();
        }
    }
    raw.to_string()
}

fn is_allowed_config_key(key: &str) -> bool {
    ALLOWED_CONFIG_KEYS.contains(&key)
}

fn entry_for_key(def: &ConfigKeyDef, home: &Path) -> ConfigEntry {
    let (value, source) = effective_value(def.env, home);
    ConfigEntry {
        key: def.friendly.to_string(),
        value,
        source,
    }
}

fn effective_value(env_key: &str, home: &Path) -> (Option<String>, Option<ConfigSource>) {
    if let Ok(value) = std::env::var(env_key) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return (Some(value), Some(ConfigSource::Env));
        }
    }

    let local_path = home.join(LOCAL_CONFIG_FILE);
    if let Ok(content) = fs::read_to_string(local_path) {
        let entries = parse_config_content(&content);
        if let Some(value) = entries.get(env_key) {
            return (Some(value.clone()), Some(ConfigSource::LocalFile));
        }
    }

    let path = home.join(CONFIG_FILE);
    if let Ok(content) = fs::read_to_string(path) {
        let entries = parse_config_content(&content);
        if let Some(value) = entries.get(env_key) {
            return (Some(value.clone()), Some(ConfigSource::File));
        }
    }

    for file_name in project_config_file_names() {
        if let Some(project_path) = project_config_path(file_name) {
            if let Ok(content) = fs::read_to_string(project_path) {
                let entries = parse_config_content(&content);
                if let Some(value) = entries.get(env_key) {
                    return (Some(value.clone()), Some(ConfigSource::ProjectFile));
                }
            }
        }
    }

    (None, None)
}

fn upsert_config_line(content: &str, key: &str, value: &str) -> String {
    let assignment = format_config_assignment(key, value);
    let mut found = false;
    let mut lines: Vec<String> = if content.is_empty() {
        Vec::new()
    } else {
        content.lines().map(String::from).collect()
    };

    for line in &mut lines {
        if let Some((existing_key, _)) = parse_config_line(line) {
            if existing_key == key {
                *line = assignment.clone();
                found = true;
                break;
            }
        }
    }

    if !found {
        if !lines.is_empty() && lines.last().is_some_and(|line| !line.is_empty()) {
            lines.push(String::new());
        }
        lines.push(assignment);
    }

    join_lines(&lines)
}

fn remove_config_line(content: &str, key: &str) -> String {
    let lines: Vec<String> = content
        .lines()
        .filter(|line| parse_config_line(line).is_none_or(|(existing_key, _)| existing_key != key))
        .map(String::from)
        .collect();
    join_lines(&lines)
}

fn format_config_assignment(key: &str, value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("{key}=\"{value}\"")
    } else {
        format!("{key}={value}")
    }
}

fn join_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn home_directory() -> Option<PathBuf> {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(path) = std::env::var_os(key) {
            return Some(PathBuf::from(path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_test_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn resolve_config_key_accepts_friendly_and_env_names() {
        assert_eq!(
            resolve_config_key("op.entry_path").unwrap(),
            "SCOUT_OP_ENTRY_PATH"
        );
        assert_eq!(
            friendly_config_key("SCOUT_BW_ITEM_ID").unwrap(),
            "bw.item_id"
        );
        assert!(resolve_config_key("api.key").is_err());
    }

    #[test]
    fn upsert_and_remove_config_lines() {
        let updated = upsert_config_line("", "SCOUT_OP_ENTRY_PATH", "op://Vault/Item");
        assert_eq!(updated, "SCOUT_OP_ENTRY_PATH=op://Vault/Item\n");
        let updated = upsert_config_line(
            "# config\nSCOUT_OP_ENTRY_PATH=old\n",
            "SCOUT_OP_ENTRY_PATH",
            "op://Vault/New",
        );
        assert!(updated.contains("SCOUT_OP_ENTRY_PATH=op://Vault/New"));
        assert!(!updated.contains("old"));
        let removed = remove_config_line(&updated, "SCOUT_OP_ENTRY_PATH");
        assert!(!removed.contains("SCOUT_OP_ENTRY_PATH"));
    }

    #[test]
    fn set_and_get_config_entry_round_trip() {
        let _guard = env_test_lock();
        let home = std::env::temp_dir().join(format!("scout-config-cli-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let original_home = std::env::var("SCOUT_HOME").ok();
        std::env::set_var("SCOUT_HOME", home.to_string_lossy().as_ref());
        std::env::remove_var("SCOUT_OP_ENTRY_PATH");

        set_config_entry("op.entry_path", "op://Vault/Scout").unwrap();
        let entry = get_config_entry("op.entry_path").unwrap();
        assert_eq!(entry.value.as_deref(), Some("op://Vault/Scout"));
        assert_eq!(entry.source, Some(ConfigSource::File));
        unset_config_entry("op.entry_path").unwrap();
        let entry = get_config_entry("op.entry_path").unwrap();
        assert_eq!(entry.value, None);

        std::env::remove_var("SCOUT_HOME");
        if let Some(value) = original_home {
            std::env::set_var("SCOUT_HOME", value);
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn parse_config_content_reads_comments_and_quotes() {
        let content = r#"
# backend
SCOUT_OP_ENTRY_PATH=op://Vault/Item
SCOUT_OP_FIELD="API_KEY"
SCOUT_KPXC_ENTRY='Scout APM'
API_KEY=plain-text-not-allowed
"#;
        let entries = parse_config_content(content);
        assert_eq!(
            entries.get("SCOUT_OP_ENTRY_PATH").map(String::as_str),
            Some("op://Vault/Item")
        );
        assert_eq!(
            entries.get("SCOUT_OP_FIELD").map(String::as_str),
            Some("API_KEY")
        );
        assert_eq!(
            entries.get("SCOUT_KPXC_ENTRY").map(String::as_str),
            Some("Scout APM")
        );
        assert!(!entries.contains_key("API_KEY"));
    }

    #[test]
    fn load_config_directory_applies_local_overrides() {
        let _guard = env_test_lock();
        let home = std::env::temp_dir().join(format!("scout-config-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(CONFIG_FILE),
            "SCOUT_OP_ENTRY_PATH=op://Vault/FromConfig\n",
        )
        .unwrap();
        fs::write(
            home.join(LOCAL_CONFIG_FILE),
            "SCOUT_OP_ENTRY_PATH=op://Vault/FromLocal\nSCOUT_OP_FIELD=LOCAL_FIELD\n",
        )
        .unwrap();

        let original_entry = std::env::var("SCOUT_OP_ENTRY_PATH").ok();
        let original_field = std::env::var("SCOUT_OP_FIELD").ok();
        std::env::remove_var("SCOUT_OP_ENTRY_PATH");
        std::env::remove_var("SCOUT_OP_FIELD");

        load_config_directory(&home).unwrap();

        assert_eq!(
            std::env::var("SCOUT_OP_ENTRY_PATH").unwrap(),
            "op://Vault/FromLocal"
        );
        assert_eq!(std::env::var("SCOUT_OP_FIELD").unwrap(), "LOCAL_FIELD");

        std::env::remove_var("SCOUT_OP_ENTRY_PATH");
        std::env::remove_var("SCOUT_OP_FIELD");
        if let Some(value) = original_entry {
            std::env::set_var("SCOUT_OP_ENTRY_PATH", value);
        }
        if let Some(value) = original_field {
            std::env::set_var("SCOUT_OP_FIELD", value);
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn load_config_directory_does_not_override_process_env() {
        let _guard = env_test_lock();
        let home =
            std::env::temp_dir().join(format!("scout-config-precedence-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(CONFIG_FILE),
            "SCOUT_OP_ENTRY_PATH=op://Vault/FromFile\n",
        )
        .unwrap();

        let original_entry = std::env::var("SCOUT_OP_ENTRY_PATH").ok();
        std::env::set_var("SCOUT_OP_ENTRY_PATH", "op://Vault/FromEnv");

        load_config_directory(&home).unwrap();

        assert_eq!(
            std::env::var("SCOUT_OP_ENTRY_PATH").unwrap(),
            "op://Vault/FromEnv"
        );

        std::env::remove_var("SCOUT_OP_ENTRY_PATH");
        if let Some(value) = original_entry {
            std::env::set_var("SCOUT_OP_ENTRY_PATH", value);
        }
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn scout_home_honors_scout_home_env() {
        let _guard = env_test_lock();
        let original = std::env::var("SCOUT_HOME").ok();
        std::env::set_var("SCOUT_HOME", "/tmp/custom-scout-home");
        assert_eq!(
            scout_home().map(|path| path.display().to_string()),
            Some("/tmp/custom-scout-home".to_string())
        );
        std::env::remove_var("SCOUT_HOME");
        if let Some(value) = original {
            std::env::set_var("SCOUT_HOME", value);
        }
    }
}
