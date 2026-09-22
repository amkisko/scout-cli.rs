# Release binary size

## Participants

- amkisko

## Decisions

- Add workspace `[profile.release]` with thin LTO, codegen-units = 1, strip, and panic = abort.
- Turn off scout_lib default `export-parquet`; expose `export-parquet` as an optional feature on the scout CLI crate.
- Keep ndjson, csv, and prometheus export in the default binary.

## Effects

- Default `cargo build --release -p scout` no longer links Arrow/Parquet.
- Parquet users install or build with `--features export-parquet`.
- README install and archive export notes updated.
- Measured arm64 macOS release size: 15.9 MB before, 5.1 MB after (-67%). With `--features export-parquet`: 6.9 MB.

## Next

- Consider feature-gating the TUI in a later pass.

## Source

- Binary size analysis session (timely, scout, status, pray)
