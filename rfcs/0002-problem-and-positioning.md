# RFC 0002: Problem and positioning

- Feature Name: problem-and-positioning
- Type: Informational
- Status: Stable
- Created: 2026-08-17
- Updated: 2026-08-18
- Author: Andrei Makarov
- Relates: RFC 0003, RFC 0004, RFC 0005

## Summary

scout-cli queries ScoutAPM apps, endpoints, traces, metrics, and errors from the terminal. The binary is `scout`. API keys live in a secret backend; the CLI rejects plain-text keys on the command line and in env files.

## Motivation

Operators need ScoutAPM data in a shell, scripts, and local archives. Opening the web UI for every comparison is slow, and it leaves no file a later `scout diff` can read. Scripts also need stable exit codes: usage 2, auth 3, API 4, I/O 5.

API keys on `--api-key` or `SCOUT_APM_API_KEY` land in shell history and process lists. This design requires 1Password, Bitwarden, or KeePassXC, tried in that order when configured. `scout config set` writes `config.env` only and rejects plain-text API keys. Changing that rule, the backend order, or config precedence (flags, process environment, `config.local.env`, `config.env`, then `.scout.env` or `.env`) without a numbered RFC breaks existing operator setups.

Local archives are a second contract: `archive pull`, `status`, `path`, `trace`, and `export` under `$SCOUT_ARCHIVE_HOME`, plus `scout diff` and `scout batch`. Parquet export is opt-in at build time; ndjson, csv, and prometheus stay in the default build.

## Guide-level explanation

Store an API key in a secret backend. Point `~/.config/scout/config.env` or `SCOUT_HOME` at that setup. Flags override env, which overrides config files. `scout config path` and `scout config list` show what will load. Archive, diff, and batch commands work on pulled snapshots. `--plain` and `--json` stabilize script output; `SCOUT_OUTPUT` sets the default.

## Drawbacks

Operators need a secret manager. Local archives grow on disk. Parquet is a rebuild, not a flag on the default binary.

## Rationale and alternatives

A CLI plus local archive lets operators compare two time ranges without a second API round trip. Shipping parquet in the default binary would pull extra crates into every install. Reading a plain-text key from the environment would be shorter to document and would put the secret in process listings again.

## Prior art

ScoutAPM web UI, kubectl-style CLIs, OS secret stores. Contract detail is RFC 0003 through RFC 0005.

## Unresolved questions

Whether archive format versioning needs a dedicated RFC before a second consumer appears (RFC 0004).

Whether secret-backend order and config precedence should freeze as a registrar of env keys (RFC 0003).
