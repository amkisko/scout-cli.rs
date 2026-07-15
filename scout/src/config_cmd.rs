//! `scout config` subcommand handlers.

use crate::output::{self, OutputMode};
use crate::util;
use scout_lib::{
    config_file_path, get_config_entry, list_config_entries, scout_home, set_config_entry,
    unset_config_entry, ConfigSource,
};

pub struct ConfigContext {
    pub json: bool,
    pub quiet: bool,
    pub dry_run: bool,
}

pub fn run_path(context: &ConfigContext, mode: OutputMode) -> Result<(), String> {
    let payload = path_config_value()?;
    if context.json {
        output::emit_value(mode, &payload).map_err(|error| error.to_string())?;
        return Ok(());
    }
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    let config_path = config_file_path()?;
    println!("{}", home.display());
    println!("{}", config_path.display());
    Ok(())
}

pub fn run_list(context: &ConfigContext) -> Result<(), String> {
    let entries = list_config_entries()?;
    if context.json {
        let output = serde_json::json!({ "entries": entries });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    for entry in entries {
        match (&entry.value, entry.source) {
            (Some(value), Some(source)) => {
                println!("{}={} ({})", entry.key, value, source_label(source));
            }
            (Some(value), None) => println!("{}={}", entry.key, value),
            (None, _) => println!("{}=", entry.key),
        }
    }
    Ok(())
}

pub fn run_get(key: &str, context: &ConfigContext) -> Result<(), String> {
    let entry = get_config_entry(key)?;
    if context.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entry).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    match (&entry.value, entry.source) {
        (Some(value), Some(source)) => println!("{} ({})", value, source_label(source)),
        (Some(value), None) => println!("{value}"),
        (None, _) => return Err(format!("config key not set: {key}")),
    }
    Ok(())
}

pub fn config_command_value(
    command: crate::cli::ConfigCommands,
    _context: &ConfigContext,
    _mode: OutputMode,
) -> Result<serde_json::Value, String> {
    let value = match command {
        crate::cli::ConfigCommands::List => list_config_value()?,
        crate::cli::ConfigCommands::Get { key } => {
            let entry = get_config_entry(&key)?;
            if entry.value.is_none() {
                return Err(format!("config key not set: {key}"));
            }
            serde_json::to_value(entry).map_err(|error| error.to_string())?
        }
        crate::cli::ConfigCommands::Path => path_config_value()?,
        crate::cli::ConfigCommands::Set { .. } | crate::cli::ConfigCommands::Unset { .. } => {
            return Err("config set/unset are disabled in batch (state-changing)".to_string());
        }
    };
    Ok(value)
}

pub fn run_set(key: &str, value: &str, context: &ConfigContext) -> Result<(), String> {
    if context.dry_run {
        let resolved = scout_lib::friendly_config_key(key)?;
        if context.json {
            let preview = serde_json::json!({
                "key": resolved,
                "value": value,
                "dry_run": true
            });
            println!("{}", serde_json::to_string_pretty(&preview).unwrap());
        } else if !context.quiet {
            util::user_notice(context.quiet, &format!("would set {}={value}", resolved));
        }
        return Ok(());
    }

    let entry = set_config_entry(key, value)?;
    if context.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entry).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if !context.quiet {
        util::user_notice(context.quiet, &format!("set {}", entry.key));
    }
    Ok(())
}

pub fn run_unset(key: &str, context: &ConfigContext) -> Result<(), String> {
    if context.dry_run {
        if context.json {
            let preview = serde_json::json!({ "key": key, "dry_run": true });
            println!("{}", serde_json::to_string_pretty(&preview).unwrap());
        } else if !context.quiet {
            util::user_notice(context.quiet, &format!("would unset {key}"));
        }
        return Ok(());
    }

    unset_config_entry(key)?;
    if !context.quiet {
        util::user_notice(context.quiet, &format!("unset {key}"));
    }
    Ok(())
}

fn path_config_value() -> Result<serde_json::Value, String> {
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    let config_path = config_file_path()?;
    Ok(serde_json::json!({
        "scout_home": home.display().to_string(),
        "config_path": config_path.display().to_string(),
    }))
}

fn list_config_value() -> Result<serde_json::Value, String> {
    let entries = list_config_entries()?;
    Ok(serde_json::json!({ "entries": entries }))
}

fn source_label(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::Env => "env",
        ConfigSource::LocalFile => "local",
        ConfigSource::File => "file",
        ConfigSource::ProjectFile => "project",
    }
}
