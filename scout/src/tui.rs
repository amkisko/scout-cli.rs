//! Interactive TUI: app-scoped view with breadcrumbs and tabs (Endpoints, Insights, Metrics, Errors).

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use scout_lib::{format_timestamp_display, helpers::calculate_range, Client};
use serde_json::Value;
use std::collections::HashSet;
use std::io;
use std::time::Instant;
use tokio::runtime::Runtime;

/// TUI options (from --app, --tab, --refresh, --utc).
#[derive(Clone)]
pub struct Options {
    pub app: Option<String>,
    pub tab: Tab,
    pub refresh_secs: u64,
    /// If true, show timestamps in UTC; otherwise use local timezone.
    pub use_utc: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Endpoints,
    Insights,
    Metrics,
    Errors,
}

/// Restores terminal (leave alternate screen, disable raw mode, show cursor) on drop.
/// Ensures cleanup on any return path or panic so the user's terminal is never left stuck.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Drill-down content: preformatted text (endpoint/insight/error table) or raw metric series (formatted at draw time with terminal width).
#[derive(Clone)]
pub enum DrillContent {
    Preformatted(String),
    MetricSeries(Value),
}

impl Tab {
    fn as_str(self) -> &'static str {
        match self {
            Tab::Endpoints => "Endpoints",
            Tab::Insights => "Insights",
            Tab::Metrics => "Metrics",
            Tab::Errors => "Errors",
        }
    }
    fn all() -> [Tab; 4] {
        [Tab::Endpoints, Tab::Insights, Tab::Metrics, Tab::Errors]
    }
}

/// Indices into `apps` whose name or id contains `query` (case-insensitive). Empty query = all.
fn filtered_app_indices(apps: &[Value], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..apps.len()).collect();
    }
    apps
        .iter()
        .enumerate()
        .filter(|(_, app)| {
            let name = app
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let id = app
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            name.contains(&q) || id.contains(&q)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Resolve --app (id or name) to (index, app_id, app_name). Returns None if not found.
fn resolve_app(apps: &[Value], app_arg: &str) -> Option<(usize, u64, String)> {
    let app_arg = app_arg.trim();
    if app_arg.is_empty() {
        return None;
    }
    let (index, app) = if let Ok(id) = app_arg.parse::<u64>() {
        let i = apps
            .iter()
            .position(|a| a.get("id").and_then(|v| v.as_u64()) == Some(id))?;
        (i, &apps[i])
    } else {
        let lower = app_arg.to_lowercase();
        let i = apps.iter().position(|a| {
            a.get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.to_lowercase() == lower)
                == Some(true)
        })?;
        (i, &apps[i])
    };
    let app_id = app.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    let name = app
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    Some((index, app_id, name))
}

async fn run_async<F, T>(f: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || -> Result<T, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(f)
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn fetch_endpoints(client: &Client, app_id: u64) -> Result<Value, String> {
    let client = client.clone();
    run_async(async move {
        client
            .list_endpoints(app_id, None, None, Some("7days"))
            .await
            .map_err(|e| e.to_string())
    })
    .await
}

async fn fetch_insights(client: &Client, app_id: u64) -> Result<Value, String> {
    let client = client.clone();
    run_async(async move {
        client
            .get_all_insights(app_id, Some(50))
            .await
            .map_err(|e| e.to_string())
    })
    .await
}

async fn fetch_metrics_list(client: &Client, app_id: u64) -> Result<Vec<String>, String> {
    let client = client.clone();
    run_async(async move { client.list_metrics(app_id).await.map_err(|e| e.to_string()) }).await
}

async fn fetch_metric_series(
    client: &Client,
    app_id: u64,
    metric_type: &str,
) -> Result<Value, String> {
    let client = client.clone();
    let metric_type = metric_type.to_string();
    run_async(async move {
        client
            .get_metric(app_id, &metric_type, None, None, Some("7days"))
            .await
            .map_err(|e| e.to_string())
    })
    .await
}

async fn fetch_errors(client: &Client, app_id: u64) -> Result<Vec<Value>, String> {
    let client = client.clone();
    let (from, to) = calculate_range("7days", None).map_err(|e| e.to_string())?;
    run_async(async move {
        client
            .list_error_groups(app_id, Some(&from), Some(&to), None)
            .await
            .map_err(|e| e.to_string())
    })
    .await
}

/// Format an endpoint (or any object) as a key-value table.
fn format_endpoint_table(v: &Value) -> String {
    let mut rows = Vec::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            let val_str = match val {
                Value::Null => "—".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                Value::Array(a) => format!("[{} items]", a.len()),
                Value::Object(_) => "{…}".to_string(),
            };
            rows.push((k.as_str(), val_str));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(b.0));
    let max_key = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0).min(24);
    let mut out = String::new();
    for (k, val) in rows {
        let key = if k.len() > max_key { &k[..max_key] } else { k };
        out.push_str(&format!(
            "  {:<width$}  {}\n",
            key,
            val.replace('\n', " "),
            width = max_key
        ));
    }
    if out.is_empty() {
        out = "  (no data)".to_string();
    }
    out
}

