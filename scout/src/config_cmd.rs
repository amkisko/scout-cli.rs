//! `scout config` subcommand handlers.

use scout_lib::{
    config_file_path, get_config_entry, list_config_entries, scout_home, set_config_entry,
    unset_config_entry, ConfigSource,
};

pub fn run_path() -> Result<(), String> {
    let home = scout_home().ok_or_else(|| "could not resolve scout home directory".to_string())?;
    let config_path = config_file_path()?;
    println!("{}", home.display());
    println!("{}", config_path.display());
    Ok(())
}

pub fn run_list(json: bool) -> Result<(), String> {
    let entries = list_config_entries()?;
    if json {
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

pub fn run_get(key: &str, json: bool) -> Result<(), String> {
    let entry = get_config_entry(key)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entry).map_err(|e| e.to_string())?
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

pub fn run_set(key: &str, value: &str, json: bool) -> Result<(), String> {
    let entry = set_config_entry(key, value)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entry).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    println!("set {}", entry.key);
    Ok(())
}

pub fn run_unset(key: &str) -> Result<(), String> {
    unset_config_entry(key)?;
    println!("unset {key}");
    Ok(())
}

fn source_label(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::Env => "env",
        ConfigSource::LocalFile => "local",
        ConfigSource::File => "file",
    }
}
