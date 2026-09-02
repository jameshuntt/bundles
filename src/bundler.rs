use crate::{io_at, BundleError};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// How to bundle.
#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// Extensions to include, lowercase, without the dot; empty means every
    /// text file.
    pub extensions: Vec<String>,
    /// Marker prefix, e.g. `#` or `//`.
    pub comment_prefix: String,
    /// Directory names skipped anywhere in the tree.
    pub ignore_dirs: Vec<String>,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            comment_prefix: "#".to_string(),
            ignore_dirs: ["target", ".git", "node_modules"].iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// What a bundle run did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BundleReport {
    pub included: Vec<PathBuf>,
    /// Files skipped because they are not UTF-8 text.
    pub skipped_binary: Vec<PathBuf>,
}

/// Bundle every matching text file under `root_dir` into `output_file`.
pub fn bundle_files(root_dir: &Path, output_file: &Path, opts: &BundleOptions) -> Result<BundleReport, BundleError> {
    let root = root_dir.canonicalize().map_err(io_at(root_dir))?;
    let mut files: Vec<PathBuf> = Vec::new();
    let walker = WalkDir::new(&root).sort_by_file_name().into_iter().filter_entry(|e| {
        !(e.file_type().is_dir() && e.depth() > 0 && opts.ignore_dirs.iter().any(|d| e.file_name() == d.as_str()))
    });
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext_ok = opts.extensions.is_empty()
            || path.extension().and_then(|e| e.to_str()).map(|e| opts.extensions.iter().any(|x| x.eq_ignore_ascii_case(e))).unwrap_or(false);
        if ext_ok {
            files.push(path.to_path_buf());
        }
    }
    write_bundle(&root, output_file, &files, &opts.comment_prefix)
}

/// Bundle exactly `used_files` (e.g. a crate's module tree), in the order given.
pub fn bundle_only_used(root_dir: &Path, output_file: &Path, comment_prefix: &str, used_files: &[PathBuf]) -> Result<BundleReport, BundleError> {
    let root = root_dir.canonicalize().map_err(io_at(root_dir))?;
    let mut files = Vec::new();
    for f in used_files {
        files.push(f.canonicalize().map_err(io_at(f))?);
    }
    write_bundle(&root, output_file, &files, comment_prefix)
}

fn write_bundle(root: &Path, output_file: &Path, files: &[PathBuf], prefix: &str) -> Result<BundleReport, BundleError> {
    if prefix.trim().is_empty() {
        return Err(BundleError::EmptyPrefix);
    }
    let start_re = crate::unbundler::marker_regex(prefix, "start");
    let end_re = crate::unbundler::marker_regex(prefix, "end");
    let mut report = BundleReport::default();
    let mut out = File::create(output_file).map_err(io_at(output_file))?;
    for path in files {
        let rel = path.strip_prefix(root).map_err(|_| BundleError::OutsideRoot { path: path.clone(), root: root.to_path_buf() })?;
        let rel_str = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        let bytes = fs::read(path).map_err(io_at(path))?;
        let Ok(content) = String::from_utf8(bytes) else {
            report.skipped_binary.push(rel.to_path_buf());
            continue;
        };
        for (i, line) in content.lines().enumerate() {
            if start_re.is_match(line) || end_re.is_match(line) {
                return Err(BundleError::MarkerInContent { path: path.clone(), line: i + 1 });
            }
        }
        writeln!(out, "{prefix} start of {rel_str}").map_err(io_at(output_file))?;
        out.write_all(content.as_bytes()).map_err(io_at(output_file))?;
        if !content.ends_with('\n') {
            writeln!(out).map_err(io_at(output_file))?;
        }
        writeln!(out, "{prefix} end of {rel_str}").map_err(io_at(output_file))?;
        writeln!(out).map_err(io_at(output_file))?;
        report.included.push(rel.to_path_buf());
    }
    Ok(report)
}
