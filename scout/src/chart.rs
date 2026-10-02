//! Chart series helpers for TUI metric plots.

use chrono::{DateTime, Utc};

/// Keep the largest-magnitude value in each downsample bucket.
pub fn downsample_peak(points: &[(String, f64)], max_count: usize) -> Vec<(String, f64)> {
    if max_count == 0 || points.len() <= max_count {
        return points.to_vec();
    }
    let mut result = Vec::with_capacity(max_count);
    for index in 0..max_count {
        let start = index * points.len() / max_count;
        let mut end = (index + 1) * points.len() / max_count;
        if end <= start {
            end = start + 1;
        }
        let mut peak = &points[start];
        for candidate in &points[start + 1..end] {
            if candidate.1.abs() > peak.1.abs() {
                peak = candidate;
            }
        }
        result.push(peak.clone());
    }
    result
}

/// Drop the trailing partial time bucket when it is still filling.
pub fn trim_partial_bucket(points: &[(String, f64)], now: DateTime<Utc>) -> Vec<(String, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let Ok(last) = scout_lib::parse_instant(&points[points.len() - 1].0, now) else {
        return points.to_vec();
    };
    let Ok(prev) = scout_lib::parse_instant(&points[points.len() - 2].0, now) else {
        return points.to_vec();
    };
    let interval = last - prev;
    if interval.num_milliseconds() <= 0 {
        return points.to_vec();
    }
    if last + interval > now {
        return points[..points.len() - 1].to_vec();
    }
    points.to_vec()
}

/// Format a chart statistic, promoting large millisecond values to seconds.
pub fn format_stat(value: f64, unit: &str) -> String {
    if unit == "ms" && value.abs() >= 1000.0 {
        return format!("{:.1}s", value / 1000.0);
    }
    if value.abs() >= 1000.0 {
        return format!("{:.1}k{unit}", value / 1000.0);
    }
    if (value - value.trunc()).abs() < f64::EPSILON {
        return format!("{value:.0}{unit}");
    }
    format!("{value:.2}{unit}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn downsample_keeps_spike() {
        let mut points = Vec::new();
        for i in 0..10 {
            points.push((format!("t{i}"), 1.0));
        }
        points[3].1 = 100.0;
        let sampled = downsample_peak(&points, 2);
        assert_eq!(sampled.len(), 2);
        assert!(sampled
            .iter()
            .any(|(_, v)| (*v - 100.0).abs() < f64::EPSILON));
    }

    #[test]
    fn trim_drops_open_bucket() {
        let now = Utc.with_ymd_and_hms(2026, 10, 2, 12, 0, 0).unwrap();
        let points = vec![
            ("2026-10-02T11:00:00Z".to_string(), 10.0),
            ("2026-10-02T11:30:00Z".to_string(), 10.0),
            ("2026-10-02T12:00:00Z".to_string(), 0.0),
        ];
        let trimmed = trim_partial_bucket(&points, now);
        assert_eq!(trimmed.len(), 2);
    }

    #[test]
    fn ms_promotes_to_seconds() {
        assert_eq!(format_stat(5700.0, "ms"), "5.7s");
    }
}
