# RFC 0005: Query commands and output

- Feature Name: query-commands-and-output
- Type: Standards Track
- Status: Stable
- Created: 2026-08-18
- Author: Andrei Makarov
- Relates: RFC 0002, RFC 0003, RFC 0004

## Summary

`scout` queries ScoutAPM apps, metrics, endpoints, jobs, traces, errors, and insights. Exit codes: usage 2, auth 3, API 4, I/O 5. Output modes are human tables, `--plain`, JSON, and `--json`. No arguments starts the TUI.

## Motivation

Scripts need stable verbs and exit codes. Changing a subcommand name or remapping 3 versus 4 without a numbered RFC breaks operator automation.

## Guide-level explanation

Examples: `scout apps`, `scout metrics`, `scout endpoints`, `scout jobs`, `scout trace`, `scout errors`, `scout insights`, `scout parse-url`. `scout` with no arguments opens the TUI unless `--no-input` or a non-terminal environment.

Output: default human tables; `--plain` tab-separated records; `-o json` pretty JSON; `--json` compact JSON. `SCOUT_OUTPUT` sets the default.

`scout completions bash|zsh|fish` writes shell completions. `scout version` prints the version.

## Reference-level explanation

Auth failures are exit 3 (RFC 0003). Archive, diff, and batch are RFC 0004. Usage errors are exit 2. API transport errors are exit 4. Local I/O errors are exit 5.

## Registrar

Exit codes: 2 usage, 3 auth, 4 API, 5 I/O. Output flags: `--plain`, `--json`, `-o` / `--output`.

## Drawbacks

Exit-code matrix must stay small. TUI on empty argv surprises non-interactive callers unless `--no-input` is set.

## Rationale and alternatives

HTTP status as the only signal would not distinguish auth from usage in a script. Always-JSON output would hurt interactive use. Doing nothing leaves operators scraping the web UI.

## Prior art

kubectl, gh, and similar CLIs with stable verbs and exit codes. ScoutAPM HTTP API. RFC 0002 positions this binary as the terminal client.

## Unresolved questions

Whether OpenAPI additions require an RFC per endpoint family, or only when a field a script parses changes.
