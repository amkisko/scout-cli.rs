//! Output formatting for human, script, and JSON consumers.

use crate::util::{stdout_is_tty, terminal_columns};
use serde_json::Value;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    HumanPlain,
    ScriptPlain,
    JsonCompact,
    JsonPretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Plain,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "plain" | "text" | "p" => Ok(OutputFormat::Plain),
            "json" | "j" => Ok(OutputFormat::Json),
            _ => Err(format!("unknown output format: {value}")),
        }
    }
}

pub fn resolve_output_mode(
    output: OutputFormat,
    json_flag: bool,
    plain_script_flag: bool,
    json_pretty_flag: bool,
) -> OutputMode {
    if json_flag {
        return OutputMode::JsonCompact;
    }
    if plain_script_flag {
        return OutputMode::ScriptPlain;
    }
    if json_pretty_flag {
        return OutputMode::JsonPretty;
    }
    match output {
        OutputFormat::Plain => OutputMode::HumanPlain,
        OutputFormat::Json => OutputMode::JsonPretty,
    }
}

pub fn format_value(mode: OutputMode, value: &Value) -> Result<String, String> {
    match mode {
        OutputMode::HumanPlain => Ok(format_human_plain(value, terminal_columns())),
        OutputMode::ScriptPlain => Ok(format_script_plain(value)),
        OutputMode::JsonCompact => {
            serde_json::to_string(value).map_err(|error| error.to_string())
        }
        OutputMode::JsonPretty => {
            serde_json::to_string_pretty(value).map_err(|error| error.to_string())
        }
    }
}

pub fn emit_value(mode: OutputMode, value: &Value) -> Result<(), String> {
    let formatted = format_value(mode, value)?;
    emit_text(&formatted, mode == OutputMode::HumanPlain)
}

