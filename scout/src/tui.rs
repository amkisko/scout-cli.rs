//! Interactive TUI: app-scoped view with breadcrumbs and tabs (Endpoints, Insights, Metrics, Errors).

use ratatui::{
    layout::Alignment,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Bar, BarChart, Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame, Terminal,
};
use scout_lib::{format_timestamp_display, helpers::calculate_range, Client};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio::runtime::Runtime;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

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
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
    }
}

/// Drill-down content: preformatted text (endpoint/insight/error table) or raw metric series (formatted at draw time with terminal width).
#[derive(Clone)]
pub enum DrillContent {
    Preformatted(String),
    MetricSeries(Value),
}

enum TabPayload {
    Endpoints(Vec<(String, Value)>),
    Insights(Vec<(String, Value)>),
    Metrics(Vec<String>),
    Errors(Vec<Value>),
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
    apps.iter()
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

fn downsample_points(points: &[(String, f64)], max_count: usize) -> Vec<(String, f64)> {
    if points.len() <= max_count {
        return points.to_vec();
    }
    let step = points.len() as f64 / max_count as f64;
    (0..max_count)
        .map(|i| {
            let idx = ((i as f64 * step).floor() as usize).min(points.len() - 1);
            points[idx].clone()
        })
        .collect()
}

fn compact_time_label(ts: &str, use_utc: bool) -> String {
    let display = format_timestamp_display(ts, use_utc);
    let chars: Vec<char> = display.chars().collect();
    if chars.len() > 5 {
        chars[chars.len() - 5..].iter().collect()
    } else {
        display
    }
}

fn render_metric_chart(
    f: &mut Frame,
    content_area: ratatui::layout::Rect,
    v: &Value,
    use_utc: bool,
    metric_type: Option<&str>,
) {
    let mut points: Vec<(String, f64)> = Vec::new();
    collect_series_points(v, &mut points);
    points.sort_by(|a, b| a.0.cmp(&b.0)); // asc by time (oldest -> newest)

    if points.is_empty() {
        let empty = Paragraph::new("No time-series points in response.")
            .block(
                Block::default()
                    .title(" Metric chart ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White));
        f.render_widget(empty, content_area);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(2)])
        .split(content_area);
    let chart_area = vertical[0];
    let meta_area = vertical[1];

    let inner_w = chart_area.width.saturating_sub(2) as usize;
    let target_bars = (inner_w / 4).clamp(1, 32);
    let sampled = downsample_points(&points, target_bars);
    let max_v = sampled
        .iter()
        .map(|(_, v)| *v)
        .fold(0.0_f64, |a, b| a.max(b))
        .max(1.0);
    let min_v = sampled
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::MAX, |a, b| a.min(b));
    let latest_v = sampled.last().map(|(_, v)| *v).unwrap_or(0.0);

    let bar_width = if target_bars >= 24 {
        1
    } else if target_bars >= 12 {
        2
    } else {
        3
    };

    let bars: Vec<Bar> = sampled
        .iter()
        .map(|(ts, val)| {
            let scaled = ((*val / max_v) * 100.0).round().clamp(0.0, 100.0) as u64;
            Bar::with_label(compact_time_label(ts, use_utc), scaled)
                .style(Color::Cyan)
                .value_style((Color::Black, Color::Cyan))
                .text_value(format!("{:.1}", val))
        })
        .collect();

    let title = if let Some(mt) = metric_type {
        format!(" {} chart ", mt)
    } else {
        " Metric chart ".to_string()
    };
    let chart = BarChart::vertical(bars)
        .bar_width(bar_width)
        .bar_gap(1)
        .max(100)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(chart, chart_area);

    let unit = metric_type.map(metric_unit).unwrap_or("");
    let suffix = if unit.is_empty() {
        "".to_string()
    } else {
        format!(" {}", unit)
    };
    let meta = format!(
        "latest: {:.2}{}  min: {:.2}{}  max: {:.2}{}  points: {}",
        latest_v,
        suffix,
        min_v,
        suffix,
        max_v,
        suffix,
        points.len()
    );
    let meta_widget = Paragraph::new(meta).block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(meta_widget, meta_area);
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
    enable_raw_mode().map_err(|e| e.to_string())?;
    execute!(io::stdout(), EnterAlternateScreen, Hide).map_err(|e| e.to_string())?;
    let _guard = TerminalGuard;
    let mut terminal =
        Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout())).map_err(|e| e.to_string())?;

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
    let mut loaded_tabs: HashSet<(u64, Tab)> = HashSet::new(); // skip reload when switching back to a tab
    let mut tab_errors: HashMap<(u64, Tab), String> = HashMap::new();
    let mut pending_tab_loads: HashMap<(u64, Tab), JoinHandle<Result<TabPayload, String>>> =
        HashMap::new();
    let mut pending_metric_load: Option<(u64, String, JoinHandle<Result<Value, String>>)> = None;
    let refresh_secs = opts.refresh_secs;
    let mut last_refresh = Instant::now();
    let spinner_started = Instant::now();
    let poll_timeout = std::time::Duration::from_millis(100);
    let search_debounce = std::time::Duration::from_millis(200);
    let mut app_search_pending = String::new();
    let mut app_search_committed = String::new();
    let mut app_search_last_typed: Option<Instant> = None;

    // If we have an app from --app, start loading initial tab in background.
    if let Some((app_id, _)) = current_app {
        start_tab_load(&mut pending_tab_loads, &client, app_id, tab);
    }

    loop {
        // Apply completed tab loads.
        let finished_tab_keys: Vec<(u64, Tab)> = pending_tab_loads
            .iter()
            .filter_map(|(k, h)| if h.is_finished() { Some(*k) } else { None })
            .collect();
        for key in finished_tab_keys {
            if let Some(handle) = pending_tab_loads.remove(&key) {
                match handle.await {
                    Ok(Ok(payload)) => {
                        let is_current_app =
                            current_app.as_ref().map(|(id, _)| *id) == Some(key.0);
                        if is_current_app {
                            apply_tab_payload(&mut tab_data, key.1, payload);
                            loaded_tabs.insert(key);
                            tab_errors.remove(&key);
                            if key.1 == tab {
                                selected = selected.min(tab_data.list_len(tab).saturating_sub(1));
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        if current_app.as_ref().map(|(id, _)| *id) == Some(key.0) {
                            tab_errors.insert(key, e);
                        }
                    }
                    Err(e) => {
                        if current_app.as_ref().map(|(id, _)| *id) == Some(key.0) {
                            tab_errors.insert(key, e.to_string());
                        }
                    }
                }
            }
        }

        // Apply completed metric drill load.
        if pending_metric_load
            .as_ref()
            .map(|(_, _, h)| h.is_finished())
            .unwrap_or(false)
        {
            if let Some((_, metric_name, handle)) = pending_metric_load.take() {
                match handle.await {
                    Ok(Ok(v)) => drill = Some(DrillContent::MetricSeries(v)),
                    Ok(Err(e)) => {
                        drill = Some(DrillContent::Preformatted(format!("Error: {}", e)));
                    }
                    Err(e) => {
                        drill = Some(DrillContent::Preformatted(format!("Error: {}", e)));
                    }
                }
                if drill_label.is_none() {
                    drill_label = Some(metric_name);
                }
            }
        }

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

        let loading_msg = if let Some((app_id, _)) = current_app {
            if let Some((metric_app_id, metric_name, _)) = pending_metric_load.as_ref() {
                if *metric_app_id == app_id {
                    Some(format!("Loading metric {}…", metric_name))
                } else {
                    None
                }
            } else if pending_tab_loads.contains_key(&(app_id, tab))
                && tab_data.list_len(tab) == 0
                && drill.is_none()
            {
                Some(format!("Loading {}…", tab.as_str().to_lowercase()))
            } else if tab_data.list_len(tab) == 0 && drill.is_none() {
                tab_errors.get(&(app_id, tab)).map(|e| format!("Error: {}", e))
            } else {
                None
            }
        } else {
            None
        };

        // Safety net: if current tab was not kicked off by a keypath, start it here.
        if let Some((app_id, _)) = current_app {
            if !loaded_tabs.contains(&(app_id, tab))
                && !pending_tab_loads.contains_key(&(app_id, tab))
                && pending_metric_load.is_none()
            {
                start_tab_load(&mut pending_tab_loads, &client, app_id, tab);
            }
        }

        let pending_count = pending_tab_loads.len() + usize::from(pending_metric_load.is_some());
        let loading_indicator = if pending_count > 0 {
            let frames = ["◐", "◓", "◑", "◒"];
            let idx =
                ((spinner_started.elapsed().as_millis() / 140) % frames.len() as u128) as usize;
            Some(format!("{} {}", frames[idx], pending_count))
        } else {
            None
        };

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
                    if current_app.is_none() {
                        Some(app_selected)
                    } else {
                        Some(selected)
                    },
                    loading_indicator.as_deref(),
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

        if event::poll(poll_timeout).map_err(|e| e.to_string())? {
            if let Event::Key(k) = event::read().map_err(|e| e.to_string())? {
                match k.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            // Back to project list
                            if let Some((_, _, h)) = pending_metric_load.take() {
                                h.abort();
                            }
                            for (_, h) in pending_tab_loads.drain() {
                                h.abort();
                            }
                            tab_data = TabData::default();
                            loaded_tabs.clear();
                            tab_errors.clear();
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
                        if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            let tabs = Tab::all();
                            let i = tabs.iter().position(|&t| t == tab).unwrap_or(0);
                            let next = if i == 0 { tabs.len() - 1 } else { i - 1 };
                            tab = tabs[next];
                            if let Some((app_id, _)) = current_app {
                                if !loaded_tabs.contains(&(app_id, tab))
                                    && !pending_tab_loads.contains_key(&(app_id, tab))
                                {
                                    start_tab_load(&mut pending_tab_loads, &client, app_id, tab);
                                }
                            }
                            selected = 0;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if current_app.is_some() {
                            let tabs = Tab::all();
                            let i = tabs.iter().position(|&t| t == tab).unwrap_or(0);
                            let next = (i + 1) % tabs.len();
                            tab = tabs[next];
                            if let Some((app_id, _)) = current_app {
                                if !loaded_tabs.contains(&(app_id, tab))
                                    && !pending_tab_loads.contains_key(&(app_id, tab))
                                {
                                    start_tab_load(&mut pending_tab_loads, &client, app_id, tab);
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
                                tab_data = TabData::default();
                                loaded_tabs.clear();
                                tab_errors.clear();
                                if let Some((_, _, h)) = pending_metric_load.take() {
                                    h.abort();
                                }
                                for (_, h) in pending_tab_loads.drain() {
                                    h.abort();
                                }
                                if let Some((id, _)) = current_app {
                                    if !loaded_tabs.contains(&(id, tab))
                                        && !pending_tab_loads.contains_key(&(id, tab))
                                    {
                                        start_tab_load(&mut pending_tab_loads, &client, id, tab);
                                    }
                                }
                                selected = 0;
                            }
                        } else if drill.is_some() {
                            drill = None;
                            drill_label = None;
                        } else if let Some((app_id, _)) = current_app {
                            if tab == Tab::Metrics {
                                if let Some(mt) = tab_data.get_metric_type(selected) {
                                    if let Some((_, _, old_handle)) = pending_metric_load.take() {
                                        old_handle.abort();
                                    }
                                    let metric_name = mt.to_string();
                                    drill_label = Some(mt.to_string());
                                    drill = Some(DrillContent::Preformatted(format!(
                                        "Loading metric {}…",
                                        metric_name
                                    )));
                                    let client_clone = client.clone();
                                    let metric_clone = metric_name.clone();
                                    let handle = tokio::spawn(async move {
                                        fetch_metric_series(&client_clone, app_id, &metric_clone)
                                            .await
                                    });
                                    pending_metric_load = Some((app_id, metric_name, handle));
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
                        if current_app.is_none() && !['q', 'h', 'j', 'k', 'l'].contains(&c) {
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
                if !pending_tab_loads.contains_key(&(app_id, tab)) {
                    start_tab_load(&mut pending_tab_loads, &client, app_id, tab);
                }
            }
        }
    }

    execute!(io::stdout(), LeaveAlternateScreen, Show).map_err(|e| e.to_string())?;
    disable_raw_mode().map_err(|e| e.to_string())?;
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

async fn fetch_tab_payload(client: &Client, app_id: u64, tab: Tab) -> Result<TabPayload, String> {
    match tab {
        Tab::Endpoints => {
            let v = fetch_endpoints(client, app_id).await?;
            Ok(TabPayload::Endpoints(endpoints_as_list(&v)))
        }
        Tab::Insights => {
            let v = fetch_insights(client, app_id).await?;
            Ok(TabPayload::Insights(insights_as_list(&v)))
        }
        Tab::Metrics => Ok(TabPayload::Metrics(fetch_metrics_list(client, app_id).await?)),
        Tab::Errors => {
            let mut errs = fetch_errors(client, app_id).await?;
            errs.sort_by_key(|b| std::cmp::Reverse(time_sort_key(b))); // desc (latest first)
            Ok(TabPayload::Errors(errs))
        }
    }
}

fn apply_tab_payload(data: &mut TabData, tab: Tab, payload: TabPayload) {
    match (tab, payload) {
        (Tab::Endpoints, TabPayload::Endpoints(v)) => data.endpoints = v,
        (Tab::Insights, TabPayload::Insights(v)) => data.insights = v,
        (Tab::Metrics, TabPayload::Metrics(v)) => data.metrics = v,
        (Tab::Errors, TabPayload::Errors(v)) => data.errors = v,
        _ => {}
    }
}

fn start_tab_load(
    pending: &mut HashMap<(u64, Tab), JoinHandle<Result<TabPayload, String>>>,
    client: &Client,
    app_id: u64,
    tab: Tab,
) {
    if pending.contains_key(&(app_id, tab)) {
        return;
    }
    let client_clone = client.clone();
    let handle = tokio::spawn(async move { fetch_tab_payload(&client_clone, app_id, tab).await });
    pending.insert((app_id, tab), handle);
}

#[allow(clippy::too_many_arguments)]
fn build_ui_state<'a>(
    current_app: Option<&(u64, String)>,
    breadcrumb: &[String],
    tab: Tab,
    tab_data: &TabData,
    app_list: &[Value],
    app_search: &str,
    _app_selected: usize,
    _selected: usize,
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
        if msg.starts_with("Error:") {
            (Vec::new(), " Error ".to_string(), Some(msg.to_string()))
        } else {
            let estimate = "Usually 1–3 seconds depending on network.";
            let text = format!("⟳  {}\n\n{}\n\nPlease wait…", msg, estimate);
            (Vec::new(), " Loading ".to_string(), Some(text))
        }
    } else if current_app.is_none() {
        let indices = filtered_app_indices(app_list, app_search);
        let items: Vec<ListItem> = indices
            .iter()
            .enumerate()
            .map(|(_, &idx)| {
                let app = &app_list[idx];
                let name = app.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let id = app.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                ListItem::new(Line::from(format!("{}  {}", id, name)))
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
                .map(|(_, (name, _))| ListItem::new(Line::from(name.clone())))
                .collect(),
            Tab::Insights => tab_data
                .insights
                .iter()
                .enumerate()
                .map(|(_, (name, _))| ListItem::new(Line::from(name.clone())))
                .collect(),
            Tab::Metrics => tab_data
                .metrics
                .iter()
                .enumerate()
                .map(|(_, name)| ListItem::new(Line::from(name.clone())))
                .collect(),
            Tab::Errors => tab_data
                .errors
                .iter()
                .enumerate()
                .map(|(_, v)| {
                    let name = v
                        .get("message")
                        .or_else(|| v.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("?");
                    ListItem::new(Line::from(name.to_string()))
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
    list_selected: Option<usize>,
    loading_indicator: Option<&str>,
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
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(breadcrumb_height),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(f.area())
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
    if let Some(indicator) = loading_indicator {
        let indicator_widget = Paragraph::new(indicator)
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(indicator_widget, breadcrumb_area);
    }

    if !is_app_select && vertical.len() > 2 {
        let tab_index = tab_names
            .iter()
            .position(|&name| name == current_tab.as_str())
            .unwrap_or(0);
        let tabs = Tabs::new(tab_names.iter().map(|name| Line::from(format!(" {} ", name))))
            .select(tab_index)
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(tabs, vertical[1]);
    }
    let is_loading = content_title.trim() == "Loading";
    let detail_str: Option<String> = match (detail_text, drill) {
        (Some(t), _) => Some(t.to_string()),
        (None, Some(DrillContent::Preformatted(s))) => Some(s.clone()),
        (None, Some(DrillContent::MetricSeries(_))) => None,
        (None, None) => None,
    };
    if let Some(DrillContent::MetricSeries(v)) = drill {
        render_metric_chart(f, content_area, v, use_utc, Some(content_title.trim()));
    } else if list_items.is_empty() && detail_str.is_none() {
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
            .style(Style::default().fg(if is_loading {
                Color::Yellow
            } else {
                Color::White
            }));
        f.render_widget(para, content_area);
    } else {
        let list = List::new(list_items)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .block(
                Block::default()
                    .title(content_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        let mut state = ListState::default();
        state.select(list_selected);
        f.render_stateful_widget(list, content_area, &mut state);
    }
}
