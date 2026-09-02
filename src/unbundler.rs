use crate::{io_at, BundleError};
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub(crate) fn marker_regex(prefix: &str, kind: &str) -> Regex {
    Regex::new(&format!(r"^{}\s*{} of (.+)$", regex::escape(prefix), kind)).expect("marker regex from an escaped prefix")
}

/// What an unbundle run did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UnbundleReport {
    pub written: Vec<PathBuf>,
}

/// Recreate the files of `bundled_file` under `output_dir`. Every path is
/// checked with `pathgen::checked_join`: `..` and absolute paths are errors,
/// and nothing is written before the whole bundle has been validated.
pub fn unbundle_files(bundled_file: &Path, output_dir: &Path, comment_prefix: &str) -> Result<UnbundleReport, BundleError> {
    if comment_prefix.trim().is_empty() {
        return Err(BundleError::EmptyPrefix);
    }
    let start_re = marker_regex(comment_prefix, "start");
    let end_re = marker_regex(comment_prefix, "end");

    // pass 1: parse and validate every entry
    let file = File::open(bundled_file).map_err(io_at(bundled_file))?;
    let mut entries: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let mut current: Option<(String, PathBuf, Vec<String>)> = None;
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(io_at(bundled_file))?;
        let n = idx + 1;
        if let Some(cap) = start_re.captures(&line) {
            let rel = cap[1].trim().to_string();
            if let Some((open, _, _)) = &current {
                return Err(BundleError::Mismatched { line: n, open: open.clone(), found: format!("start of {rel}") });
            }
            let target = pathgen::checked_join(output_dir, &rel).map_err(|e| BundleError::Escape { line: n, path: rel.clone(), reason: e.to_string() })?;
            current = Some((rel, target, Vec::new()));
        } else if let Some(cap) = end_re.captures(&line) {
            let rel = cap[1].trim().to_string();
            match current.take() {
                Some((open, target, buf)) if open == rel => entries.push((target, buf)),
                Some((open, _, _)) => return Err(BundleError::Mismatched { line: n, open, found: rel }),
                None => return Err(BundleError::Mismatched { line: n, open: String::new(), found: rel }),
            }
        } else if let Some((_, _, buf)) = &mut current {
            buf.push(line);
        }
    }
    if let Some((open, _, _)) = current {
        return Err(BundleError::Unterminated(open));
    }

    // pass 2: write
    let mut report = UnbundleReport::default();
    for (target, lines) in entries {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_at(parent))?;
        }
        let mut out = File::create(&target).map_err(io_at(&target))?;
        for l in &lines {
            out.write_all(l.as_bytes()).map_err(io_at(&target))?;
            out.write_all(b"\n").map_err(io_at(&target))?;
        }
        report.written.push(target);
    }
    Ok(report)
}