pub fn emit_text(text: &str, use_pager: bool) -> Result<(), String> {
    if use_pager
        && (stdout_is_tty() || crate::util::stdin_is_tty())
        && text.lines().count() > terminal_rows()
    {
        pipe_to_pager(text)?;
    } else {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn terminal_rows() -> usize {
    std::env::var("LINES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|rows| *rows > 0)
        .unwrap_or(24)
}

fn pipe_to_pager(text: &str) -> Result<(), String> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let pager_program = pager.split_whitespace().next().unwrap_or("less");
    let pager_args: Vec<&str> = pager.split_whitespace().skip(1).collect();

    let mut command = Command::new(pager_program);
    command.stdin(Stdio::piped()).stdout(Stdio::inherit());
    if pager_program == "less" && pager_args.is_empty() {
        command.arg("-FIRX");
    } else {
        command.args(pager_args);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("start pager: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("write pager input: {error}"))?;
    }
    let status = child
        .wait()
        .map_err(|error| format!("wait for pager: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("pager exited with an error".to_string())
    }
}

pub fn format_human_plain(value: &Value, max_width: usize) -> String {
    let mut output = String::new();
    format_human_plain_impl(value, &mut output, 0, max_width);
    output
}

fn format_human_plain_impl(value: &Value, output: &mut String, indent: usize, max_width: usize) {
    let padding = "  ".repeat(indent);
    match value {
        Value::Null => {
            let _ = writeln!(output, "{padding}null");
        }
        Value::Bool(boolean) => {
            let _ = writeln!(output, "{padding}{boolean}");
        }
        Value::Number(number) => {
            let _ = writeln!(output, "{padding}{number}");
        }
        Value::String(text) => {
            let _ = writeln!(output, "{padding}{text}");
        }
        Value::Array(items) => {
            if items.is_empty() {
                let _ = writeln!(output, "{padding}<empty>");
                return;
            }
            if items.iter().all(Value::is_object) {
                let keys = union_object_keys(items);
                if !keys.is_empty() {
                    render_table(output, &padding, &keys, items, max_width);
                    return;
                }
            }
            for (index, item) in items.iter().enumerate() {
                if item.is_object() || item.is_array() {
                    let _ = writeln!(output, "{padding}[{}]", index + 1);
                    format_human_plain_impl(item, output, indent + 1, max_width);
                } else {
                    let _ = writeln!(output, "{padding}{item}");
                }
            }
        }
        Value::Object(map) => {
            for (key, nested) in map {
                if nested.is_object() || nested.is_array() {
                    let _ = writeln!(output, "{padding}{key}:");
                    format_human_plain_impl(nested, output, indent + 1, max_width);
                } else {
                    let rendered = as_short_str(nested).unwrap_or_else(|| "null".to_string());
                    let _ = writeln!(output, "{padding}{key}: {rendered}");
                }
            }
        }
    }
}

fn render_table(
    output: &mut String,
    padding: &str,
    keys: &[String],
    rows: &[Value],
    max_width: usize,
) {
    let column_width = ((max_width.saturating_sub(keys.len())) / keys.len().max(1)).clamp(8, 24);
    let header = keys
        .iter()
        .map(|key| format!("{key:>column_width$}"))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(output, "{padding}{header}");
    let _ = writeln!(
        output,
        "{padding}{}",
        "-".repeat(header.len().min(max_width))
    );
    for row in rows {
        if let Value::Object(map) = row {
            let line = keys
                .iter()
                .map(|key| {
                    let cell = map
                        .get(key)
                        .and_then(as_short_str)
                        .unwrap_or_else(|| "-".to_string());
                    format!("{:>width$}", truncate(&cell, column_width), width = column_width)
                })
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(output, "{padding}{line}");
        }
    }
}

pub fn format_script_plain(value: &Value) -> String {
    match value {
        Value::Array(items) if items.iter().all(Value::is_object) => items
            .iter()
            .map(format_script_record)
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => format_script_record(value),
        other => other.to_string(),
    }
}

fn format_script_record(value: &Value) -> String {
    let Some(map) = value.as_object() else {
        return value.to_string();
    };
    map.iter()
        .map(|(key, nested)| {
            let rendered = as_short_str(nested).unwrap_or_else(|| nested.to_string());
            format!("{key}={rendered}")
        })
        .collect::<Vec<_>>()
        .join("\t")
}

fn union_object_keys(items: &[Value]) -> Vec<String> {
    let mut keys = Vec::new();
    for item in items {
        if let Some(map) = item.as_object() {
            for key in map.keys() {
                if !keys.iter().any(|existing| existing == key) {
                    keys.push(key.clone());
                }
            }
        }
    }
    keys
}

fn as_short_str(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        Value::Null => Some("null".to_string()),
        _ => None,
    }
}

fn truncate(text: &str, max: usize) -> String {
    let flattened = text.replace('\n', " ");
    if flattened.chars().count() <= max {
        flattened
    } else {
        let end = flattened
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(index, _)| index)
            .unwrap_or(flattened.len());
        format!("{}…", &flattened[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_output_mode_prefers_flags() {
        assert_eq!(
            resolve_output_mode(OutputFormat::Plain, true, false, false),
            OutputMode::JsonCompact
        );
        assert_eq!(
            resolve_output_mode(OutputFormat::Json, false, true, false),
            OutputMode::ScriptPlain
        );
    }

    #[test]
    fn format_script_plain_one_record_per_line() {
        let value = serde_json::json!([
            {"id": 1, "name": "a"},
            {"id": 2, "name": "b"}
        ]);
        let output = format_script_plain(&value);
        assert_eq!(output.lines().count(), 2);
        assert!(output.contains("id=1"));
        assert!(output.contains("name=b"));
    }

    #[test]
    fn format_json_roundtrip() {
        let value = serde_json::json!({"x": 1, "y": [2, 3]});
        let rendered = format_value(OutputMode::JsonPretty, &value).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed, value);
    }
}
