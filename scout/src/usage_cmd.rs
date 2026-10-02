//! `scout usage` — web transaction estimates from throughput.

use crate::cli::AppIdArgs;
use crate::concurrency::resolve_concurrency;
use crate::output::{self, OutputMode};
use crate::usage_report::{usage_by_day, usage_totals};
use chrono::Utc;
use scout_lib::{clamp_billing_end, resolve_app_target, resolve_timeframe, Client, Error};
use serde_json::{json, Value};

pub struct UsageOptions {
    pub app: AppIdArgs,
    pub from: Option<String>,
    pub to: Option<String>,
    pub range: Option<String>,
    pub all: bool,
    pub by_day: bool,
    pub by_app: bool,
    pub billing_period: bool,
    pub limit: Option<u32>,
    /// Requested parallel app fetches; resolved via `resolve_concurrency`.
    pub concurrency: Option<u32>,
    pub app_id_override: Option<u64>,
    pub quiet: bool,
    pub mode: OutputMode,
}

pub struct UsageTimeframe {
    pub from: String,
    pub to: String,
    pub billing: Option<Value>,
}

pub async fn run(client: &Client, mut options: UsageOptions) -> Result<(), Error> {
    let concurrency = resolve_concurrency(options.concurrency).map_err(Error::Other)?;
    options.concurrency = Some(concurrency as u32);

    let timeframe = resolve_usage_timeframe(client, &options).await?;
    let filter_id = resolve_optional_app(client, &options).await?;

    let value = if options.by_day {
        usage_by_day(client, &options, &timeframe, filter_id, concurrency).await?
    } else {
        usage_totals(client, &options, &timeframe, filter_id, concurrency).await?
    };

    let value = wrap_billing_json(&timeframe, value);
    output::emit_value(options.mode, &value).map_err(Error::Other)
}

async fn resolve_usage_timeframe(
    client: &Client,
    options: &UsageOptions,
) -> Result<UsageTimeframe, Error> {
    if options.billing_period {
        if options.from.is_some() || options.to.is_some() || options.range.is_some() {
            return Err(Error::Other(
                "--billing-period cannot be used with --from, --to, or --range".to_string(),
            ));
        }
        let billing = client.get_org_usage().await?;
        let start = billing
            .pointer("/billing_period/start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Other("billing period not available for this organization".into())
            })?;
        let end = billing
            .pointer("/billing_period/end")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Other("billing period not available for this organization".into())
            })?;
        let start_t = scout_lib::parse_instant(start, Utc::now()).map_err(Error::Other)?;
        let end_t = clamp_billing_end(
            scout_lib::parse_instant(end, Utc::now()).map_err(Error::Other)?,
            Utc::now(),
        );
        return Ok(UsageTimeframe {
            from: scout_lib::format_utc_time(start_t),
            to: scout_lib::format_utc_time(end_t),
            billing: Some(billing),
        });
    }

    let (from, to) = if let Some(range) = options.range.as_deref() {
        scout_lib::calculate_range_at(range, options.to.as_deref(), Utc::now())
            .map_err(Error::Other)?
    } else {
        resolve_timeframe(options.from.as_deref(), options.to.as_deref(), Utc::now())
            .map_err(Error::Other)?
    };
    Ok(UsageTimeframe {
        from,
        to,
        billing: None,
    })
}

async fn resolve_optional_app(
    client: &Client,
    options: &UsageOptions,
) -> Result<Option<u64>, Error> {
    let app_ref = options.app.as_ref_str();
    if app_ref.is_none() && options.app_id_override.is_none() {
        return Ok(None);
    }
    if scout_lib::app_ref_needs_list(app_ref, options.app_id_override) {
        let apps = client.list_apps(None).await?;
        return resolve_app_target(app_ref, options.app_id_override, Some(&apps))
            .map(Some)
            .map_err(Error::Other);
    }
    resolve_app_target(app_ref, options.app_id_override, None)
        .map(Some)
        .map_err(Error::Other)
}

fn wrap_billing_json(timeframe: &UsageTimeframe, usage: Value) -> Value {
    let Some(billing) = timeframe.billing.as_ref() else {
        return usage;
    };
    let mut wrapped = json!({
        "billing_period": billing.get("billing_period").cloned().unwrap_or(Value::Null),
        "usage": usage,
    });
    if let Some(apm) = billing.get("apm") {
        let mut server_total = json!({
            "transactions": apm.get("total_transactions").cloned().unwrap_or(Value::Null),
        });
        if let Some(limit) = apm.get("limit") {
            server_total
                .as_object_mut()
                .unwrap()
                .insert("limit".to_string(), limit.clone());
        }
        wrapped
            .as_object_mut()
            .unwrap()
            .insert("server_total".to_string(), server_total);
    }
    wrapped
}
