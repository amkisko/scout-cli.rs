# scout/scout-cli

ScoutAPM CLI (`scout`) usage skill for coding agents.

This tree is the local prayer under `prayers/` (path source). The name `v1` under `prayers/` is reserved for the published catalog (`prayers/v1/`).

Exports:

- `scout-cli` skill — auth, app targeting, preferred commands, output modes, and batch

## In this repository

`Prayfile` trees the package from `prayers/scout-cli`. Run `pray install`. Edit files under `prayers/scout-cli/skills/scout-cli/`, then re-run `pray install`. Keep `scout/skills/scout-cli/` in sync; that tree is what `scout setup --copy` embeds.

## Other repositories

```prayfile
source "scout", git: "https://github.com/amkisko/scout-cli.rs.git", distribution: "prayers"

tree ".agents/skills" do
  pray "scout/scout-cli", "~> 1.0"
end
```

Then `pray install`. Tree under `.agents/skills`. Do not compose into AGENTS.md.

## Publish (maintainers)

Requires [pray](https://github.com/kiskolabs/pray) 1.18+ (Cargo install). From the repo root:

```sh
pray package prayers/scout-cli
pray publish
```

That updates `prayers/v1/` (git-tracked catalog). Bump `spec.version` in `scout-scout-cli.prayspec` before a new release.
