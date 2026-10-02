//! Relative and absolute time parsing for Scout query windows.

use chrono::{DateTime, Duration, Utc};

/// Maximum relative lookback accepted for compact `--from` / `--to` values.
pub const MAX_RELATIVE_DAYS: i64 = 5 * 365;

/// Parse ISO 8601 or compact relative time (`30m`, `1h`, `7d`, `2w`).
///
/// Relative values are offsets before `now` (UTC).
pub fn parse_instant(input: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("empty time input".to_string());
    }
    if let Some(relative) = parse_compact_relative(input)? {
        if relative.num_seconds() < 0 {
            return Err(format!(
                "relative time {input:?} is negative — use a positive value like 1h or 7d"
            ));
        }
        if relative.num_days() > MAX_RELATIVE_DAYS {
            return Err(format!(
                "relative time {input:?} is too far in the past (maximum {MAX_RELATIVE_DAYS} days)"
            ));
        }
        return Ok(now - relative);
    }
    parse_iso8601(input)
}

/// Parse a range duration string into seconds.
///
/// Accepts `30min`, `1hour`, `7days`, and compact `30m`, `1h`, `7d`, `2w`.
pub fn parse_duration_secs(range_str: &str) -> Result<u64, String> {
    let s = range_str.trim().to_lowercase().replace(' ', "");
    if s.is_empty() {
        return Err(format!("Invalid range: {range_str}"));
    }
    if let Some(duration) = parse_compact_relative(&s)? {
        if duration.num_seconds() <= 0 {
            return Err(format!("Invalid range: {range_str}"));
        }
        return Ok(duration.num_seconds() as u64);
    }
    let mut num_end = 0;
    for character in s.chars() {
        if character.is_ascii_digit() {
            num_end += 1;
        } else {
            break;
        }
    }
    if num_end == 0 {
        return Err(format!("Invalid range: {range_str}"));
    }
    let num: u64 = s[..num_end]
        .parse()
        .map_err(|_| format!("Invalid range: {range_str}"))?;
    let unit = s[num_end..].trim();
    let secs = match unit {
        u if u.starts_with("min") => num * 60,
        u if u.starts_with("hr") || u.starts_with("hour") => num * 3600,
        u if u.starts_with("day") => num * 86400,
        u if u.starts_with("week") => num * 7 * 86400,
        _ => return Err(format!("Unknown time unit in range: {range_str}")),
    };
    Ok(secs)
}

/// Resolve `(from, to)` ISO strings. Defaults: `to` = now, `from` = now − 3h when both absent.
pub fn resolve_timeframe(
    from: Option<&str>,
    to: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(String, String), String> {
    let to_time = match to {
        Some(value) => parse_instant(value, now)?,
        None => now,
    };
    let from_time = match from {
        Some(value) => parse_instant(value, now)?,
        None => now - Duration::hours(3),
    };
    if to_time <= from_time {
        return Err(format!(
            "invalid timeframe: --to ({}) must be after --from ({})",
            format_time(to_time),
            format_time(from_time)
        ));
    }
    Ok((format_time(from_time), format_time(to_time)))
}

/// Compute `(from, to)` for a `--range` ending at `to` (or now).
pub fn calculate_range_at(
    range: &str,
    to: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(String, String), String> {
    let end_time = match to {
        Some(value) => parse_instant(value, now)?,
        None => now,
    };
    let secs = parse_duration_secs(range)?;
    let start_time = end_time - Duration::seconds(secs as i64);
    if end_time <= start_time {
        return Err(format!(
            "invalid timeframe: --to ({}) must be after --from ({})",
            format_time(end_time),
            format_time(start_time)
        ));
    }
    Ok((format_time(start_time), format_time(end_time)))
}

pub fn format_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn parse_compact_relative(input: &str) -> Result<Option<Duration>, String> {
    let bytes = input.as_bytes();
    if bytes.len() < 2 {
        return Ok(None);
    }
    let unit = *bytes.last().unwrap() as char;
    if !matches!(unit, 'm' | 'h' | 'd' | 'w') {
        return Ok(None);
    }
    // Reject long-form units that end in these letters (min, hour, day, week).
    if input.len() > 2
        && (input.ends_with("min")
            || input.ends_with("mins")
            || input.ends_with("minute")
            || input.ends_with("minutes")
            || input.ends_with("hr")
            || input.ends_with("hrs")
            || input.ends_with("hour")
            || input.ends_with("hours")
            || input.ends_with("day")
            || input.ends_with("days")
            || input.ends_with("week")
            || input.ends_with("weeks"))
    {
        return Ok(None);
    }
    let num_str = &input[..input.len() - 1];
    if !num_str.chars().all(|c| c.is_ascii_digit()) {
        return Ok(None);
    }
    let n: i64 = num_str
        .parse()
        .map_err(|_| format!("invalid relative time: {input}"))?;
    if n < 0 {
        return Err(format!(
            "relative time {input:?} is negative — use a positive value like 1h or 7d"
        ));
    }
    let duration = match unit {
        'm' => Duration::minutes(n),
        'h' => Duration::hours(n),
        'd' => Duration::days(n),
        'w' => Duration::days(n * 7),
        _ => return Ok(None),
    };
    Ok(Some(duration))
}

fn parse_iso8601(input: &str) -> Result<DateTime<Utc>, String> {
    let trimmed = input.trim().trim_end_matches('Z').trim_end_matches('z');
    DateTime::parse_from_rfc3339(&format!("{trimmed}Z"))
        .or_else(|_| DateTime::parse_from_rfc3339(input))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S")
                .map(|naive| naive.and_utc().fixed_offset())
        })
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc().fixed_offset())
        })
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| {
            format!(
                "invalid time format: {input:?} (use relative like 1h, 7d, 30m, 2w or ISO 8601)"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 2, 12, 0, 0).unwrap()
    }

    #[test]
    fn compact_relative_units() {
        assert_eq!(parse_duration_secs("30m").unwrap(), 30 * 60);
        assert_eq!(parse_duration_secs("1h").unwrap(), 3600);
        assert_eq!(parse_duration_secs("7d").unwrap(), 7 * 86400);
        assert_eq!(parse_duration_secs("2w").unwrap(), 14 * 86400);
    }

    #[test]
    fn long_form_still_works() {
        assert_eq!(parse_duration_secs("30min").unwrap(), 30 * 60);
        assert_eq!(parse_duration_secs("7days").unwrap(), 7 * 86400);
        assert_eq!(parse_duration_secs("1week").unwrap(), 7 * 86400);
    }

    #[test]
    fn resolve_timeframe_rejects_reversed() {
        let err = resolve_timeframe(
            Some("2026-10-02T12:00:00Z"),
            Some("2026-10-01T12:00:00Z"),
            fixed_now(),
        )
        .unwrap_err();
        assert!(err.contains("must be after"));
    }

    #[test]
    fn parse_instant_relative() {
        let t = parse_instant("1h", fixed_now()).unwrap();
        assert_eq!(t, fixed_now() - Duration::hours(1));
    }

    #[test]
    fn parse_instant_rejects_huge_relative() {
        let err = parse_instant("9999d", fixed_now()).unwrap_err();
        assert!(err.contains("too far"));
    }

    #[test]
    fn default_timeframe_is_three_hours() {
        let (from, to) = resolve_timeframe(None, None, fixed_now()).unwrap();
        assert_eq!(to, "2026-10-02T12:00:00Z");
        assert_eq!(from, "2026-10-02T09:00:00Z");
    }
}
