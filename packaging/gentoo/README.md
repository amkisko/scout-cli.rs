# Gentoo

**Option A – Generated ebuild (recommended, offline build):**

From the project root (with Rust installed):

```bash
cargo install cargo-ebuild
cargo ebuild
```

Use the generated ebuild (it will include all `CARGO_CRATE_URIS`). Copy it into your overlay under `app-misc/scout-cli/`.

**Option B – Template ebuild:**

The provided `app-misc/scout-cli/scout-cli-0.1.0.ebuild` is a minimal template. Copy it to a local overlay. It may require network access during build to fetch crates. For a fully offline, reproducible build use Option A.

**Install from source (no overlay):**

```bash
cargo install --path scout
```
