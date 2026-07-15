# CHANGELOG

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
