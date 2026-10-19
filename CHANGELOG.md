# Changelog

## 0.2.3 (pending, not published)

A caller that already knows which files belong in a bundle had no way to say
so. `bundle_files` walks a root
([old](https://github.com/jameshuntt/bundles/blob/a3c2ff765d2ba87118fb2177220f0c7fdebf97cd/src/bundler.rs#L38-L56)), and `bundle_only_used` takes a list but
reads every file from disk under the root and carries nothing per file
([old](https://github.com/jameshuntt/bundles/blob/a3c2ff765d2ba87118fb2177220f0c7fdebf97cd/src/bundler.rs#L59-L66)). custodian computes its file set from
klepto's data and holds secret windows in memory; it needs to hand those bytes
over in its own order, with a note per file and some lines left out.

- An explicit file set: `FileEntry` (path, bytes or a reader, note, redact)
  ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/bundler.rs#L170-L200)), `BundleWriter` writing entries to any
  `Write` ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/bundler.rs#L239-L344)), and `bundle_set` to a file
  ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/bundler.rs#L346-L355)). One format: the entry writer
  `write_entry` is the one `bundle_files` and `bundle_only_used` now go
  through ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/bundler.rs#L102-L165)), and a set of the walked files
  is byte for byte the walked bundle. Test: `tests/set.rs`
  `a_set_writes_the_same_bytes_bundle_files_writes`.
- Notes: `# note of <path>: <text>` lines just before an entry. Unbundle
  already skips lines between entries
  ([old](https://github.com/jameshuntt/bundles/blob/a3c2ff765d2ba87118fb2177220f0c7fdebf97cd/src/unbundler.rs#L48-L50)), so a note never lands in a file.
  Test: `notes_sit_between_entries_and_never_land_in_a_file`.
- Redaction: `Redact::Whole` or `Redact::Lines` replaces the file or each
  window with one marker line,
  `# redacted level=<level> lines=<a>-<b>|all bytes=<n> digest=sha256:<hex>`,
  the digest over the exact bytes left out. Declared once in
  `src/redaction.rs` (`render`, `parse`, `markers_in`)
  ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/redaction.rs#L50-L115)). A borrowed window is hashed in place,
  never copied. Unbundle is unchanged, so a restored file holds the marker,
  never the bytes. Tests:
  `a_redacted_window_is_a_marker_in_the_bundle_and_in_the_restored_file`,
  `a_window_ending_on_a_last_line_without_newline_closes_the_entry`,
  `a_marker_line_reads_back_and_a_near_miss_does_not`.
- A bad entry (empty path, `..` or absolute, a newline in the path, a path
  twice, a window outside the file or over another, a level that is not one
  word) is refused before any of it is written, as the new
  `BundleError::Entry` ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/lib.rs#L57-L58)). A match on
  `BundleError` with no wildcard arm needs that arm. Test:
  `a_bad_entry_is_refused_before_any_of_it_is_written`.
- `bundles bundle --stdin`: exactly the files listed on stdin, in order,
  `path[<TAB>note[<TAB>redact]]` ([new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/cli.rs#L36-L43),
  [new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/cli.rs#L56-L83), [new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/src/main.rs#L39-L61)). Test:
  `the_program_bundles_the_list_on_stdin`.
- The program and its clap and `installation` dependencies are the `bin`
  feature, on by default so `cargo install bundles` installs what it did
  ([old](https://github.com/jameshuntt/bundles/blob/a3c2ff765d2ba87118fb2177220f0c7fdebf97cd/Cargo.toml#L14-L25), [new](https://github.com/jameshuntt/bundles/blob/6534aac2f02b15b7e94c58aa8f1a032a1b6f1aa5/Cargo.toml#L14-L31)). A library user
  sets `default-features = false` and gets neither. `bundles::cli` is under
  the same feature. Nothing removed.

Proof: `cargo test` 13 passed (4 old, 9 new); `cargo test
--no-default-features` 12 passed; `cargo clippy --all-targets` clean.
