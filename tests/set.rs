//! The explicit file set: entries in the caller's order from bytes or a
//! reader, notes between entries, redaction markers in place of left-out
//! lines, and the `--stdin` list of the program.

use bundles::redaction::{digest_of, markers_in, LineRedaction, Redact, RedactionMarker, Span};
use bundles::{bundle_files, bundle_set, unbundle_files, BundleError, BundleOptions, BundleWriter, FileEntry, Source};
use std::fs;

const SECRET_FN: &str = "fn key() -> &'static str {\n    \"hunter2\"\n}\n";

fn lib_rs() -> String {
    format!("pub fn open() {{}}\n\n{SECRET_FN}\npub fn close() {{}}\n")
}

#[test]
fn a_set_writes_the_same_bytes_bundle_files_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("a.txt"), "alpha\n").unwrap();
    fs::write(src.join("nested/b.md"), "# beta\nno trailing newline").unwrap();
    fs::write(src.join("empty.txt"), "").unwrap();
    let walked = tmp.path().join("walked.bundle");
    bundle_files(&src, &walked, &BundleOptions::default()).unwrap();

    let a = fs::read(src.join("a.txt")).unwrap();
    let set = tmp.path().join("set.bundle");
    let report = bundle_set(
        [
            FileEntry::bytes("a.txt", &a),
            FileEntry { path: "empty.txt".into(), source: Source::Reader(Box::new(std::io::empty())), note: None, redact: Redact::Keep },
            FileEntry { path: "nested/b.md".into(), source: Source::Bytes(fs::read(src.join("nested/b.md")).unwrap().into()), note: None, redact: Redact::Keep },
        ],
        &set,
        "#",
    )
    .unwrap();
    assert_eq!(report.included, ["a.txt", "empty.txt", "nested/b.md"], "the caller's order");
    assert_eq!(fs::read(&set).unwrap(), fs::read(&walked).unwrap(), "one format: a set of the walked files is the walked bundle");
}

#[test]
fn notes_sit_between_entries_and_never_land_in_a_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("n.bundle");
    bundle_set(
        [
            FileEntry::bytes("z.rs", b"fn z() {}\n").with_note("crate:demo, kept whole\nsecond line"),
            FileEntry::bytes("a.rs", b"fn a() {}\n"),
        ],
        &out,
        "//",
    )
    .unwrap();
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("// note of z.rs: crate:demo, kept whole\n// note of z.rs: second line\n// start of z.rs\n"), "{text}");
    let restored = tmp.path().join("r");
    let r = unbundle_files(&out, &restored, "//").unwrap();
    assert_eq!(r.written.len(), 2);
    assert_eq!(fs::read_to_string(restored.join("z.rs")).unwrap(), "fn z() {}\n");
}

#[test]
fn a_redacted_window_is_a_marker_in_the_bundle_and_in_the_restored_file() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = lib_rs();
    let key = b"-----BEGIN KEY-----\nAAAA\n-----END KEY-----\n";
    let out = tmp.path().join("r.bundle");
    let report = bundle_set(
        [
            FileEntry::bytes("src/lib.rs", lib.as_bytes()).with_redact(Redact::Lines(vec![LineRedaction { start: 3, end: 5, level: "secret".into() }])),
            FileEntry::bytes("keys/dev.pem", key).with_redact(Redact::Whole { level: "key_material".into() }),
        ],
        &out,
        "//",
    )
    .unwrap();
    let text = fs::read_to_string(&out).unwrap();
    assert!(!text.contains("hunter2") && !text.contains("AAAA"), "{text}");

    let window = RedactionMarker { level: "secret".into(), span: Span::Lines { start: 3, end: 5 }, bytes: SECRET_FN.len(), digest: digest_of(SECRET_FN.as_bytes()) };
    let whole = RedactionMarker { level: "key_material".into(), span: Span::All, bytes: key.len(), digest: digest_of(key) };
    assert_eq!(window.render("//"), format!("// redacted level=secret lines=3-5 bytes={} digest={}", SECRET_FN.len(), digest_of(SECRET_FN.as_bytes())));
    assert_eq!(report.redacted.iter().map(|r| (r.path.as_str(), r.marker.clone())).collect::<Vec<_>>(), [("src/lib.rs", window.clone()), ("keys/dev.pem", whole.clone())]);
    assert_eq!(markers_in(&text, "//"), [window.clone(), whole.clone()], "the markers read back from the bundle");

    let restored = tmp.path().join("restored");
    unbundle_files(&out, &restored, "//").unwrap();
    assert_eq!(fs::read_to_string(restored.join("src/lib.rs")).unwrap(), format!("pub fn open() {{}}\n\n{}\n\npub fn close() {{}}\n", window.render("//")));
    assert_eq!(fs::read_to_string(restored.join("keys/dev.pem")).unwrap(), format!("{}\n", whole.render("//")));
}

#[test]
fn a_window_ending_on_a_last_line_without_newline_closes_the_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("t.bundle");
    bundle_set([FileEntry::bytes("t.txt", b"keep\ndrop").with_redact(Redact::Lines(vec![LineRedaction { start: 2, end: 2, level: "secret".into() }]))], &out, "#").unwrap();
    let m = RedactionMarker::over("secret", Span::Lines { start: 2, end: 2 }, b"drop");
    assert_eq!(fs::read_to_string(&out).unwrap(), format!("# start of t.txt\nkeep\n{}\n# end of t.txt\n\n", m.render("#")));
}

