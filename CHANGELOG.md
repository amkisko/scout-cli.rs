# CHANGELOG

## Unreleased

## 0.5.0 (2026-09-21)

- Store Scout metric series that arrive as `[timestamp, value]` tuples so `archive pull --resource metrics` writes daily buckets and `scout diff metrics` can read them.
- Apply a per-resource lookback when pulling an archive; record a refused Scout window and continue the rest of the pull. `--range max` is 30 days before those clamps.
- Write the archive manifest after each resource and reindex files already on disk so `archive status` is not `manifest: null` after a mid-pull failure.
- Default `archive pull` stores app metrics, endpoint and job listings, per-endpoint and per-job series, errors, anomalies, and insights. Traces stay out of that set; fetch one with `archive trace` or `--trace-id`. `--trace-endpoint-limit 0` means all endpoints and jobs.
- Document `--resource`, `--range`, `--from`, `--to`, and the 7-day and 30-day Scout caps in `archive pull --help`.
- Accept app name or id for `APP`; omit `APP` when `--app-id`, `app.id` / `SCOUT_APP_ID`, or `app.name` / `SCOUT_APP` is set (RFC 0006).
- Load home config before CLI parsing so saved `app.id` / `app.name` defaults apply.
- Add `--web` / `-w` to open the matching ScoutAPM UI URL (print on stderr with `--no-input`).
- Add `scout setup` to provision the agent skill via pray when available; `--copy` honors `--path`.
- Provision `.agents/skills/scout-cli` from the in-repo `scout/scout-cli` prayer (`prayers/scout-cli`) through `Prayfile`; publish the catalog under `prayers/v1/`.
- Add `rfcs/` starter pack: process (RFC 0001), positioning (RFC 0002), and Standards Track design RFCs 0003–0005 for secrets, archive/diff/batch, and query output.

## 0.4.0 (2026-07-25)

- Shrink the default release binary: enable thin LTO, symbol stripping, and a single codegen unit.
- Make parquet archive export opt-in via `--features export-parquet` (ndjson/csv/prometheus remain in the default build).

## 0.3.0 (2026-07-15)

- Add local archive: `archive pull`, `status`, `path`, `trace`, and `export` (csv, prometheus, ndjson, parquet) under `$SCOUT_ARCHIVE_HOME` (default `{SCOUT_HOME}/archive`).
- Add `scout diff` to compare archived endpoint, metric, error, and job snapshots without API calls.
- Add `scout batch` to run multiple operations from a JSON plan with per-operation results on stdout.
- Add `scout completions` (bash, zsh, fish) and `scout man`; Homebrew and AUR packages install them.
- Add `--plain` for script-stable tab-separated output and `--json` for compact JSON; set default via `SCOUT_OUTPUT`.
- Add global flags `--quiet`, `--verbose`, `--debug`, `--no-color`, `--no-input`, `--timeout`, `--api-base`, and `--app-id`.
- Map failures to script-friendly exit codes: usage (2), auth (3), API (4), I/O (5).
- Extend interactive TUI with `--tab`, `--refresh`, `--utc`, and local-time timestamps by default.
- Add `scout config --dry-run` to preview config writes.

## 0.2.0 (2026-07-15)

- Add background job commands: `jobs`, `job-metrics`, `job-metric`, `job-traces`.
- Add anomaly event commands: `anomaly-events`, `anomaly-event` (filter by state, metric, or endpoint).
- Add endpoint listing options `--sort-by`, `--limit`, and `--offset` for sorted, paginated results.
- Extend `parse-url` to recognize job and job trace ScoutAPM URLs.
- Load secret-backend settings from `SCOUT_HOME` (default `~/.scout/config.env`).
- Add `scout config` to list, get, set, and unset home config values.

## 0.1.0 (2025-02-10)

- ScoutAPM API client library (`scout_lib`): apps, metrics, endpoints, traces, error groups, insights.
- CLI (`scout`): subcommands for apps, app, metrics, metric, endpoints, trace, errors, error, insights, insight, parse-url, version.
- API key from environment (`SCOUT_APM_API_KEY`, `API_KEY`) or `--api-key` (removed in a later release in favor of secret backends only).
- Time ranges: `--range` (e.g. 30min, 1day, 7days) and `--from` / `--to` (ISO 8601).
- Release script in Rust: `cargo run -p release` (format, clippy, test, tag, publish).
- CI: test workflow (format check, clippy, tests).
- Packaging: Homebrew formula, Nix flake and default.nix, Flatpak manifest, AUR PKGBUILD.
