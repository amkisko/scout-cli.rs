# CHANGELOG

## 0.1.0 (2025-02-10)

- ScoutAPM API client library (`scout_lib`): apps, metrics, endpoints, traces, error groups, insights.
- CLI (`scout`): subcommands for apps, app, metrics, metric, endpoints, trace, errors, error, insights, insight, parse-url, version.
- API key from environment (`SCOUT_APM_API_KEY`, `API_KEY`) or `--api-key` (removed in a later release in favor of secret backends only).
- Time ranges: `--range` (e.g. 30min, 1day, 7days) and `--from` / `--to` (ISO 8601).
- Release script in Rust: `cargo run -p release` (format, clippy, test, tag, publish).
- CI: test workflow (format check, clippy, tests).
- Packaging: Homebrew formula, Nix flake and default.nix, Flatpak manifest, AUR PKGBUILD.
