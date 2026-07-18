# scout-cli

[![Test Status](https://github.com/amkisko/scout-cli.rs/actions/workflows/test.yml/badge.svg)](https://github.com/amkisko/scout-cli.rs/actions/workflows/test.yml)

ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors from the terminal.

## Requirements

- Rust 1.70+ (for building from source), or use a pre-built package below.

## Quick start

1. Create an API key in ScoutAPM: [Organization settings](https://scoutapm.com/settings).
2. Store the key in a **secret backend** (1Password, Bitwarden, or KeePassXC) and configure the CLI via `~/.scout/config.env` or env vars—see below.

### Home config (`SCOUT_HOME`)

By default the CLI reads backend settings from `~/.config/scout/config.env` (XDG) or legacy `~/.scout/config.env`, plus optional `config.local.env`. Set `SCOUT_HOME` to use another directory. Project-level `.scout.env` or `.env` in the current working directory can also supply `SCOUT_*` keys.

Configuration precedence (highest first):

1. Command-line flags
2. Process environment variables
3. `config.local.env` in the scout config directory
4. `config.env` in the scout config directory
5. `.scout.env` or `.env` in the current working directory

Copy [config.env.example](config.env.example) as a starting point:

```bash
mkdir -p ~/.scout
cp config.env.example ~/.scout/config.env
# edit ~/.scout/config.env
```

Process environment variables override file values. `config.local.env` overrides `config.env` for keys not already set in the shell.

```bash
scout config path
scout config list
scout config get op.entry_path
scout config set op.entry_path 'op://Vault/Scout APM'
scout config unset op.entry_path
```

Use `--output json` with `list` or `get` for machine-readable output. Friendly keys:

| Key | Env var |
|-----|---------|
| `op.entry_path` | `SCOUT_OP_ENTRY_PATH` |
| `op.vault` | `SCOUT_OP_VAULT` |
| `op.item` | `SCOUT_OP_ITEM` |
| `op.field` | `SCOUT_OP_FIELD` |
| `bw.item_id` | `SCOUT_BW_ITEM_ID` |
| `bw.session` | `SCOUT_BW_SESSION` |
| `kpxc.db` | `SCOUT_KPXC_DB` |
| `kpxc.entry` | `SCOUT_KPXC_ENTRY` |
| `kpxc.attribute` | `SCOUT_KPXC_ATTRIBUTE` |

`scout config set` writes to `config.env` only. Plain-text API keys are rejected.

### API key (secret backends only)

**Plain-text API keys are not supported.** The CLI does not accept `--api-key` or `API_KEY` / `SCOUT_APM_API_KEY` environment variables. You must use one of the supported secret backends so the key is never on the command line or in shell history.

Resolution order: **1Password** → **Bitwarden** → **KeePassXC**. Each backend is only tried when its settings are set (in the environment or home config).

| Backend     | Env vars | Notes |
|------------|----------|--------|
| **1Password** | `SCOUT_OP_ENTRY_PATH=op://Vault/Item` or `op://Vault/Item/Field`, or `SCOUT_OP_VAULT` + `SCOUT_OP_ITEM` | When the path has no field segment, `SCOUT_OP_FIELD` is appended (default `API_KEY`). Uses `op read`. |
| **Bitwarden** | `SCOUT_BW_ITEM_ID` (login item UUID) | Optional `SCOUT_BW_SESSION` (from `bw unlock --raw`). Uses `bw get password`. |
| **KeePassXC** | `SCOUT_KPXC_DB` (path to .kdbx), `SCOUT_KPXC_ENTRY` (entry title/path) | Optional `SCOUT_KPXC_ATTRIBUTE` (default `Password`). Uses `keepassxc-cli show`. |

Install the CLI for your chosen backend (`op`, `bw`, or `keepassxc-cli`) and ensure the vault is unlocked (e.g. `op signin`, `bw unlock`) when running `scout`.

### Install

**Cargo (from source)**

```bash
cargo install --path scout
# or from git
cargo install --git https://github.com/amkisko/scout-cli.rs --package scout
```

**Homebrew** (macOS/Linux)

```bash
brew tap amkisko/tap  # once
brew install scout-cli
```

Formula lives in [packaging/homebrew/scout-cli.rb](packaging/homebrew/scout-cli.rb). Update the formula's `url` and `sha256` for new releases.

**Nix**

```bash
nix build .#default
# or with flake-utils: nix run .#default
```

See [flake.nix](flake.nix) and [packaging/nix/](packaging/nix/).

**Arch Linux (AUR)**

```bash
yay -S scout-cli
```

PKGBUILD and instructions: [packaging/aur/](packaging/aur/).

**FreeBSD**

Port template in [packaging/freebsd/](packaging/freebsd/). Or install Rust (`pkg install rust`) and run `cargo install --path scout` from the repo.

**Gentoo**

Ebuild template in [packaging/gentoo/](packaging/gentoo/). For a full offline build run `cargo ebuild` from the repo and use the generated ebuild in a local overlay. Or `cargo install --path scout`.

**Flatpak**

See [packaging/flatpak/](packaging/flatpak/). Build may require a Rust-enabled SDK.

## Usage

All [OpenAPI v0.1](doc/openapi.yaml) endpoints are supported. For ScoutAPM API questions or additional endpoints, see [ScoutAPM documentation](https://scoutapm.com/docs).

**Output format:** use `-o` / `--output`, `--json`, or `--plain`:

- **plain** (default) — human-readable tables and key-value text
- **--plain** — script-stable tab-separated records (one per line)
- **-o json** — pretty JSON (backward compatible)
- **--json** — compact JSON for scripts

**Global flags:** `--quiet`, `--verbose`, `--debug`, `--no-color`, `--no-input`, `--timeout`, `--api-base`. Use `scout config --dry-run` to preview config writes. Most flags work before or after the subcommand.

**Shell completions:** `scout completions bash|zsh|fish` (also installed by Homebrew and AUR packages).

**Uninstall:** remove the binary (`brew uninstall scout-cli`, `cargo uninstall scout`, or your package manager). Config in `SCOUT_HOME` / `~/.config/scout` is left in place unless you delete it manually.

**Interactive TUI:** run `scout` with no arguments to start the interactive TUI and browse apps and endpoints (↑/↓ to select, Enter to load endpoints for the selected app, `?` for shortcuts, q or Esc to quit). Timestamps are shown in your local timezone by default; use `--utc` to show UTC only. Use `--no-input` or run in a non-terminal environment to disable the TUI.

```bash
# Plain text (default)
scout apps
scout -o json apps    # JSON output

# Interactive TUI (no arguments)
scout

# Applications
scout apps
scout app 123

# Metrics
scout metrics 123
scout metric 123 response_time --range 7days
scout metric 123 errors --from 2025-01-01T00:00:00Z --to 2025-01-02T00:00:00Z

# Endpoints
scout endpoints 123 --range 1day
scout endpoints 123 --range 1day --sort-by response_time --limit 50 --offset 0
scout endpoint-metric 123 <endpoint_id> response_time --range 7days
scout endpoint-traces 123 <endpoint_id> --range 1day

# Jobs (background jobs)
scout jobs 123 --range 1day
scout job-metrics 123 <job_id>
scout job-metric 123 <job_id> execution_time --range 7days
scout job-traces 123 <job_id> --range 1day

# Traces
scout trace 123 456

# Anomaly events
scout anomaly-events 123 --range 7days [--state open|closed|all] [--metric response_time] [--endpoint "Controller#action"]
scout anomaly-event 123 456

# Errors
scout errors 123 [--from ...] [--to ...] [--endpoint <base64>]
scout error 123 789
scout error-group-errors 123 789

# Insights (current + history with pagination)
scout insights 123 [--limit 20]
scout insight 123 n_plus_one [--limit 20]
scout insights-history 123 [--from ...] [--to ...] [--limit 10] [--pagination-cursor ...] [--pagination-direction forward|backward] [--pagination-page 1]
scout insights-history-by-type 123 n_plus_one [same options]

# Utilities
scout parse-url "https://scoutapm.com/apps/123/endpoints/.../trace/456"
scout parse-url -   # read URL from stdin
scout completions bash > ~/.local/share/bash-completion/completions/scout
scout man > /tmp/scout.1
scout version
scout --version

# Local archive (extends ScoutAPM retention on your machine)
scout archive path
scout archive status
scout archive status 123
scout archive pull 123 --range 1day
scout archive pull 123 --dry-run --range 1day
scout archive pull 123 --incremental
scout archive pull 123 --from 2025-01-01T00:00:00Z --to 2025-01-02T00:00:00Z --resource metrics --resource endpoints

# Diff archived snapshots (local only, no API calls)
scout diff endpoints 123 \
  --left-from 2025-01-01T00:00:00Z --left-to 2025-01-02T00:00:00Z \
  --right-from 2025-01-08T00:00:00Z --right-to 2025-01-09T00:00:00Z
scout diff errors 123 --left-from ... --left-to ... --right-from ... --right-to ...
scout diff jobs 123 --left-from ... --left-to ... --right-from ... --right-to ...
scout diff metrics 123 response_time --left-date 2025-01-01 --right-date 2025-01-08

# Archive one trace or pull traces from endpoint listings
scout archive trace 123 456
scout archive pull 123 --resource traces --range 1day
scout archive pull 123 --trace-id 456 --trace-id 789

# Export archived data for other systems
scout archive export 123 --resource metrics --metric response_time --date 2025-01-01 --format prometheus
scout archive export 123 --resource endpoints --from 2025-01-01T00:00:00Z --to 2025-01-02T00:00:00Z --format csv --output endpoints.csv
scout archive export 123 --resource metrics --metric response_time --date 2025-01-01 --format parquet --output metrics.parquet
scout archive export 123 --resource errors --from ... --to ... --format ndjson --output errors.ndjson

# Batch multiple scout operations (stdout is always a JSON report)
echo '[{"args":["archive","path"]},{"args":["config","path"]}]' | scout batch
scout batch --file plan.json
```

Archive data is stored under `$SCOUT_ARCHIVE_HOME` (default: `{SCOUT_HOME}/archive`). Pulls are idempotent: existing range snapshots are skipped, and metric points are merged into daily buckets without overwriting known timestamps.

`scout batch` runs several operations in one invocation. Each item is a normal scout subcommand (`args` array). Scout resolves API access only when an operation needs it; local commands such as `archive status` or `diff` use on-disk data. Output is always a JSON report on stdout with per-operation `ok`, `data`, and `error` fields; use `--json-pretty` for indented output. Pass `--fail-fast` to stop after the first failed operation. Nested batch and `config set`/`unset` are rejected. Run `scout batch` in a terminal without `--file` or piped input for concise usage help.

Example batch plan (`plan.json`):

```json
{
  "operations": [
    { "id": "archive", "args": ["archive", "status", "123"] },
    { "id": "apps", "args": ["apps"] }
  ]
}
```

Example cron for daily incremental pull:

```bash
0 6 * * * scout archive pull 123 --incremental --json >> ~/scout-pull.log 2>&1
```

API key: configure one secret backend in `~/.scout/config.env` or via env vars (see above). Plain-text keys are not supported.

## Development

- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace`
- Release (from repo root): `cargo run -p release` — runs checks, verifies packaging, then publish and GitHub release.
- Packaging sync: `make sync-packaging` (or `cargo run -p release --bin sync-packaging`) — align packaging manifests with the workspace version in `Cargo.toml`.

## Repository layout

- `scout_lib` — ScoutAPM API client library
- `scout` — CLI binary
- `usr/bin/release` — release tooling (`sync-packaging`, `release`)
- `packaging/` — Homebrew, Nix, Flatpak, AUR, FreeBSD (port), Gentoo (ebuild)
- `.github/workflows` — CI (test, format, clippy)

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/amkisko/scout-cli.rs

Contribution policy:
- New features are not necessarily added to the project
- Pull request should have test coverage for affected parts
- Pull request should have changelog entry

Review policy:
- It might take up to 2 calendar weeks to review and merge critical fixes
- It might take up to 6 calendar months to review and merge pull request
- It might take up to 1 calendar year to review an issue

For questions or coordination, see [CONTRIBUTING.md](CONTRIBUTING.md) or open a [GitHub Discussion](https://github.com/amkisko/scout-cli.rs/discussions).

## Security

If you discover a security vulnerability, please report it responsibly. **Do not** open a public issue. See [SECURITY.md](SECURITY.md) for how to report.

## Links

- [GitHub](https://github.com/amkisko/scout-cli.rs)
- [GitLab](https://gitlab.com/amkisko/scout-cli.rs)
- [SonarCloud](https://sonarcloud.io/project/overview?id=amkisko_scout-cli.rs)
- [Snyk](https://snyk.io/test/github/amkisko/scout-cli.rs)
- [Codecov](https://app.codecov.io/github/amkisko/scout-cli.rs)
- [OpenSSF Scorecard](https://scorecard.dev/viewer/?uri=github.com/amkisko/scout-cli.rs)

## License

MIT. See [LICENSE.md](LICENSE.md).

## Sponsors

Sponsored by [Kisko Labs](https://www.kiskolabs.com).

<a href="https://www.kiskolabs.com">
  <img src="kisko.svg" width="200" alt="Sponsored by Kisko Labs" />
</a>