fn collect_series_points(v: &Value, points: &mut Vec<(String, f64)>) {
    if let Some(arr) = v.as_array() {
        for p in arr {
            if let Some((ts, val)) = p.get(0).and_then(|t| t.as_str()).zip(
                p.get(1)
                    .and_then(|v| v.as_f64().or_else(|| v.as_u64().map(|u| u as f64))),
            ) {
                points.push((ts.to_string(), val));
            } else if let (Some(ts), Some(val)) = (
                p.get("timestamp")
                    .or_else(|| p.get("time"))
                    .and_then(|t| t.as_str()),
                p.get("value")
                    .and_then(|v| v.as_f64().or_else(|| v.as_u64().map(|u| u as f64))),
            ) {
                points.push((ts.to_string(), val));
            }
        }
        return;
    }
    if let Some(obj) = v.as_object() {
        if let Some(arr) = obj
            .get("points")
            .or_else(|| obj.get("data"))
            .and_then(|a| a.as_array())
        {
            for p in arr {
                if let Some((ts, val)) = p.get(0).and_then(|t| t.as_str()).zip(
                    p.get(1)
                        .and_then(|v| v.as_f64().or_else(|| v.as_u64().map(|u| u as f64))),
                ) {
                    points.push((ts.to_string(), val));
                } else if let (Some(ts), Some(val)) = (
                    p.get("timestamp")
                        .or_else(|| p.get("time"))
                        .and_then(|t| t.as_str()),
                    p.get("value")
                        .and_then(|v| v.as_f64().or_else(|| v.as_u64().map(|u| u as f64))),
                ) {
                    points.push((ts.to_string(), val));
                }
            }
            return;
        }
        // Nested: e.g. { "response_time": { "points": [...] } } — use first child
        for (_, child) in obj {
            collect_series_points(child, points);
            if !points.is_empty() {
                return;
            }
        }
    }
}

/// Unit for display per metric type (e.g. "ms", "RPM").
fn metric_unit(metric_type: &str) -> &'static str {
    match metric_type.trim().to_lowercase().as_str() {
        "throughput" => "RPM",
        "response_time" | "response_time_95th" | "queue_time" => "ms",
        "apdex" => "", // 0–1, unitless
        "errors" => "count",
        _ => "",
    }
}

