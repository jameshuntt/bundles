# bundles

Put a source tree into one annotated file, and get it back.

```text
bundles bundle   -r ./src -o project.bundle --ext rs --ext toml
bundles unbundle -b project.bundle -o ./restored
bundles bundle   -r . -o lib.bundle --use-lib src/lib.rs     # only the crate's module tree
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

`bundles install` uses the [`installation`](https://crates.io/crates/installation)
crate: cargo is asked where the built binary really is, a plan is printed,
the copy is verified, a receipt is kept, and `~/.local/bin` is added to
`PATH` once.
