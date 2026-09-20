# RFC 0006: App resolve, web open, and agent setup

- Feature Name: app-resolve-web-agent-setup
- Type: Standards Track
- Status: Proposed
- Created: 2026-09-20
- Author: Andrei Makarov
- Relates: RFC 0003, RFC 0005
- Feedback until: 2026-10-04

## Summary

Operators and agents address an app by id or name, open the matching ScoutAPM UI URL with `--web`, and provision the scout agent skill through Prayfile when pray is available.

## Motivation

Numeric `APP_ID` on every verb slows interactive and agent use. Terminal results stay disconnected from the ScoutAPM UI. Agent guidance that only copies files into `~/.agents` ignores repos that already provision skills with pray.

## Guide-level explanation

```text
scout endpoints my-app --range 1day
scout error 42 1001 --web
scout config set app.id 42
scout setup
```

`APP` accepts a numeric id or an exact app name (case-insensitive). `APP` may be omitted when `--app-id`, `SCOUT_APP_ID` / `app.id`, or `SCOUT_APP` / `app.name` supplies the target. Config is loaded before CLI parsing so saved defaults apply. `--web` opens the resource in the browser, or prints the URL on stderr when `--no-input` is set (stdout stays one JSON document for `--json`).

`scout setup` prefers pray: if a `Prayfile` can provision the scout-cli skill, it runs `pray install`. If pray is missing, it explains how to install pray and can copy the skill tree into `.agents/skills/scout-cli` as a fallback.

## Reference-level explanation

App resolution order:

1. Positional `APP` when present on the argv
2. Global `--app-id` (env `SCOUT_APP_ID` / config `app.id`) when it does not conflict with a numeric `APP`
3. `SCOUT_APP` / config `app.name` (including when clap fills `APP` from that env after config load)
4. Fail with usage guidance

Home config is loaded before clap parsing. When `SCOUT_APP` is unset and `SCOUT_APP_ID` is set, the id is copied into `SCOUT_APP` so the optional positional can resolve.

A numeric `APP` is used as the id. A non-numeric `APP` loads `list_apps` and matches exact name case-insensitively. Zero matches or more than one match fail closed. Local archive/diff commands that cannot call the API require a numeric `APP` or `--app-id`.

`--web` builds `https://scoutapm.com/...` permalinks for the addressed resource (app, endpoint, job, trace, error group, insight). Host follows `SCOUT_API_BASE` by stripping a trailing `/api/v0` when present; otherwise `https://scoutapm.com`. With `--no-input`, the CLI prints the URL on stderr and does not spawn a browser.

Agent skill source of truth for pray is `prayers/scout-cli` (`scout/scout-cli`), declared under `tree ".agents/skills"` in `Prayfile` and published to `prayers/v1/`. The `scout` crate embeds a copy under `scout/skills/scout-cli` for `scout setup --copy`. `scout setup --copy` MUST write to `--path` (or the default skill dir) and MUST NOT run `pray install`. Without `--copy`, `scout setup` MUST prefer `pray install` when a Prayfile is in scope and pray is on `PATH`.

## Security considerations

App name resolution calls the API with the existing secret-backend key. `--web` only opens or prints URLs derived from ids already returned or supplied; it does not embed secrets. Setup writes under the project `.agents/skills` tree or paths the operator passes; it does not write API keys.

## Registrar

- CLI: `APP` (id or name), `--web`, `scout setup`
- Config/env: `app.id` → `SCOUT_APP_ID`, `app.name` → `SCOUT_APP` (non-secret; extends RFC 0003 registrar for targeting only)

## Drawbacks

Name resolution costs a `list_apps` call. Optional `APP` plus extra positionals can mis-bind when operators omit `APP` and pass another token first.

## Rationale and alternatives

Keeping numeric-only `APP_ID` preserves script brevity and blocks humane targeting. A separate `scout open` command duplicates `--web`. Shipping agent skills only as a home-directory copy fights pray-managed repos.

## Prior art

Scout TUI `--app` name match. `parse-url` path shapes. Pray `tree` skill provisioning.

## Unresolved questions

Whether ambiguous partial name match should be allowed later. Whether `scout setup` should write a Prayfile stanza automatically.