/// Format metric series as a table and optional ASCII sparkline. `content_width` is the terminal content area width; `use_utc` forces UTC else local TZ. Sorted by time desc (latest on top). `metric_type` is used for the value column unit (e.g. "Value (ms)").
fn format_metric_series(
    v: &Value,
    content_width: usize,
    use_utc: bool,
    metric_type: Option<&str>,
) -> String {
    let mut points: Vec<(String, f64)> = Vec::new();
    collect_series_points(v, &mut points);
    points.sort_by(|a, b| b.0.cmp(&a.0)); // desc by time (latest first)
    if points.is_empty() {
        return "  No time-series points in response.\n  (Raw structure may differ; check API docs.)".to_string();
    }
    let mut out = String::new();
    let vals: Vec<f64> = points.iter().map(|(_, v)| *v).collect();
    let (min_v, max_v) = vals
        .iter()
        .cloned()
        .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    let range = max_v - min_v;
    let spark_width = content_width.saturating_sub(4).clamp(10, 80);
    if range > 0.0 && points.len() > 1 {
        let spark: String = vals
            .iter()
            .map(|&v| {
                let i = ((v - min_v) / range * (spark_width - 1) as f64).round() as usize;
                let i = i.min(spark_width - 1);
                "▁▂▃▄▅▆▇█"
                    .chars()
                    .nth((i * 8) / spark_width.max(1))
                    .unwrap_or('·')
            })
            .collect();
        out.push_str(&format!(
            "  Sparkline ({} pts):\n  {}\n\n",
            points.len(),
            spark
        ));
    }
    let time_w = (content_width / 2).saturating_sub(4).max(10);
    let sep = "  ";
    let time_header = if use_utc {
        "Time (UTC)"
    } else {
        "Time (local)"
    };
    let unit = metric_type.map(metric_unit).unwrap_or("");
    let value_header = if unit.is_empty() {
        "Value".to_string()
    } else {
        format!("Value ({})", unit)
    };
    out.push_str(&format!(
        "  {:<time_w$}  {}\n  ",
        time_header,
        value_header,
        time_w = time_w
    ));
    out.push_str(&"-".repeat(content_width.saturating_sub(2).max(10)));
    out.push('\n');
    for (ts, val) in points.iter().take(30) {
        let t = format_timestamp_display(ts, use_utc);
        let t = if t.len() > time_w {
            format!("…{}", &t[t.len().saturating_sub(time_w - 1)..])
        } else {
            t
        };
        let val_str = format!("{:.2}", val);
        out.push_str(&format!(
            "  {:<time_w$}{}{}\n",
            t,
            sep,
            val_str,
            time_w = time_w
        ));
    }
    if points.len() > 30 {
        out.push_str(&format!("  … and {} more\n", points.len() - 30));
    }
    out
}

