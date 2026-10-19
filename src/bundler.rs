use crate::redaction::{level_ok, Redact, RedactionMarker, Span};
use crate::{io_at, BundleError};
use regex::Regex;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
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
        let markers = Markers { prefix, start: &start_re, end: &end_re };
        write_entry(&mut out, output_file, &markers, &rel_str, &content, path, &[])?;
        report.included.push(rel.to_path_buf());
    }
    Ok(report)
}

/// The marker lines of one bundle, built once.
struct Markers<'a> {
    prefix: &'a str,
    start: &'a Regex,
    end: &'a Regex,
}

/// One entry: the start marker, the content (each window in `windows`
/// replaced by its redaction marker), the end marker, a blank line. A line
/// that would be written and looks like a start or end marker is refused
/// before anything of the entry is written. `windows` are sorted, inside the
/// file and apart; `[(0, 0, level)]` stands for the whole file. A file
/// without a final newline gets one. Returns the markers written.
fn write_entry(
    out: &mut impl Write,
    out_path: &Path,
    m: &Markers<'_>,
    rel_str: &str,
    content: &str,
    err_path: &Path,
    windows: &[(usize, usize, &str)],
) -> Result<Vec<RedactionMarker>, BundleError> {
    let prefix = m.prefix;
    let mut written = Vec::new();
    if let [(0, 0, level)] = windows {
        writeln!(out, "{prefix} start of {rel_str}").map_err(io_at(out_path))?;
        let marker = RedactionMarker::over(level, Span::All, content.as_bytes());
        writeln!(out, "{}", marker.render(prefix)).map_err(io_at(out_path))?;
        written.push(marker);
    } else {
        // each line with its ending, and where it starts in `content`
        let mut lines: Vec<(usize, &str)> = Vec::new();
        let mut at = 0;
        for line in content.split_inclusive('\n') {
            lines.push((at, line));
            at += line.len();
        }
        let in_window = |n: usize| windows.iter().any(|&(start, end, _)| n >= start && n <= end);
        for (i, (_, line)) in lines.iter().enumerate().filter(|(i, _)| !in_window(i + 1)) {
            let bare = line.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(line);
            if m.start.is_match(bare) || m.end.is_match(bare) {
                return Err(BundleError::MarkerInContent { path: err_path.to_path_buf(), line: i + 1 });
            }
        }
        writeln!(out, "{prefix} start of {rel_str}").map_err(io_at(out_path))?;
        let mut closed = false;
        let mut n = 0;
        while n < lines.len() {
            if let Some(&(start, end, level)) = windows.iter().find(|w| w.0 == n + 1) {
                // the window's bytes are a slice of the caller's content: hashed, never copied
                let (from, (last_at, last)) = (lines[start - 1].0, lines[end - 1]);
                let marker = RedactionMarker::over(level, Span::Lines { start, end }, &content.as_bytes()[from..last_at + last.len()]);
                writeln!(out, "{}", marker.render(prefix)).map_err(io_at(out_path))?;
                written.push(marker);
                closed = true;
                n = end;
                continue;
            }
            let line = lines[n].1;
            out.write_all(line.as_bytes()).map_err(io_at(out_path))?;
            closed = line.ends_with('\n');
            n += 1;
        }
        if !closed {
            writeln!(out).map_err(io_at(out_path))?;
        }
    }
    writeln!(out, "{prefix} end of {rel_str}").map_err(io_at(out_path))?;
    writeln!(out).map_err(io_at(out_path))?;
    Ok(written)
}

// ---------------------------------------------------------------------------
// an explicit file set

/// Where an entry's bytes come from.
pub enum Source<'a> {
    /// Bytes in hand, borrowed or owned. Borrowed bytes are not copied: a
    /// redacted window is hashed and left out without leaving the caller's buffer.
    Bytes(Cow<'a, [u8]>),
    /// Read to the end when the entry is written.
    Reader(Box<dyn Read + 'a>),
}

impl std::fmt::Debug for Source<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Bytes(b) => write!(f, "Bytes({} bytes)", b.len()),
            Source::Reader(_) => f.write_str("Reader"),
        }
    }
}

/// One file of an explicit set.
#[derive(Debug)]
pub struct FileEntry<'a> {
    /// Where the file lands on unbundle: relative, `/`-separated, no `..`.
    pub path: String,
    pub source: Source<'a>,
    /// Written as `<prefix> note of <path>: <line>` lines just before the
    /// entry. Unbundling skips lines between entries, so a note never lands
    /// in a restored file.
    pub note: Option<String>,
    /// What to leave out (see [`crate::redaction`]).
    pub redact: Redact,
}

impl<'a> FileEntry<'a> {
    /// An entry of borrowed bytes, kept whole, with no note.
    pub fn bytes(path: impl Into<String>, bytes: &'a [u8]) -> Self {
        Self { path: path.into(), source: Source::Bytes(Cow::Borrowed(bytes)), note: None, redact: Redact::Keep }
    }

    /// The same entry with a note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The same entry with a redaction.
    pub fn with_redact(mut self, redact: Redact) -> Self {
        self.redact = redact;
        self
    }
}

/// One redaction a set bundle wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    pub path: String,
    pub marker: RedactionMarker,
}

