# Archive pull windows, metric tuples, and aggregate cache

## Participants

Andrei Makarov

## Decisions

Treat a full archive cache as aggregates: app metrics, endpoint and job listings, per-endpoint and per-job series, errors, anomalies, and insights. Traces stay an explicit sample (`archive trace` or `--trace-id`, or `--resource traces`). Default pull omits traces.

Accept live metric points as `[timestamp, value]` tuples and store `{timestamp, value}` objects in daily buckets so `scout diff metrics` can average them.

One requested `--range` is split into 14-day request chunks, then each resource clamps to its Scout lookback (errors and anomalies just under 30 days, traces just under 7 days). `--range max` means 30 days before those clamps. A refused window is recorded on the pull report; other resources keep going. The manifest is saved after each resource. Skip-if-exists reindexes files already on disk. `--trace-endpoint-limit 0` means all endpoints and jobs.

Insights history in archive pull pages at 20 rows. Leftover unindexed traces from an earlier bulk pull were not deleted.

## Effects

Fact-check against a live ScoutAPM app on 2026-09-21.

`scout metric … throughput --range 1day --json` returns a series object whose points are `[iso, number]` tuples. Workspace `archive pull --resource metrics --range 1day` into a temp archive wrote daily buckets. Installed `scout` on PATH still wrote 0 points into an empty temp archive (old parser).

`--range 30days` with metrics, errors, and anomalies into a temp archive finished with an empty refusals list, three request chunks, and a non-null manifest with last_pull fields. Exact `7days` / `30days` as N * 86400 from now is clamped per resource by one hour of lookback slack.

`--range max --dry-run --json` lists resources app, metrics, endpoints, jobs, endpoint_metrics, job_metrics, errors, anomalies, insights (no traces) and three chunks.

`--resource endpoint_metrics --trace-endpoint-limit 1 --range 1day` wrote per-endpoint daily buckets.

`insights-history --limit 50` page 1 succeeded (~4MB). `--pagination-page 4 --limit 50` without a cursor returned API error: Must specify pagination_cursor for pages after the first (exit 4). The reported page-4 `error decoding response body` was not reproduced on this client (response body is parsed as text then JSON; invalid JSON becomes Null). `--limit 20` page 1 is ~3MB.

`merge_series_into_buckets` of 10000 tuple points in debug tests finished in 0.58s (ceiling 2s).

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo run -p release --bin check-loc` passed (12 soft loc warnings). Branch remained `main`.

## Next

Default pull now issues per-endpoint and per-job metric calls for up to 50 items (6 app metric types, 5 job metric types) per 14-day chunk. That N+1 path was measured for 1 endpoint over 1 day, not for 50 items over 30 days.

`Client::send` maps a JSON parse failure to Null on HTTP 200. A truncated insights page would be stored as empty rather than refused.

`archive status` entity counts for anomalies and error_groups stay 0 because those live as range snapshots. Range snapshot records are the compare unit.

Orphan `traces/by_id` files from the earlier bulk pull can be deleted by the operator if disk is needed; this change set did not delete them.

## Source

Operator report in this session (tuple series, one window aborting the chunk, 7d/30d caps, manifest-at-end, insights page size, traces vs aggregates). Live probes against ScoutAPM REST. Implementation under `scout_lib/src/archive/` and `scout/src/cli_archive.rs`.

## Claims

Claim: live `scout metric` series are `[timestamp, value]` tuples. Outcome: supported. Evidence: 2026-09-21 JSON series values are two-element arrays of ISO timestamp and number.

Claim: old merge accepted only `{timestamp, value}` objects so archive metrics stored 0 points. Outcome: supported. Evidence: installed PATH `scout` temp pull `metric_points_added: 0`; workspace binary wrote buckets.

Claim: one `--range` applied to every resource and one API refusal aborted the rest. Outcome: supported as prior behavior (pull returned Err). Outcome after fix: `--range 30days` metrics+errors+anomalies refusals [].

Claim: `--range 7days` / `30days` is N * 86400 and Scout rejects "older than N days". Outcome: partially supported. Exact 30-day errors+anomalies pull succeeded after one hour of slack. Error body "older than 7 days" for a 30-day errors lookback was reported earlier and not re-hit on this 30-day pull. 29-day vs 30-day anomaly cap remains the reason for slack.

Claim: manifest only written at end, so abort left `manifest: null` and skip-if-exists did not reindex. Outcome: supported as prior behavior. After fix, temp 30-day pull wrote last_pull_* and metric_buckets; `reindex_app` counts orphan traces.

Claim: insights-history `--limit 50` failed on page 4 with error decoding response body; `--limit 20` walked the full history. Outcome: partially supported. Page 1 limit 50 succeeded (~4MB). Page 4 without cursor is an API validation error, not a decode error. Full history walk not re-run this session. Archive pages at 20.

Claim: leftover traces from an earlier bulk pull are not in the manifest. Outcome: unverifiable this run (operator testimony; files not counted here).

## Engineering audit

Pipeline: CLI ingress (`archive pull`) -> window clamp -> ScoutAPM HTTP -> daily JSON buckets / range snapshots -> manifest -> `scout diff` egress.

Iteration 1 (contract, pipeline, boundary): metric tuple shape vs object parser (fixed). One window vs per-resource Scout caps (fixed). Abort-on-first-refusal vs PullRefusal (fixed). Scout error text "older than 7 days" is not the measured errors lookback. Intended window, commanded query, and API-enforced lookback can diverge; clamp is the coupling.

Iteration 2 (resource and budget, performance): 10000-point merge debug test 0.58s on this machine class (ceiling 2s). 1-day app metrics wrote buckets. 30-day metrics+errors+anomalies used three chunks and completed without refusals. Insights page 20 ~3MB, page 50 ~4MB. Default series N+1 at 50 items is inference until benched. Merge timestamp membership is linear scan per point (O(n^2) within a series); 10k stayed under 2s. Energy not metered; CPU-seconds proxy is the merge test wall.

Iteration 3 (trace and identification, privacy, security): Archive stores APM app metrics, endpoint names, error groups, and optional traces under SCOUT_ARCHIVE_HOME. API key is not passed on argv in these commands; secret backends remain the auth path. No extra analytics id is minted by pull. TLS to scoutapm.com is the channel.

Product surface: `archive pull --help` now names resources, `--range max`, 7-day and 30-day caps, and `0 = all`. `--dry-run --range max` lists the default resource set without traces.

Observability: skip as a long-lived service. Operator progress is stderr lines per resource, per endpoint/job for traces and series. Failed windows land in report.refusals.

Learned-systems: skip; this tree is a ScoutAPM client, not a generative runtime.

Remaining severity: default per-item series N+1 unbounded by time (50 * 11 types * chunk count) is medium, confidence high, kind observed for 1 item, inferred at 50. Silent Null on JSON parse is low, confidence high, kind observed in `client.rs` send(). Status entity counters vs range snapshots is low product-surface mismatch.