/// Extract a sortable time string from a Value (ISO 8601 or similar). Tries common field names.
fn time_sort_key(v: &Value) -> String {
    v.get("last_seen")
        .or_else(|| v.get("first_seen"))
        .or_else(|| v.get("timestamp"))
        .or_else(|| v.get("created_at"))
        .or_else(|| v.get("time"))
        .or_else(|| v.get("reported_at"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

/// Extract endpoint list from API response (results may be object with "endpoints" or array). Sorted by time desc (latest on top).
fn endpoints_as_list(v: &Value) -> Vec<(String, Value)> {
    let arr = v
        .get("endpoints")
        .and_then(|a| a.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    let mut out: Vec<(String, Value)> = arr
        .into_iter()
        .map(|o| {
            let name = o
                .get("name")
                .or_else(|| o.get("transaction_name"))
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_string();
            (name, o)
        })
        .collect();
    out.sort_by(|a, b| time_sort_key(&b.1).cmp(&time_sort_key(&a.1)));
    out
}

/// Flatten insights result (may have n_plus_one, memory_bloat, slow_query arrays). Sorted by time desc (latest on top).
fn insights_as_list(v: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for (kind, arr) in obj {
            if let Some(arr) = arr.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    let label = item
                        .get("name")
                        .or_else(|| item.get("title"))
                        .and_then(|n| n.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| format!("{} #{}", kind, i + 1));
                    out.push((label, item.clone()));
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(arr) = v.as_array() {
            for (i, item) in arr.iter().enumerate() {
                let label = item
                    .get("name")
                    .or_else(|| item.get("title"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("Item {}", i + 1));
                out.push((label, item.clone()));
            }
        }
    }
    out.sort_by(|a, b| time_sort_key(&b.1).cmp(&time_sort_key(&a.1)));
    out
}

pub async fn run(client: &Client, opts: Options) -> Result<(), String> {
    let apps: Vec<Value> = client.list_apps(None).await.map_err(|e| e.to_string())?;

    let client = client.clone();
    crossterm::terminal::enable_raw_mode().map_err(|e| e.to_string())?;
    crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)
        .map_err(|e| e.to_string())?;
    let _guard = TerminalGuard;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))
            .map_err(|e| e.to_string())?;

    // If no --app, we need to show app picker first. Otherwise resolve app and go to app view.
    let mut current_app: Option<(u64, String)> = opts
        .app
        .as_ref()
        .and_then(|a| resolve_app(&apps, a).map(|(_, id, name)| (id, name)));
    let app_list = apps;
    let mut app_selected = opts
        .app
        .as_ref()
        .and_then(|a| resolve_app(&app_list, a))
        .map(|(i, _, _)| i)
        .unwrap_or(0);

    let mut tab = opts.tab;
    let mut breadcrumb: Vec<String> = current_app
        .as_ref()
        .map(|(_, name)| vec![name.clone()])
        .unwrap_or_else(|| vec!["Select app".to_string()]);
    let mut tab_data: TabData = TabData::default();
    let mut selected = 0usize;
    let mut drill: Option<DrillContent> = None; // detail view content (metric series formatted at draw time with width)
    let mut drill_label: Option<String> = None; // extra breadcrumb segment
    let mut loading_msg: Option<String> = None; // "Loading endpoints…" etc.
    let mut loaded_tabs: HashSet<(u64, Tab)> = HashSet::new(); // skip reload when switching back to a tab
    let refresh_secs = opts.refresh_secs;
    let mut last_refresh = Instant::now();
    let poll_timeout = std::time::Duration::from_millis(100);
    let search_debounce = std::time::Duration::from_millis(200);
    let mut app_search_pending = String::new();
    let mut app_search_committed = String::new();
    let mut app_search_last_typed: Option<Instant> = None;

    // If we have an app from --app, load default tab (Endpoints) immediately.
    if let Some((app_id, _)) = current_app {
        loading_msg = Some(format!("Loading {}…", tab.as_str().to_lowercase()));
        let (bc, tab_names, list_items, content_title, detail_text) = build_ui_state(
            current_app.as_ref(),
            &breadcrumb,
            tab,
            &tab_data,
            &app_list,
            app_search_committed.as_str(),
            app_selected,
            selected,
            drill_label.as_deref(),
            loading_msg.as_deref(),
            drill.is_some(),
        );
        terminal
            .draw(|f| {
                draw_ui(
                    f,
                    &bc,
                    tab,
                    &tab_names,
                    list_items,
                    content_title,
                    detail_text.as_deref(),
                    drill.as_ref(),
                    refresh_secs,
                    opts.use_utc,
                );
            })
            .map_err(|e| e.to_string())?;
        load_tab(&client, app_id, tab, &mut tab_data).await?;
        loaded_tabs.insert((app_id, tab));
        loading_msg = None;
        selected = selected.min(tab_data.list_len(tab).saturating_sub(1));
    }

    loop {
        // Apply search debounce when on project list: commit pending query after idle
        if current_app.is_none()
            && app_search_pending != app_search_committed
            && app_search_last_typed
                .map(|t| t.elapsed() >= search_debounce)
                .unwrap_or(true)
        {
            app_search_committed = app_search_pending.clone();
            let len = filtered_app_indices(&app_list, &app_search_committed).len();
            app_selected = app_selected.min(len.saturating_sub(1));
        }

        let (bc, tab_names, list_items, content_title, detail_text) = build_ui_state(
            current_app.as_ref(),
            &breadcrumb,
            tab,
            &tab_data,
            &app_list,
            app_search_committed.as_str(),
            app_selected,
            selected,
            drill_label.as_deref(),
            loading_msg.as_deref(),
            drill.is_some(),
        );

        let use_utc = opts.use_utc;
        terminal
            .draw(|f| {
                draw_ui(
                    f,
                    &bc,
                    tab,
                    &tab_names,
                    list_items,
                    content_title,
                    detail_text.as_deref(),
                    drill.as_ref(),
                    refresh_secs,
                    use_utc,
                );
            })
            .map_err(|e| e.to_string())?;

        let should_refresh = refresh_secs > 0
            && current_app.is_some()
            && last_refresh.elapsed() >= std::time::Duration::from_secs(refresh_secs);

        if crossterm::event::poll(poll_timeout).map_err(|e| e.to_string())? {
            if let crossterm::event::Event::Key(k) =
                crossterm::event::read().map_err(|e| e.to_string())?
            {
                use crossterm::event::KeyCode;
                match k.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            // Back to project list
                            current_app = None;
                            breadcrumb = vec!["Select app".to_string()];
                            tab = Tab::Endpoints;
                            selected = 0;
                            app_search_pending.clear();
                            app_search_committed.clear();
                            app_search_last_typed = None;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if loading_msg.is_some() {
                            // ignore while loading
                        } else if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            let tabs = Tab::all();
                            let i = tabs.iter().position(|&t| t == tab).unwrap_or(0);
                            let next = if i == 0 { tabs.len() - 1 } else { i - 1 };
                            tab = tabs[next];
                            if let Some((app_id, _)) = current_app {
                                if !loaded_tabs.contains(&(app_id, tab)) {
                                    loading_msg =
                                        Some(format!("Loading {}…", tab.as_str().to_lowercase()));
                                    let (bc, tab_names, list_items, content_title, detail_text) =
                                        build_ui_state(
                                            current_app.as_ref(),
                                            &breadcrumb,
                                            tab,
                                            &tab_data,
                                            &app_list,
                                            app_search_committed.as_str(),
                                            app_selected,
                                            selected,
                                            drill_label.as_deref(),
                                            loading_msg.as_deref(),
                                            drill.is_some(),
                                        );
                                    terminal
                                        .draw(|f| {
                                            draw_ui(
                                                f,
                                                &bc,
                                                tab,
                                                &tab_names,
                                                list_items,
                                                content_title,
                                                detail_text.as_deref(),
                                                drill.as_ref(),
                                                refresh_secs,
                                                opts.use_utc,
                                            );
                                        })
                                        .map_err(|e| e.to_string())?;
                                    load_tab(&client, app_id, tab, &mut tab_data).await?;
                                    loaded_tabs.insert((app_id, tab));
                                    loading_msg = None;
                                }
                            }
                            selected = 0;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if loading_msg.is_some() {
                            // ignore while loading
                        } else if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            let tabs = Tab::all();
                            let i = tabs.iter().position(|&t| t == tab).unwrap_or(0);
                            let next = (i + 1) % tabs.len();
                            tab = tabs[next];
                            if let Some((app_id, _)) = current_app {
                                if !loaded_tabs.contains(&(app_id, tab)) {
                                    loading_msg =
                                        Some(format!("Loading {}…", tab.as_str().to_lowercase()));
                                    let (bc, tab_names, list_items, content_title, detail_text) =
                                        build_ui_state(
                                            current_app.as_ref(),
                                            &breadcrumb,
                                            tab,
                                            &tab_data,
                                            &app_list,
                                            app_search_committed.as_str(),
                                            app_selected,
                                            selected,
                                            drill_label.as_deref(),
                                            loading_msg.as_deref(),
                                            drill.is_some(),
                                        );
                                    terminal
                                        .draw(|f| {
                                            draw_ui(
                                                f,
                                                &bc,
                                                tab,
                                                &tab_names,
                                                list_items,
                                                content_title,
                                                detail_text.as_deref(),
                                                drill.as_ref(),
                                                refresh_secs,
                                                opts.use_utc,
                                            );
                                        })
                                        .map_err(|e| e.to_string())?;
                                    load_tab(&client, app_id, tab, &mut tab_data).await?;
                                    loaded_tabs.insert((app_id, tab));
                                    loading_msg = None;
                                }
                            }
                            selected = 0;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if drill.is_none() {
                            if current_app.is_none() {
                                let max = filtered_app_indices(&app_list, &app_search_committed)
                                    .len()
                                    .saturating_sub(1);
                                app_selected = app_selected.saturating_sub(1).min(max);
                            } else {
                                let max = tab_data.list_len(tab).saturating_sub(1);
                                selected = selected.saturating_sub(1).min(max);
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if drill.is_none() {
                            if current_app.is_none() {
                                let max = filtered_app_indices(&app_list, &app_search_committed)
                                    .len()
                                    .saturating_sub(1);
                                app_selected = (app_selected + 1).min(max);
                            } else {
                                let max = tab_data.list_len(tab).saturating_sub(1);
                                selected = (selected + 1).min(max);
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if current_app.is_none() {
                            let indices = filtered_app_indices(&app_list, &app_search_committed);
                            if let Some(&idx) = indices.get(app_selected) {
                                let app = &app_list[idx];
                                let app_id = app.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                                let name = app
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?")
                                    .to_string();
                                current_app = Some((app_id, name.clone()));
                                breadcrumb = vec![name];
                                tab = Tab::Endpoints;
                                if let Some((id, _)) = current_app {
                                    loading_msg = Some("Loading endpoints…".to_string());
                                    let (bc, tab_names, list_items, content_title, detail_text) =
                                        build_ui_state(
                                            current_app.as_ref(),
                                            &breadcrumb,
                                            tab,
                                            &tab_data,
                                            &app_list,
                                            app_search_committed.as_str(),
                                            app_selected,
                                            selected,
                                            drill_label.as_deref(),
                                            loading_msg.as_deref(),
                                            drill.is_some(),
                                        );
                                    terminal
                                        .draw(|f| {
                                            draw_ui(
                                                f,
                                                &bc,
                                                tab,
                                                &tab_names,
                                                list_items,
                                                content_title,
                                                detail_text.as_deref(),
                                                drill.as_ref(),
                                                refresh_secs,
                                                opts.use_utc,
                                            );
                                        })
                                        .map_err(|e| e.to_string())?;
                                    load_tab(&client, id, tab, &mut tab_data).await?;
                                    loaded_tabs.insert((id, tab));
                                    loading_msg = None;
                                }
                                selected = 0;
                            }
                        } else if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if let Some((app_id, _)) = current_app {
                            if tab == Tab::Metrics {
                                if let Some(mt) = tab_data.get_metric_type(selected) {
                                    drill_label = Some(mt.to_string());
                                    loading_msg = Some(format!("Loading metric {}…", mt));
                                    let (bc, tab_names, list_items, content_title, detail_text) =
                                        build_ui_state(
                                            current_app.as_ref(),
                                            &breadcrumb,
                                            tab,
                                            &tab_data,
                                            &app_list,
                                            app_search_committed.as_str(),
                                            app_selected,
                                            selected,
                                            drill_label.as_deref(),
                                            loading_msg.as_deref(),
                                            drill.is_some(),
                                        );
                                    terminal
                                        .draw(|f| {
                                            draw_ui(
                                                f,
                                                &bc,
                                                tab,
                                                &tab_names,
                                                list_items,
                                                content_title,
                                                detail_text.as_deref(),
                                                drill.as_ref(),
                                                refresh_secs,
                                                opts.use_utc,
                                            );
                                        })
                                        .map_err(|e| e.to_string())?;
                                    match fetch_metric_series(&client, app_id, mt).await {
                                        Ok(v) => drill = Some(DrillContent::MetricSeries(v)),
                                        Err(e) => {
                                            drill = Some(DrillContent::Preformatted(format!(
                                                "Error: {}",
                                                e
                                            )))
                                        }
                                    }
                                    loading_msg = None;
                                }
                            } else if let Some((label, detail_value)) =
                                tab_data.get_item(tab, selected)
                            {
                                drill_label = Some(label.clone());
                                drill = Some(DrillContent::Preformatted(format_endpoint_table(
                                    &detail_value,
                                )));
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if current_app.is_none() {
                            app_search_pending.pop();
                            app_search_last_typed = Some(Instant::now());
                        }
                    }
                    KeyCode::Char(c) => {
                        // Only add to search when on app list; leave q/h/j/k/l for quit and navigation
                        if current_app.is_none()
                            && !['q', 'h', 'j', 'k', 'l'].contains(&c)
                        {
                            app_search_pending.push(c);
                            app_search_last_typed = Some(Instant::now());
                        }
                    }
                    _ => {}
                }
            }
        } else if should_refresh {
            last_refresh = Instant::now();
            if let Some((app_id, _)) = current_app {
                loading_msg = Some(format!("Refreshing {}…", tab.as_str().to_lowercase()));
                let (bc, tab_names, list_items, content_title, detail_text) = build_ui_state(
                    current_app.as_ref(),
                    &breadcrumb,
                    tab,
                    &tab_data,
                    &app_list,
                    app_search_committed.as_str(),
                    app_selected,
                    selected,
                    drill_label.as_deref(),
                    loading_msg.as_deref(),
                    drill.is_some(),
                );
                terminal
                    .draw(|f| {
                        draw_ui(
                            f,
                            &bc,
                            tab,
                            &tab_names,
                            list_items,
                            content_title,
                            detail_text.as_deref(),
                            drill.as_ref(),
                            refresh_secs,
                            opts.use_utc,
                        );
                    })
                    .map_err(|e| e.to_string())?;
                load_tab(&client, app_id, tab, &mut tab_data).await?;
                loading_msg = None;
                selected = selected.min(tab_data.list_len(tab).saturating_sub(1));
            }
        }
    }

    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)
        .map_err(|e| e.to_string())?;
    crossterm::terminal::disable_raw_mode().map_err(|e| e.to_string())?;
    terminal.show_cursor().map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Default)]
struct TabData {
    endpoints: Vec<(String, Value)>,
    insights: Vec<(String, Value)>,
    metrics: Vec<String>,
    errors: Vec<Value>,
}

impl TabData {
    /// Length of the list for the given tab only (scoped to active tab).
    fn list_len(&self, tab: Tab) -> usize {
        match tab {
            Tab::Endpoints => self.endpoints.len(),
            Tab::Insights => self.insights.len(),
            Tab::Metrics => self.metrics.len(),
            Tab::Errors => self.errors.len(),
        }
    }
    /// Item at index for the given tab only. Metrics tab has no Value items (use get_metric_type).
    fn get_item(&self, tab: Tab, index: usize) -> Option<(String, Value)> {
        match tab {
            Tab::Endpoints => self.endpoints.get(index).cloned(),
            Tab::Insights => self.insights.get(index).cloned(),
            Tab::Metrics => None,
            Tab::Errors => self
                .errors
                .get(index)
                .map(|v| (format!("Error #{}", index + 1), v.clone())),
        }
    }
    fn get_metric_type(&self, index: usize) -> Option<&str> {
        self.metrics.get(index).map(String::as_str)
    }
}

async fn load_tab(
    client: &Client,
    app_id: u64,
    tab: Tab,
    data: &mut TabData,
) -> Result<(), String> {
    match tab {
        Tab::Endpoints => {
            let v = fetch_endpoints(client, app_id).await?;
            data.endpoints = endpoints_as_list(&v);
        }
        Tab::Insights => {
            let v = fetch_insights(client, app_id).await?;
            data.insights = insights_as_list(&v);
        }
        Tab::Metrics => {
            data.metrics = fetch_metrics_list(client, app_id).await?;
        }
        Tab::Errors => {
            let mut errs = fetch_errors(client, app_id).await?;
            errs.sort_by_key(|b| std::cmp::Reverse(time_sort_key(b))); // desc (latest first)
            data.errors = errs;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_ui_state<'a>(
    current_app: Option<&(u64, String)>,
    breadcrumb: &[String],
    tab: Tab,
    tab_data: &TabData,
    app_list: &[Value],
    app_search: &str,
    app_selected: usize,
    selected: usize,
    drill_label: Option<&str>,
    loading_msg: Option<&str>,
    is_drill_view: bool,
) -> (
    Vec<String>,
    Vec<&'static str>,
    Vec<ListItem<'a>>,
    String,
    Option<String>,
) {
    let mut bc = breadcrumb.to_vec();
    if current_app.is_some() {
        if bc.len() == 1 {
            bc.push(tab.as_str().to_string());
        } else {
            bc.truncate(1);
            bc.push(tab.as_str().to_string());
        }
        if let Some(l) = drill_label {
            bc.push(l.to_string());
        }
    }
    let tab_names = Tab::all().iter().map(|t| t.as_str()).collect::<Vec<_>>();
    let (list_items, content_title, detail_text) = if let Some(msg) = loading_msg {
        let estimate = "Usually 1–3 seconds depending on network.";
        let text = format!("⟳  {}\n\n{}\n\nPlease wait…", msg, estimate);
        (Vec::new(), " Loading ".to_string(), Some(text))
    } else if current_app.is_none() {
        let indices = filtered_app_indices(app_list, app_search);
        let items: Vec<ListItem> = indices
            .iter()
            .enumerate()
            .map(|(i, &idx)| {
                let app = &app_list[idx];
                let name = app.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let id = app.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let style = if i == app_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(format!("{}  {}", id, name), style)))
            })
            .collect();
        let title = if app_search.is_empty() {
            " Select an app (Enter to open, type to search) ".to_string()
        } else {
            format!(
                " Select an app — filter: \"{}\" (Enter to open) ",
                app_search
            )
        };
        (items, title, None)
    } else if is_drill_view {
        let title = drill_label
            .map(|l| format!(" {} ", l))
            .unwrap_or_else(|| " Detail ".to_string());
        (Vec::new(), title, None) // drill content rendered in draw_ui from Option<DrillContent>
    } else {
        let items = match tab {
            Tab::Endpoints => tab_data
                .endpoints
                .iter()
                .enumerate()
                .map(|(i, (name, _))| {
                    let style = if i == selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(name.clone(), style)))
                })
                .collect(),
            Tab::Insights => tab_data
                .insights
                .iter()
                .enumerate()
                .map(|(i, (name, _))| {
                    let style = if i == selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(name.clone(), style)))
                })
                .collect(),
            Tab::Metrics => tab_data
                .metrics
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = if i == selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(name.clone(), style)))
                })
                .collect(),
            Tab::Errors => tab_data
                .errors
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let name = v
                        .get("message")
                        .or_else(|| v.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("?");
                    let style = if i == selected {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(name.to_string(), style)))
                })
                .collect(),
        };
        let title = format!(" {} ", tab.as_str());
        (items, title, None)
    };
    (bc, tab_names, list_items, content_title, detail_text)
}

