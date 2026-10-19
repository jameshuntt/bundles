//! Redaction: what a bundle writes in place of lines it must not carry.
//!
//! A redacted file, or a window of lines in one, is written as one marker line
//! inside the file's entry:
//!
//! ```text
//! # redacted level=secret lines=12-30 bytes=640 digest=sha256:<64 hex>
//! # redacted level=secret lines=all bytes=812 digest=sha256:<64 hex>
//! ```
//!
//! `level` is the level that redacted it, as the caller names it. `lines` is the
//! window in the original file (1-based, inclusive), or `all` for the whole
//! file. `bytes` and `digest` are over the exact bytes taken out, line endings
//! included: for `lines=all` that is the file's digest. Unbundling is
//! unchanged, so a restored file holds the marker line, never the bytes.
//!
//! The format is declared here once: [`RedactionMarker::render`] writes it and
//! [`RedactionMarker::parse`] reads it back.

use sha2::{Digest, Sha256};

/// How much of an entry a bundle leaves out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Redact {
    /// Everything is written.
    #[default]
    Keep,
    /// The whole file is replaced by one marker.
    Whole { level: String },
    /// Each window is replaced by one marker; the rest is written.
    Lines(Vec<LineRedaction>),
}

/// One window of lines to leave out: 1-based, inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRedaction {
    pub start: usize,
    pub end: usize,
    pub level: String,
}

/// Which lines a marker stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// The whole file.
    All,
    /// Lines `start..=end` of the original file.
    Lines { start: usize, end: usize },
}

/// One marker line, as written and as read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionMarker {
    pub level: String,
    pub span: Span,
    /// How many bytes were taken out.
    pub bytes: usize,
    /// `sha256:<hex>` of the bytes taken out.
    pub digest: String,
}

/// The digest a marker carries: `sha256:` and the lowercase hex of the bytes.
pub fn digest_of(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A level is one word: ASCII letters, digits, `_`, `-`, `.`.
pub(crate) fn level_ok(level: &str) -> bool {
    !level.is_empty() && level.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

impl RedactionMarker {
    /// The marker for `taken`, the exact bytes left out.
    pub fn over(level: &str, span: Span, taken: &[u8]) -> Self {
        Self { level: level.to_string(), span, bytes: taken.len(), digest: digest_of(taken) }
    }

    /// The marker line, without its newline.
    pub fn render(&self, prefix: &str) -> String {
        let lines = match self.span {
            Span::All => "all".to_string(),
            Span::Lines { start, end } => format!("{start}-{end}"),
        };
        format!("{prefix} redacted level={} lines={lines} bytes={} digest={}", self.level, self.bytes, self.digest)
    }

    /// Read a marker line written with `prefix`; `None` when the line is not one.
    pub fn parse(prefix: &str, line: &str) -> Option<Self> {
        let rest = line.strip_prefix(prefix)?.trim_start().strip_prefix("redacted ")?;
        let mut fields = rest.split(' ');
        let level = fields.next()?.strip_prefix("level=")?;
        let lines = fields.next()?.strip_prefix("lines=")?;
        let bytes = fields.next()?.strip_prefix("bytes=")?.parse().ok()?;
        let digest = fields.next()?.strip_prefix("digest=")?;
        if fields.next().is_some() || !level_ok(level) {
            return None;
        }
        let hex = digest.strip_prefix("sha256:")?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let span = if lines == "all" {
            Span::All
        } else {
            let (a, b) = lines.split_once('-')?;
            Span::Lines { start: a.parse().ok()?, end: b.parse().ok()? }
        };
        Some(Self { level: level.to_string(), span, bytes, digest: digest.to_string() })
    }
}

/// Every marker line in a text written with `prefix`, in order.
pub fn markers_in(text: &str, prefix: &str) -> Vec<RedactionMarker> {
    text.lines().filter_map(|l| RedactionMarker::parse(prefix, l)).collect()
}