#[test]
fn a_bad_entry_is_refused_before_any_of_it_is_written() {
    let refused = |e: FileEntry<'_>| {
        let mut w = BundleWriter::new(Vec::new(), "#").unwrap();
        w.entry(FileEntry::bytes("ok.txt", b"fine\n")).unwrap();
        let before = w.get_ref().len();
        let err = w.entry(e).unwrap_err();
        let (bytes, report) = w.finish().unwrap();
        assert_eq!(bytes.len(), before, "nothing of a refused entry is written");
        assert_eq!(report.included, ["ok.txt"]);
        err
    };
    let reason = |e: BundleError| match e {
        BundleError::Entry { reason, .. } => reason,
        other => panic!("{other}"),
    };
    assert!(reason(refused(FileEntry::bytes("../up.txt", b"x\n"))).contains(".."));
    assert!(matches!(refused(FileEntry::bytes("/etc/passwd", b"x\n")), BundleError::Entry { .. }));
    assert!(matches!(refused(FileEntry::bytes("", b"x\n")), BundleError::Entry { .. }));
    assert!(reason(refused(FileEntry::bytes("a\nb", b"x\n"))).contains("newline"));
    assert!(reason(refused(FileEntry::bytes("ok.txt", b"again\n"))).contains("already"));
    let lines = |ws: &[(usize, usize, &str)]| Redact::Lines(ws.iter().map(|&(start, end, level)| LineRedaction { start, end, level: level.into() }).collect());
    assert!(reason(refused(FileEntry::bytes("w.txt", b"1\n2\n3\n").with_redact(lines(&[(1, 2, "secret"), (2, 3, "secret")])))).contains("overlap"));
    assert!(reason(refused(FileEntry::bytes("w.txt", b"1\n2\n").with_redact(lines(&[(2, 3, "secret")])))).contains("not inside"));
    assert!(reason(refused(FileEntry::bytes("w.txt", b"1\n").with_redact(Redact::Whole { level: "top secret".into() }))).contains("one word"));
    assert!(matches!(refused(FileEntry::bytes("m.txt", b"x\n# end of ok.txt\n")), BundleError::MarkerInContent { line: 2, .. }));
}

#[test]
fn a_marker_looking_line_inside_a_redacted_window_is_left_out_not_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("m.bundle");
    let r = bundle_set([FileEntry::bytes("m.txt", b"x\n# end of m.txt\n").with_redact(Redact::Lines(vec![LineRedaction { start: 2, end: 2, level: "secret".into() }]))], &out, "#").unwrap();
    assert_eq!(r.redacted.len(), 1);
    unbundle_files(&out, &tmp.path().join("r"), "#").unwrap();
}

#[test]
fn binary_entries_are_skipped_and_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let r = bundle_set([FileEntry::bytes("blob.bin", &[0, 159, 146, 150, 255]), FileEntry::bytes("t.txt", b"t\n")], &tmp.path().join("b"), "#").unwrap();
    assert_eq!(r.skipped_binary, ["blob.bin"]);
    assert_eq!(r.included, ["t.txt"]);
}

#[test]
fn a_marker_line_reads_back_and_a_near_miss_does_not() {
    let m = RedactionMarker::over("confidential", Span::Lines { start: 7, end: 9 }, b"abc");
    assert_eq!(RedactionMarker::parse("#", &m.render("#")), Some(m.clone()));
    assert_eq!(RedactionMarker::parse("//", &m.render("#")), None);
    assert_eq!(RedactionMarker::parse("#", "# redacted level=secret lines=all bytes=3 digest=sha256:zz"), None);
    assert_eq!(RedactionMarker::parse("#", &format!("{} extra", m.render("#"))), None);
}

#[cfg(feature = "bin")]
#[test]
fn the_program_bundles_the_list_on_stdin() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("crate");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), lib_rs()).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(root.join("README.md"), "not listed\n").unwrap();
    let out = tmp.path().join("p.bundle");
    let mut child = Command::new(env!("CARGO_BIN_EXE_bundles"))
        .args(["bundle", "-r"])
        .arg(&root)
        .arg("-o")
        .arg(&out)
        .args(["-c", "//", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"Cargo.toml\nsrc/lib.rs\tthe library\tsecret:3-5\n").unwrap();
    let done = child.wait_with_output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stdout));
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("// start of Cargo.toml\n"), "{text}");
    assert!(text.contains("// note of src/lib.rs: the library\n// start of src/lib.rs\n"));
    assert!(!text.contains("README") && !text.contains("hunter2"));
    assert_eq!(markers_in(&text, "//").len(), 1);

    let restored = tmp.path().join("restored");
    let st = Command::new(env!("CARGO_BIN_EXE_bundles")).args(["unbundle", "-b"]).arg(&out).arg("-o").arg(&restored).args(["-c", "//"]).output().unwrap();
    assert!(st.status.success());
    assert!(fs::read_to_string(restored.join("src/lib.rs")).unwrap().contains("// redacted level=secret lines=3-5 "));

    let bad = Command::new(env!("CARGO_BIN_EXE_bundles"))
        .args(["bundle", "-r"])
        .arg(&root)
        .arg("-o")
        .arg(tmp.path().join("bad.bundle"))
        .arg("--stdin")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            c.stdin.take().unwrap().write_all(b"../outside.txt\n").unwrap();
            c.wait_with_output()
        })
        .unwrap();
    assert!(!bad.status.success(), "a listed path that escapes the root is an error");
}