#[allow(clippy::too_many_arguments)]
fn draw_ui(
    f: &mut Frame,
    breadcrumb: &[String],
    current_tab: Tab,
    tab_names: &[&str],
    list_items: Vec<ListItem>,
    content_title: String,
    detail_text: Option<&str>,
    drill: Option<&DrillContent>,
    _refresh_secs: u64,
    use_utc: bool,
) {
    let is_app_select = content_title.contains("Select an app");
    let has_project = breadcrumb.len() >= 2;
    let breadcrumb_height = if has_project { 2 } else { 1 };
    let vertical = if is_app_select {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(breadcrumb_height), Constraint::Min(1)])
            .split(f.size())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(breadcrumb_height),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(f.size())
    };
    let breadcrumb_area = vertical[0];
    let content_area = vertical[vertical.len() - 1];

    let breadcrumb_block = if has_project {
        let line0 = format!("App: {}", breadcrumb[0]);
        let line1 = breadcrumb[1..].join(" > ");
        Paragraph::new(vec![Line::from(line0), Line::from(line1)])
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::BOTTOM))
    } else {
        let bc_str = breadcrumb
            .first()
            .map(String::as_str)
            .unwrap_or("Select app");
        Paragraph::new(bc_str)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::BOTTOM))
    };
    f.render_widget(breadcrumb_block, breadcrumb_area);

    if !is_app_select && vertical.len() > 2 {
        let tab_line: Line = Line::from(
            tab_names
                .iter()
                .map(|&name| {
                    let active = name == current_tab.as_str();
                    Span::styled(
                        format!(" {} ", name),
                        if active {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Cyan)
                        },
                    )
                })
                .collect::<Vec<_>>(),
        );
        let tab_block = Paragraph::new(tab_line).block(Block::default());
        f.render_widget(tab_block, vertical[1]);
    }
    let is_loading = content_title.trim() == "Loading";
    let detail_str: Option<String> = match (detail_text, drill) {
        (Some(t), _) => Some(t.to_string()),
        (None, Some(DrillContent::Preformatted(s))) => Some(s.clone()),
        (None, Some(DrillContent::MetricSeries(v))) => Some(format_metric_series(
            v,
            content_area.width as usize,
            use_utc,
            Some(content_title.trim()),
        )),
        (None, None) => None,
    };
    if list_items.is_empty() && detail_str.is_none() {
        let empty = Paragraph::new("No data or select an item and press Enter.")
            .block(
                Block::default()
                    .title(content_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White));
        f.render_widget(empty, content_area);
    } else if let Some(text) = detail_str.as_deref() {
        let border_style = if is_loading {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let para = Paragraph::new(text)
            .block(
                Block::default()
                    .title(content_title)
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(if is_loading {
                Color::Yellow
            } else {
                Color::White
            }));
        f.render_widget(para, content_area);
    } else {
        let list = List::new(list_items).block(
            Block::default()
                .title(content_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(list, content_area);
    }
}
