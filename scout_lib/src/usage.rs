//! Web-transaction usage math from throughput series.

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

const MAX_CHUNK: Duration = Duration::days(14);

/// Split `[from, to]` into chunks of at most 14 days for Scout metric queries.
pub fn split_timeframe(from: &str, to: &str) -> Result<Vec<(String, String)>, String> {
    let mut from_time = crate::time_parse::parse_instant(from, Utc::now())?;
    let to_time = crate::time_parse::parse_instant(to, Utc::now())?;
    if to_time <= from_time {
        return Err("invalid timeframe for usage split".to_string());
    }
    let mut chunks = Vec::new();
    while from_time < to_time {
        let mut chunk_end = from_time + MAX_CHUNK;
        if chunk_end > to_time {
            chunk_end = to_time;
        }
        chunks.push((
            crate::time_parse::format_time(from_time),
            crate::time_parse::format_time(chunk_end),
        ));
        from_time = chunk_end;
    }
    Ok(chunks)
}

/// Sum web transactions from throughput RPM points: prev_rpm * interval_minutes.
pub fn calculate_transactions(points: &[(String, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for window in points.windows(2) {
        total += interval_transactions(&window[0], &window[1]);
    }
    total
}

fn interval_transactions(prev: &(String, f64), curr: &(String, f64)) -> f64 {
    let Ok(t0) = crate::time_parse::parse_instant(&prev.0, Utc::now()) else {
        return 0.0;
    };
    let Ok(t1) = crate::time_parse::parse_instant(&curr.0, Utc::now()) else {
        return 0.0;
    };
    let minutes = (t1 - t0).num_milliseconds() as f64 / 60_000.0;
    if minutes <= 0.0 {
        return 0.0;
    }
    prev.1 * minutes
}

/// Extract `[timestamp, value]` points from a Scout metric series payload.
pub fn series_points(series: &Value) -> Vec<(String, f64)> {
    let mut points = Vec::new();
    collect_points(series, &mut points);
    points.sort_by(|a, b| a.0.cmp(&b.0));
    points
}

fn collect_points(value: &Value, out: &mut Vec<(String, f64)>) {
    match value {
        Value::Array(items) => {
            if items.len() == 2 {
                if let (Some(ts), Some(v)) = (items[0].as_str(), as_f64(&items[1])) {
                    out.push((ts.to_string(), v));
                    return;
                }
            }
            for item in items {
                collect_points(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(throughput) = map.get("throughput") {
                collect_points(throughput, out);
                return;
            }
            for nested in map.values() {
                collect_points(nested, out);
            }
        }
        _ => {}
    }
}

fn as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
}

/// Bucket transaction totals by UTC calendar day.
pub fn bucket_by_day(points: &[(String, f64)]) -> Vec<(String, i64)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut days: Vec<(String, f64)> = Vec::new();
    for window in points.windows(2) {
        let Ok(t0) = crate::time_parse::parse_instant(&window[0].0, Utc::now()) else {
            continue;
        };
        let day = t0.format("%Y-%m-%d").to_string();
        let tx = interval_transactions(&window[0], &window[1]);
        if let Some(last) = days.last_mut() {
            if last.0 == day {
                last.1 += tx;
                continue;
            }
        }
        days.push((day, tx));
    }
    days.into_iter()
        .map(|(date, tx)| (date, tx.round() as i64))
        .collect()
}

/// Clamp a billing period end that is still in the future down to `now`.
pub fn clamp_billing_end(end: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    if end > now {
        now
    } else {
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_two_weeks_is_one_chunk() {
        let chunks = split_timeframe("2026-01-01T00:00:00Z", "2026-01-10T00:00:00Z").unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn split_month_into_chunks() {
        let chunks = split_timeframe("2026-01-01T00:00:00Z", "2026-02-01T00:00:00Z").unwrap();
        assert!(chunks.len() >= 2);
        assert_eq!(chunks.first().unwrap().0, "2026-01-01T00:00:00Z");
        assert_eq!(chunks.last().unwrap().1, "2026-02-01T00:00:00Z");
    }

    #[test]
    fn transactions_from_rpm_points() {
        let points = vec![
            ("2026-01-01T00:00:00Z".to_string(), 60.0),
            ("2026-01-01T00:01:00Z".to_string(), 0.0),
        ];
        assert!((calculate_transactions(&points) - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn series_points_from_named_throughput() {
        let series = json!({
            "throughput": [
                ["2026-01-01T00:00:00Z", 10.0],
                ["2026-01-01T00:01:00Z", 20.0]
            ]
        });
        let points = series_points(&series);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].1, 10.0);
    }

    #[test]
    fn bucket_by_day_groups() {
        let points = vec![
            ("2026-01-01T00:00:00Z".to_string(), 60.0),
            ("2026-01-01T00:01:00Z".to_string(), 0.0),
            ("2026-01-02T00:00:00Z".to_string(), 30.0),
            ("2026-01-02T00:01:00Z".to_string(), 0.0),
        ];
        let days = bucket_by_day(&points);
        assert_eq!(
            days,
            vec![
                ("2026-01-01".to_string(), 60),
                ("2026-01-02".to_string(), 30)
            ]
        );
    }
}
