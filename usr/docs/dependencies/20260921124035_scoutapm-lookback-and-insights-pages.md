# ScoutAPM lookback caps and insights history page size

## Dependency

ScoutAPM REST API at https://scoutapm.com/api/v0 (this repo's default api_base). No lockfile pin; HTTP JSON.

## Symptom

Exact `--range 30days` (30 * 86400 seconds before now) was refused for anomalies with "older than 30 days". Errors on a 30-day lookback previously returned "from date cannot be older than 7 days" even though a 29-day errors window succeeded. Traces OpenAPI documents 7 days; exact 7 * 86400 was refused. Insights history `--limit 50` was reported to fail on page 4 with error decoding response body; `--limit 20` completed the history walk.

## Evidence

Live ScoutAPM app on 2026-09-21. `--range 30days` archive pull of metrics, errors, and anomalies with one hour of lookback slack: empty refusals list, three chunks. `insights-history --limit 20` page 1: ~3MB. `--limit 50` page 1: ~4MB, success on this client. `--limit 50 --pagination-page 4` without pagination_cursor: API error "Must specify pagination_cursor for pages after the first" (exit 4). Decode error on page 4 was not reproduced; this client reads the body as text then JSON, and invalid JSON becomes Null rather than that reqwest message.

## Suggested fix

Keep client clamps: errors and anomalies max lookback 30 days minus 3600 seconds; traces 7 days minus 3600 seconds; request span 14 days. Archive insights history pages at 20. Upstream: document real lookback per resource (errors message vs observed 29-day success) and a safe insights history page size. No pin or vendor; this is the hosted API.

## Next

If insights history with limit 50 and a real cursor still returns truncated JSON, surface parse failure instead of Null and keep paging at 20. Re-check the errors "7 days" message if Scout changes the errors lookback.

## Source

Operator archive pull in this session. OpenAPI in-repo notes traces 7 days. Issue `usr/docs/issues/20260921124035_archive-pull-windows-and-metrics.md`.
