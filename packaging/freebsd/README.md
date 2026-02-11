# FreeBSD Port

This directory contains a template for a FreeBSD port. The official Ports tree uses `USES=cargo` and requires a `CARGO_CRATES` list for reproducible builds.

**One-time setup (for port maintainers):**

1. Copy this `Makefile` into the Ports tree (e.g. `sysutils/scout-cli/`) or use it as a local port.
2. Run `make cargo-crates` in the port directory. This reads `Cargo.lock` and outputs a `CARGO_CRATES= ...` line.
3. Add that line to the Makefile, or put it in a separate `Makefile.crates` in the same directory (recommended for long lists).
4. Run `make makesum` to fetch distfiles and crates, then `make install`.

**Install from source (no port):**

```bash
pkg install rust
cargo install --path scout --root /usr/local
# or clone the repo and run from the scout-cli.rs directory
```
