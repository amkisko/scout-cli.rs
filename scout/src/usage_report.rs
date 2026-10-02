//! Usage report aggregation from throughput series.

use crate::concurrency::map_concurrent;
use crate::usage_cmd::{UsageOptions, UsageTimeframe};
use chrono::{TimeZone, Utc};
use scout_lib::{
    bucket_by_day, calculate_transactions, series_points, split_timeframe, Client, Error,
};
use serde_json::{json, Value};

pub async fn usage_totals(
    client: &Client,
    options: &UsageOptions,
    timeframe: &UsageTimeframe,
    filter_id: Option<u64>,
    concurrency: usize,
) -> Result<Value, Error> {
    let apps = filter_apps(client.list_apps(None).await?, filter_id)?;
    let chunks = split_timeframe(&timeframe.from, &timeframe.to).map_err(Error::Other)?;
    let work: Vec<(u64, String)> = apps
        .into_iter()
        .filter_map(|app| {
            let id = app.get("id").and_then(|v| v.as_u64())?;
            let name = app
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some((id, name))
        })
        .collect();

    let client = client.clone();
    let chunks = chunks.clone();
    let include_zero = options.all;
    let rows = map_concurrent(work, concurrency, move |(id, name)| {
        let client = client.clone();
        let chunks = chunks.clone();
        async move {
            let transactions = fetch_app_transactions(&client, id, &chunks).await;
            (id, name, transactions)
        }
    })
    .await;

    let mut rows: Vec<Value> = rows
        .into_iter()
        .filter(|(_, _, transactions)| include_zero || *transactions > 0.0)
        .map(|(id, name, transactions)| {
            json!({
                "id": id,
                "name": name,
                "transactions": transactions,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        let ta = a
            .get("transactions")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let tb = b
            .get("transactions")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });
    let grand_total: f64 = rows
        .iter()
        .map(|r| {
            r.get("transactions")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        })
        .sum();
    let total_rows = rows.len();
    if let Some(limit) = options.limit {
        rows.truncate(limit as usize);
    }
    Ok(json!({
        "from": timeframe.from,
        "to": timeframe.to,
        "total_transactions": grand_total,
        "shown": rows.len(),
        "total_apps": total_rows,
        "concurrency": concurrency,
        "apps": rows,
    }))
}

pub async fn usage_by_day(
    client: &Client,
    options: &UsageOptions,
    timeframe: &UsageTimeframe,
    filter_id: Option<u64>,
    concurrency: usize,
) -> Result<Value, Error> {
    let apps = filter_apps(client.list_apps(None).await?, filter_id)?;
    let chunks = split_timeframe(&timeframe.from, &timeframe.to).map_err(Error::Other)?;

    if options.by_app || filter_id.is_some() {
        return usage_by_day_by_app(client, options, timeframe, &apps, &chunks, concurrency).await;
    }

    let work: Vec<u64> = apps
        .iter()
        .filter_map(|app| app.get("id").and_then(|v| v.as_u64()))
        .collect();
    let client = client.clone();
    let chunks = chunks.clone();
    let per_app = map_concurrent(work, concurrency, move |id| {
        let client = client.clone();
        let chunks = chunks.clone();
        async move { bucket_by_day(&fetch_app_points(&client, id, &chunks).await) }
    })
    .await;

    let mut day_totals: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for days in per_app {
        for (date, tx) in days {
            *day_totals.entry(date).or_default() += tx;
        }
    }
    let mut days: Vec<Value> = day_totals
        .into_iter()
        .map(|(date, transactions)| json!({"date": date, "transactions": transactions}))
        .collect();
    let total_rows = days.len();
    if let Some(limit) = options.limit {
        days.truncate(limit as usize);
    }
    Ok(json!({
        "from": timeframe.from,
        "to": timeframe.to,
        "shown": days.len(),
        "total_days": total_rows,
        "concurrency": concurrency,
        "days": days,
    }))
}

async fn usage_by_day_by_app(
    client: &Client,
    options: &UsageOptions,
    timeframe: &UsageTimeframe,
    apps: &[Value],
    chunks: &[(String, String)],
    concurrency: usize,
) -> Result<Value, Error> {
    let work: Vec<(u64, String)> = apps
        .iter()
        .filter_map(|app| {
            let id = app.get("id").and_then(|v| v.as_u64())?;
            let name = app
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some((id, name))
        })
        .collect();
    let client = client.clone();
    let chunks = chunks.to_vec();
    let include_zero = options.all;
    let per_app = map_concurrent(work, concurrency, move |(id, name)| {
        let client = client.clone();
        let chunks = chunks.clone();
        async move {
            let points = fetch_app_points(&client, id, &chunks).await;
            let mut rows = Vec::new();
            for (date, transactions) in bucket_by_day(&points) {
                if !include_zero && transactions <= 0 {
                    continue;
                }
                let top_endpoint = top_endpoint_for_day(&client, id, &date).await;
                rows.push((date, id, name.clone(), transactions, top_endpoint));
            }
            rows
        }
    })
    .await;

    let mut by_date: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for rows in per_app {
        for (date, id, name, transactions, top_endpoint) in rows {
            by_date.entry(date).or_default().push(json!({
                "app_id": id,
                "app_name": name,
                "transactions": transactions,
                "top_endpoint": top_endpoint,
            }));
        }
    }
    let mut reports: Vec<Value> = by_date
        .into_iter()
        .map(|(date, mut apps_for_day)| {
            apps_for_day.sort_by(|a, b| {
                let ta = a.get("transactions").and_then(|v| v.as_i64()).unwrap_or(0);
                let tb = b.get("transactions").and_then(|v| v.as_i64()).unwrap_or(0);
                tb.cmp(&ta)
            });
            let total: i64 = apps_for_day
                .iter()
                .map(|a| a.get("transactions").and_then(|v| v.as_i64()).unwrap_or(0))
                .sum();
            json!({"date": date, "total": total, "apps": apps_for_day})
        })
        .collect();
    let total_rows = reports.len();
    if let Some(limit) = options.limit {
        reports.truncate(limit as usize);
    }
    Ok(json!({
        "from": timeframe.from,
        "to": timeframe.to,
        "shown": reports.len(),
        "total_days": total_rows,
        "concurrency": concurrency,
        "days": reports,
    }))
}

fn filter_apps(mut apps: Vec<Value>, filter_id: Option<u64>) -> Result<Vec<Value>, Error> {
    if let Some(id) = filter_id {
        apps.retain(|app| app.get("id").and_then(|v| v.as_u64()) == Some(id));
        if apps.is_empty() {
            return Err(Error::Other(format!("app {id} not found in this account")));
        }
    }
    Ok(apps)
}

async fn top_endpoint_for_day(client: &Client, app_id: u64, date: &str) -> String {
    let Ok(day) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return String::new();
    };
    let start = Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap());
    let end = Utc.from_utc_datetime(&day.and_hms_opt(23, 59, 59).unwrap());
    let from = scout_lib::format_utc_time(start);
    let to = scout_lib::format_utc_time(end);
    let Ok(data) = client
        .list_endpoints(
            app_id,
            Some(&from),
            Some(&to),
            None,
            Some("throughput"),
            Some(1),
            Some(0),
        )
        .await
    else {
        return String::new();
    };
    data.pointer("/results/endpoints/0/name")
        .or_else(|| data.pointer("/0/name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

async fn fetch_app_points(
    client: &Client,
    app_id: u64,
    chunks: &[(String, String)],
) -> Vec<(String, f64)> {
    let mut all = Vec::new();
    for (from, to) in chunks {
        if let Ok(series) = client
            .get_metric(app_id, "throughput", Some(from), Some(to), None)
            .await
        {
            all.extend(series_points(&series));
        }
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

async fn fetch_app_transactions(client: &Client, app_id: u64, chunks: &[(String, String)]) -> f64 {
    calculate_transactions(&fetch_app_points(client, app_id, chunks).await)
}
