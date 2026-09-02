//! Round trips in temporary directories, the rules that keep bundles honest,
//! and the Rust module walker on both layouts.

use bundles::{bundle_files, bundle_only_used, flatten_tree, resolve_module_tree, unbundle_files, BundleError, BundleOptions, NodeKind};
use std::fs;
use std::path::Path;

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

#[test]
fn round_trip_skips_ignored_dirs_and_binary_files() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write(&src.join("a.txt"), "alpha\n");
    write(&src.join("nested/b.md"), "# beta\nno trailing newline");
    write(&src.join("target/junk.txt"), "should be skipped\n");
    write(&src.join(".git/config"), "skipped too\n");
    fs::write(src.join("blob.bin"), [0u8, 159, 146, 150, 255]).unwrap();

    let bundle = tmp.path().join("out.bundle");
    let report = bundle_files(&src, &bundle, &BundleOptions::default()).unwrap();
    let names: Vec<String> = report.included.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["a.txt", "nested/b.md"], "{names:?}");
    assert_eq!(report.skipped_binary.len(), 1);

    let out = tmp.path().join("restored");
    let r = unbundle_files(&bundle, &out, "#").unwrap();
    assert_eq!(r.written.len(), 2);
    assert_eq!(fs::read_to_string(out.join("a.txt")).unwrap(), "alpha\n");
    assert_eq!(fs::read_to_string(out.join("nested/b.md")).unwrap(), "# beta\nno trailing newline\n", "a final newline is added");
    assert!(!out.join("target").exists());
}

#[test]
fn extension_filter_and_marker_in_content_are_honoured() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write(&src.join("keep.rs"), "fn a() {}\n");
    write(&src.join("drop.txt"), "text\n");
    let opts = BundleOptions { extensions: vec!["rs".into()], ..Default::default() };
    let report = bundle_files(&src, &tmp.path().join("b"), &opts).unwrap();
    assert_eq!(report.included.len(), 1);

    write(&src.join("evil.rs"), "// fine\n# end of keep.rs\n");
    let err = bundle_files(&src, &tmp.path().join("b2"), &BundleOptions::default()).unwrap_err();
    assert!(matches!(err, BundleError::MarkerInContent { line: 2, .. }), "{err}");
}

#[test]
fn unbundle_refuses_paths_that_escape_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = tmp.path().join("evil.bundle");
    fs::write(&bundle, "# start of ok.txt\nfine\n# end of ok.txt\n\n# start of ../escape.txt\npwned\n# end of ../escape.txt\n").unwrap();
    let out = tmp.path().join("out");
    let err = unbundle_files(&bundle, &out, "#").unwrap_err();
    assert!(matches!(err, BundleError::Escape { line: 5, .. }), "{err}");
    assert!(!out.join("ok.txt").exists(), "validation happens before any write");
    assert!(!tmp.path().join("escape.txt").exists());

    fs::write(&bundle, "# start of /etc/absolute\nx\n# end of /etc/absolute\n").unwrap();
    assert!(matches!(unbundle_files(&bundle, &out, "#").unwrap_err(), BundleError::Escape { .. }));

    fs::write(&bundle, "# start of a.txt\nx\n# end of b.txt\n").unwrap();
    assert!(matches!(unbundle_files(&bundle, &out, "#").unwrap_err(), BundleError::Mismatched { .. }));
    fs::write(&bundle, "# start of a.txt\nx\n").unwrap();
    assert!(matches!(unbundle_files(&bundle, &out, "#").unwrap_err(), BundleError::Unterminated(_)));
}

#[test]
fn module_tree_follows_both_layouts_and_path_attrs() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write(&src.join("lib.rs"), "mod flat;\nmod folder;\n#[path = \"odd_name.rs\"]\nmod renamed;\nmod missing;\nmod inline { mod deep; }\n");
    write(&src.join("flat.rs"), "mod child;\n"); // 2018 layout: child in flat/
    write(&src.join("flat/child.rs"), "pub fn c() {}\n");
    write(&src.join("folder/mod.rs"), "mod sibling;\n");
    write(&src.join("folder/sibling.rs"), "pub fn s() {}\n");
    write(&src.join("odd_name.rs"), "pub fn r() {}\n");
    write(&src.join("inline/deep.rs"), "pub fn d() {}\n");

    let tree = resolve_module_tree(&src.join("lib.rs")).unwrap();
    let mut files = Vec::new();
    flatten_tree(&tree, &mut files);
    let rel: Vec<String> = files.iter().map(|f| f.strip_prefix(&src).unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(rel, vec!["flat.rs", "flat/child.rs", "folder/mod.rs", "folder/sibling.rs", "odd_name.rs", "inline/deep.rs"], "{rel:?}");
    assert_eq!(tree[0].kind, NodeKind::Module);
    assert_eq!(tree[1].kind, NodeKind::FolderModule);
    assert_eq!(tree[2].kind, NodeKind::PathAttr);
    assert_eq!(tree[0].sha256.len(), 64);

    write(&src.join("broken.rs"), "fn {");
    assert!(matches!(resolve_module_tree(&src.join("broken.rs")).unwrap_err(), BundleError::Parse { .. }));

    // --use-lib style: bundle only the tree, relative to the crate root
    let mut only = vec![src.join("lib.rs")];
    only.extend(files);
    let bundle = tmp.path().join("lib.bundle");
    let report = bundle_only_used(tmp.path(), &bundle, "//", &only).unwrap();
    assert_eq!(report.included.len(), 7);
    let out = tmp.path().join("restored");
    unbundle_files(&bundle, &out, "//").unwrap();
    assert_eq!(fs::read_to_string(out.join("src/flat/child.rs")).unwrap(), "pub fn c() {}\n");
}
