//! Output formatting: plain text (human-readable) and JSON.

use serde_json::Value;
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Human-readable tables and key-value
    #[default]
    Plain,
    /// JSON (pretty-printed)
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plain" | "text" | "p" => Ok(OutputFormat::Plain),
            "json" | "j" => Ok(OutputFormat::Json),
            _ => Err(format!("unknown output format: {}", s)),
        }
    }
}

/// Format value as plain text (tables for arrays of objects, key-value for objects).
pub fn format_plain(value: &Value) -> String {
    let mut out = String::new();
    format_plain_impl(value, &mut out, 0);
    out
}

fn format_plain_impl(v: &Value, out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Null => {
            let _ = writeln!(out, "{}null", pad);
        }
        Value::Bool(b) => {
            let _ = writeln!(out, "{}{}", pad, b);
        }
        Value::Number(n) => {
            let _ = writeln!(out, "{}{}", pad, n);
        }
        Value::String(s) => {
            let _ = writeln!(out, "{}{}", pad, s);
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                _ = writeln!(out, "{}<empty>", pad);
                return;
            }
            let first = &arr[0];
            if first.is_object() && arr.len() > 1 {
                let keys = object_keys(first);
                if !keys.is_empty() {
                    let header: String = keys
                        .iter()
                        .map(|k| format!("{:>12}", k))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(out, "{}{}", pad, header);
                    let _ = writeln!(out, "{}{}", pad, "-".repeat(header.len().min(80)));
                    for obj in arr {
                        if let Value::Object(m) = obj {
                            let row: String = keys
                                .iter()
                                .map(|k| {
                                    let val = m
                                        .get(k)
                                        .and_then(as_short_str)
                                        .unwrap_or_else(|| "-".to_string());
                                    format!("{:>12}", truncate(val.as_str(), 12))
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let _ = writeln!(out, "{}{}", pad, row);
                        }
                    }
                    return;
                }
            }
            for (i, item) in arr.iter().enumerate() {
                if item.is_object() || item.is_array() {
                    let _ = writeln!(out, "{}[{}]", pad, i + 1);
                    format_plain_impl(item, out, indent + 1);
                } else {
                    let _ = writeln!(out, "{}{}", pad, item);
                }
            }
        }
        Value::Object(map) => {
            for (k, val) in map {
                if val.is_object() || val.is_array() {
                    let _ = writeln!(out, "{}{}:", pad, k);
                    format_plain_impl(val, out, indent + 1);
                } else {
                    let s = as_short_str(val).unwrap_or_else(|| "null".to_string());
                    let _ = writeln!(out, "{}{}: {}", pad, k, s);
                }
            }
        }
    }
}

fn object_keys(obj: &Value) -> Vec<String> {
    obj.as_object()
        .map(|m| m.keys().map(String::clone).collect::<Vec<_>>())
        .unwrap_or_default()
}

fn as_short_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        _ => None,
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() <= max {
        s
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

/// Format value as JSON (pretty).
pub fn format_json(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Format value as JSON (compact). Use for machine output.
#[allow(dead_code)]
pub fn format_json_compact(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_from_str() {
        assert_eq!(
            "plain".parse::<OutputFormat>().unwrap(),
            OutputFormat::Plain
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "Plain".parse::<OutputFormat>().unwrap(),
            OutputFormat::Plain
        );
        assert_eq!("JSON".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("p".parse::<OutputFormat>().unwrap(), OutputFormat::Plain);
        assert_eq!("j".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Plain);
        assert!("xml".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn format_plain_null() {
        assert!(format_plain(&Value::Null).contains("null"));
    }

    #[test]
    fn format_plain_bool_and_number() {
        assert!(format_plain(&Value::Bool(true)).contains("true"));
        assert!(format_plain(&Value::Number(42i64.into())).contains("42"));
    }

    #[test]
    fn format_plain_string() {
        assert!(format_plain(&Value::String("hello".to_string())).contains("hello"));
    }

    #[test]
    fn format_plain_empty_array() {
        let out = format_plain(&Value::Array(vec![]));
        assert!(out.contains("empty"));
    }

    #[test]
    fn format_plain_object() {
        let v = serde_json::json!({"name": "scout", "count": 1});
        let out = format_plain(&v);
        assert!(out.contains("name"));
        assert!(out.contains("scout"));
        assert!(out.contains("count"));
    }

    #[test]
    fn format_plain_array_of_objects() {
        let v = serde_json::json!([
            {"id": 1, "name": "a"},
            {"id": 2, "name": "b"}
        ]);
        let out = format_plain(&v);
        assert!(out.contains("id"));
        assert!(out.contains("name"));
        assert!(out.contains("1"));
        assert!(out.contains("2"));
        assert!(out.contains("a"));
        assert!(out.contains("b"));
    }

    #[test]
    fn format_json_roundtrip() {
        let v = serde_json::json!({"x": 1, "y": [2, 3]});
        let s = format_json(&v).unwrap();
        let parsed: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, v);
    }
}
