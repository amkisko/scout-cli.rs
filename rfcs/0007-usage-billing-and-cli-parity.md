# RFC 0007: Usage, billing, and CLI parity

- Feature Name: usage-billing-and-cli-parity
- Type: Standards Track
- Status: Proposed
- Created: 2026-10-02
- Author: Andrei Makarov
- Relates: RFC 0005, RFC 0006
- Feedback until: 2026-10-16

## Summary

Add `scout usage` and `scout billing`, accept job full names and compact relative times, auto-JSON when stdout is not a TTY (unless `--plain`), and document framework setup via `scout docs`. Align small query contracts with upstream Scout CLI 1.0.

## Motivation

Operators need org billing figures and web-transaction usage without the web UI. Upstream Go CLI already ships those verbs. Job ids pasted as `queue/JobName`, compact `--from 7d`, and piped JSON for agents still fail closed or require extra flags here.

## Guide-level explanation

```text
scout usage --from 7d
scout usage --by-day --by-app --billing-period
scout usage --from 30d --concurrency 8
scout billing
scout job-metric APP default/MyWorker throughput --range 1day
scout docs rails
scout endpoints APP --range 1day | jq .   # JSON when piped
```

`--plain` stays TSV even when piped. `--json` stays compact JSON. Human tables remain the default on a TTY.

## Reference-level explanation

- `usage` estimates web transactions from throughput (jobs excluded). Flags: `--from`, `--to`, `--range`, `--all`, `--by-day`, `--by-app`, `--billing-period`, `--limit`, `--concurrency`, optional `APP`. With `--billing-period --json`, wrap as `{billing_period, server_total?, usage}`.
- `--concurrency` caps parallel per-app metric fetches. Default 4, minimum 1, maximum 32. JSON reports include the effective `concurrency` value.- `billing` returns `/api/v0/usage` for the current period (APM, nodes, errors, logs).
- Job args that contain `/` encode as Base64 URL-safe (with padding). Encoded ids that decode to `queue/Name` pass through. Other values fail client-side.
- `--from`/`--to` accept ISO 8601 and compact relative forms (`30m`, `1h`, `--from 7d`, `2w`). `--range` also accepts those units. Reversed ranges and relative windows over five years fail.
- Anomaly `--endpoint` accepts scoped name, plain name (prefixed `Controller/`), or Base64 endpoint id.
- When stdout is not a TTY and neither `--plain` nor an explicit output mode was forced by `--json`/`--json-pretty`/`-o json`, default to compact JSON.
- `scout docs [framework]` lists or prints the Scout docs URL for a framework. `scout setup` stays pray/skill only.
- Job metric type `latency` renames series key `total` to `execution_time_total` in JSON when a `Latency` series is also present.
- Client-side `--limit` applies to JSON list rows the same as human tables where the command supports `--limit`.

## Registrar

CLI verbs: `usage`, `billing`, `docs`. Flags: `--all`, `--by-day`, `--by-app`, `--billing-period`, `--concurrency` on `usage`. Compact relative time units: `m`, `h`, `d`, `w`.

## Drawbacks

More network fan-out on `usage` (per-app throughput). Auto-JSON changes pipe defaults for scripts that scraped human tables.

## Rationale and alternatives

Match upstream verbs where they map to org API surfaces. Keep secret-store auth and archive/diff/batch. Reject storing plain API keys. A single `scout setup rails` would collide with agent skill setup.

## Prior art

[scoutapp/scout-cli](https://github.com/scoutapp/scout-cli) v1.0.0 usage, billing, job encode, timeutil, pipe JSON.

## Unresolved questions

Whether `usage --by-day --by-app` must fetch top endpoints for every day/app cell, or can defer that to a later flag. Concurrent app fetches are capped by `--concurrency` (default 4).
