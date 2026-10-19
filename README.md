# bundles

Put a source tree into one annotated file, and get it back.

```text
bundles bundle   -r ./src -o project.bundle --ext rs --ext toml
bundles unbundle -b project.bundle -o ./restored
bundles bundle   -r . -o lib.bundle --use-lib src/lib.rs     # only the crate's module tree
bundles bundle   -r . -o set.bundle --stdin < files.txt      # exactly these files, in this order
bundles install  [--dry-run]                                 # build + install for the current user
```

A bundle is plain text: `# start of path/file.rs`, the content, `# end of
path/file.rs`. Change the marker prefix with `-c //`.

## Rules that keep it honest

- `target/`, `.git/` and `node_modules/` are skipped (add more with `--ignore`).
- Only text files are bundled; a binary file is reported and skipped, never
  aborts the run.
- A file whose own content looks like a marker line is refused, because the
  bundle could not be read back unambiguously.
- Unbundling validates the whole bundle before writing anything, and every
  output path goes through `pathgen::checked_join`: `../x` and `/etc/x` inside
  a bundle are errors, not writes.
- A file without a final newline gets one on the way back; nothing else changes.
- The Rust module walker follows `x.rs` + `x/`, `x/mod.rs`, inline `mod x { mod y; }`
  and `#[path = "..."]`, and returns errors instead of panicking.

## An explicit file set

`bundle_set` and `BundleWriter` write the same format from entries the caller
names: a path, bytes or a reader, an optional note, an optional redaction.
Nothing is walked. A set of the files `bundle_files` would walk is the same
bytes `bundle_files` writes.

A note is written as `# note of <path>: <text>` just before its entry;
unbundling skips lines between entries, so a note never lands in a file.

A redaction replaces the whole file, or windows of its lines, with one marker
line each:

```text
# redacted level=secret lines=12-30 bytes=640 digest=sha256:<64 hex>
# redacted level=secret lines=all bytes=812 digest=sha256:<64 hex>
```

`level` is the level that redacted it, `bytes` and `digest` are over the exact
bytes left out. Unbundling is unchanged: a restored file holds the marker,
never the bytes. `redaction::RedactionMarker::parse` and `markers_in` read
markers back.

`--stdin` takes one line per file, relative to `-r`: `path`, `path<TAB>note`,
or `path<TAB>note<TAB>redact`, where redact is `level` (the whole file) or
`level:12-30,40-44`.

The program is the `bin` feature, on by default; a library user that wants
neither clap nor `installation` sets `default-features = false`.

`bundles install` uses the [`installation`](https://crates.io/crates/installation)
crate: cargo is asked where the built binary really is, a plan is printed,
the copy is verified, a receipt is kept, and `~/.local/bin` is added to
`PATH` once.

## Releasing

Bump `version` in Cargo.toml and give CHANGELOG.md a dated `## <version> (YYYY-MM-DD)` entry, then push to main.
CI (`.github/workflows/ci.yml`) tests on stable and Rust 1.85, runs clippy, and publishes to crates.io
through trusted publishing; a version already on crates.io, or an entry still marked `(unreleased)`, publishes nothing.
