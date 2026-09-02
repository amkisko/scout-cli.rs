# RFC 0004: Archive, diff, and batch

- Feature Name: archive-diff-batch
- Type: Standards Track
- Status: Stable
- Created: 2026-08-18
- Author: Andrei Makarov
- Relates: RFC 0002, RFC 0005

## Summary

Local archives live under `$SCOUT_ARCHIVE_HOME` (default `{SCOUT_HOME}/archive`). Commands are `archive pull|status|path|trace|export`, `scout diff`, and `scout batch`. Default export formats are ndjson, csv, and prometheus. Parquet is opt-in at build time.

## Motivation

Opening the web UI for every comparison leaves no file a later `scout diff` can read. A second consumer of archive files needs a format RFC before those layouts change.

## Guide-level explanation

`scout archive pull APP --range 1day` writes snapshots. Pulls are idempotent: existing range snapshots are skipped, and metric points merge into daily buckets without overwriting known timestamps. `archive status` and `archive path` inspect the tree. `archive trace` stores one trace.

`scout diff` compares two archived ranges locally (no API). `scout batch` runs several subcommands; stdout is always a JSON report. Nested batch and `config set`/`unset` are rejected.

Without the parquet build feature, parquet export returns an error naming the rebuild flag.

## Reference-level explanation

Query commands that fill archives are RFC 0005. Secrets for pull are RFC 0003. Nested `batch` and config mutation inside batch are errors.

## Registrar

Subcommands: `archive pull`, `archive status`, `archive path`, `archive trace`, `archive export`, `diff`, `batch`. Env: `SCOUT_ARCHIVE_HOME`. Feature: `export-parquet`.

## Drawbacks

Local disk grows with pulls. Parquet ships behind a feature flag so default installs stay smaller. A second consumer cannot rely on an unversioned directory layout.

## Rationale and alternatives

Shipping parquet in the default binary would pull extra crates into every install. Always hitting the API for diffs would skip local files. Doing nothing leaves operators in the web UI.

## Prior art

Prometheus recording, ScoutAPM web compare, CLI batch runners. RFC 0002 names archives as a second contract.

## Unresolved questions

Whether archive format versioning needs a dedicated RFC before a second consumer appears.
