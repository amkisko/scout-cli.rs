# Usage, billing, and CLI parity

## Participants

Andrei Makarov

## Decisions

Adopt upstream scoutapp/scout-cli 1.0 surfaces that this repo lacks: `usage`, `billing`, job full-name encode, compact relative times, pipe auto-JSON, anomaly endpoint forms, chart honesty, `scout docs` for framework links, and small contract fixes (last_reported_at, latency series rename, trace 404 copy). Spec lives in RFC 0007. Keep secret-store auth, archive/diff/batch, and pray-based `scout setup`.

## Effects

Branch `feature/usage-billing-and-cli-parity`. RFC 0007 Proposed. Implemented library modules `resource_id`, `time_parse`, `usage`, `org`; CLI modules `usage_cmd`, `usage_report`, `billing_cmd`, `docs_cmd`, `chart`. OpenAPI `/usage`. Tests pass for lib + CLI focus. LOC hard check clean (soft warnings remain on larger files).

### Engineering audit (this branch)

Pipeline: ingress (CLI flags / relative times / job ids) → app logic (usage aggregation) → external API (Scout `/metrics/throughput`, `/usage`) → egress (JSON/table).

Findings (smallest fix first):

1. [med/high] `scout usage` fans out one throughput series fetch per app (and per day for top endpoints). Cap with `--concurrency` (default 4, max 32); JSON reports the effective value.
2. [low/high] Pipe auto-JSON changes default for scripts that scraped human tables. Mitigated by `--plain` and RFC note.
3. [low/med] `get_app_enriched` may call `list_apps` when single-app payload omits `last_reported_at` (extra request). Acceptable for show command.
4. [info] Resource and budget: usage is the new hot path (N apps × chunks). Trace and identification: no new device fingerprints; API key still only from secret backends.
5. [info] Security: no plaintext key path added; docs/usage/billing do not log credentials.
6. [info] Contract: JSON shape for `--billing-period` wraps `{billing_period, server_total?, usage}` matching upstream.

Modes skipped: boundary/control (no physical plant), learned-systems, lineage.

## Next

1. Live smoke of `usage` / `billing` with a real org key.
2. Publish and tag 0.6.1 (`scoutapm-cli` on crates.io; `scout_lib` 0.6.1 already published).

## Source

- https://github.com/scoutapp/scout-cli (v1.0.0)
- rfcs/0007-usage-billing-and-cli-parity.md
- docs/changelogs/20261002071550_usage-billing-and-cli-parity.md
- docs/openapi.yaml