/// What a set bundle did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SetReport {
    /// Entry paths written, in order.
    pub included: Vec<String>,
    /// Entries skipped because they are not UTF-8 text.
    pub skipped_binary: Vec<String>,
    /// Every marker written in place of left-out bytes.
    pub redacted: Vec<Redacted>,
}

/// Writes an explicit file set, entry by entry, in the same format as
/// [`bundle_files`]: the order is the caller's, nothing is walked or read
/// from disk by the writer.
pub struct BundleWriter<W: Write> {
    out: W,
    prefix: String,
    start: Regex,
    end: Regex,
    seen: BTreeSet<String>,
    report: SetReport,
}

impl<W: Write> BundleWriter<W> {
    pub fn new(out: W, comment_prefix: &str) -> Result<Self, BundleError> {
        if comment_prefix.trim().is_empty() {
            return Err(BundleError::EmptyPrefix);
        }
        Ok(Self {
            out,
            prefix: comment_prefix.to_string(),
            start: crate::unbundler::marker_regex(comment_prefix, "start"),
            end: crate::unbundler::marker_regex(comment_prefix, "end"),
            seen: BTreeSet::new(),
            report: SetReport::default(),
        })
    }

    /// Write one entry. A path that is empty, escapes (`..`, absolute), holds
    /// a newline or comes twice, a note line or a level that cannot be
    /// written, and a window outside the file or overlapping another are
    /// refused before anything of the entry is written.
    pub fn entry(&mut self, entry: FileEntry<'_>) -> Result<(), BundleError> {
        let FileEntry { path, source, note, redact } = entry;
        let refuse = |reason: String| BundleError::Entry { path: path.clone(), reason };
        if path.contains('\n') || path.contains('\r') || path.contains('\\') {
            return Err(refuse("a path holds no newline and no backslash".into()));
        }
        pathgen::checked_join(Path::new("."), &path).map_err(|e| refuse(e.to_string()))?;
        if self.seen.contains(&path) {
            return Err(refuse("the path is already in this bundle".into()));
        }
        let bytes: Cow<'_, [u8]> = match source {
            Source::Bytes(b) => b,
            Source::Reader(mut r) => {
                let mut v = Vec::new();
                r.read_to_end(&mut v).map_err(io_at(Path::new(&path)))?;
                Cow::Owned(v)
            }
        };
        let Ok(content) = std::str::from_utf8(&bytes) else {
            self.seen.insert(path.clone());
            self.report.skipped_binary.push(path);
            return Ok(());
        };
        let line_count = content.split_inclusive('\n').count();
        let windows: Vec<(usize, usize, &str)> = match &redact {
            Redact::Keep => Vec::new(),
            Redact::Whole { level } => vec![(0, 0, level.as_str())],
            Redact::Lines(ws) => {
                let mut v: Vec<(usize, usize, &str)> = ws.iter().map(|w| (w.start, w.end, w.level.as_str())).collect();
                v.sort();
                v
            }
        };
        let mut last_end = 0;
        for &(start, end, level) in &windows {
            if !level_ok(level) {
                return Err(refuse(format!("level `{level}` is not one word (letters, digits, `_`, `-`, `.`)")));
            }
            if (start, end) == (0, 0) {
                continue;
            }
            if start == 0 || start > end || end > line_count {
                return Err(refuse(format!("lines {start}-{end} are not inside the file's {line_count} lines")));
            }
            if start <= last_end {
                return Err(refuse(format!("lines {start}-{end} overlap the window before them")));
            }
            last_end = end;
        }
        let note_lines: Vec<String> = note.iter().flat_map(|n| n.lines()).map(|l| format!("{} note of {path}: {l}", self.prefix)).collect();
        let m = Markers { prefix: &self.prefix, start: &self.start, end: &self.end };
        // the entry goes to a buffer first, so a refused entry leaves nothing behind
        let mut buf = Vec::new();
        for l in &note_lines {
            writeln!(buf, "{l}").map_err(io_at(Path::new(&path)))?;
        }
        let markers = write_entry(&mut buf, Path::new(&path), &m, &path, content, Path::new(&path), &windows)?;
        self.out.write_all(&buf).map_err(io_at(Path::new(&path)))?;
        self.report.redacted.extend(markers.into_iter().map(|marker| Redacted { path: path.clone(), marker }));
        self.seen.insert(path.clone());
        self.report.included.push(path);
        Ok(())
    }

    /// The writer, as written so far.
    pub fn get_ref(&self) -> &W {
        &self.out
    }

    /// Flush and hand back the writer and what was written.
    pub fn finish(mut self) -> Result<(W, SetReport), BundleError> {
        self.out.flush().map_err(io_at(Path::new("<bundle>")))?;
        Ok((self.out, self.report))
    }
}

/// Bundle exactly `entries`, in order, into `output_file`.
pub fn bundle_set<'a>(entries: impl IntoIterator<Item = FileEntry<'a>>, output_file: &Path, comment_prefix: &str) -> Result<SetReport, BundleError> {
    let mut w = BundleWriter::new(Vec::new(), comment_prefix)?;
    for e in entries {
        w.entry(e)?;
    }
    let (bytes, report) = w.finish()?;
    fs::write(output_file, bytes).map_err(io_at(output_file))?;
    Ok(report)
}
