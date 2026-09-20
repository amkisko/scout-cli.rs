# Scout CLI for agents

## Auth

API keys come only from secret backends (1Password, Bitwarden, KeePassXC). There is no `--api-key`. Run `scout config path` and `scout config list` when auth fails (exit code 3).

## Targeting apps

`APP` is a numeric id or an exact app name. Examples:

```text
scout endpoints 123 --range 1day
scout endpoints my-app --range 1day
```

Defaults: omit `APP` when `--app-id`, `SCOUT_APP_ID` / `app.id`, or `SCOUT_APP` / `app.name` is set.

## Preferred commands

| Goal | Command |
|------|---------|
| List apps | `scout apps --json` |
| Endpoints | `scout endpoints APP --range 1day --json` |
| Errors | `scout errors APP --json` |
| Insights | `scout insights APP --json` |
| Trace | `scout trace APP TRACE_ID --json` |
| Open UI | add `--web` (prints URL with `--no-input`) |
| Parse UI URL | `scout parse-url URL --json` |
| Local archive | `scout archive pull APP --range 7days` |
| Compare archive | `scout diff endpoints …` |
| Many ops | `scout batch --file plan.json` |

Exit codes: 0 success, 2 usage, 3 auth, 4 API, 5 I/O.

## Output

- Agents and scripts: `--json` or `-o json`
- Stable TSV: `--plain`
- Batch stdout is always a JSON report

## Batch

```text
echo '[{"args":["apps"]},{"args":["endpoints","123","--range","1day"]}]' | scout batch
```

Reject nested `batch`, and do not batch `config set` / `config unset`.
