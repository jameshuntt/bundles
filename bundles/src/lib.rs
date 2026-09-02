//! `bundles` — put a source tree into one file and get it back, without
//! surprises.
//!
//! A bundle is plain text: for every file, a `<prefix> start of <relative
//! path>` line, the file's content, a `<prefix> end of <relative path>`
//! line. Unbundling recreates the tree under a directory you choose.
//!
//! Rules that keep it honest:
//! - `target/`, `.git/`, `node_modules/` (and whatever you add) are skipped.
//! - Only text files are bundled; a binary file is reported and skipped,
//!   never aborts the bundle.
//! - A file whose own content contains a marker line is refused, because
//!   the bundle could not be read back unambiguously.
//! - Every output path of an unbundle goes through `pathgen::checked_join`:
//!   `../` and absolute paths inside a bundle are errors, not writes.
//! - The Rust module walker follows both layouts (`x.rs` + `x/`, `x/mod.rs`)
//!   and `#[path]`, and returns errors instead of panicking.

pub mod bundler;
pub mod cli;
pub mod module_tree;
pub mod unbundler;

pub use bundler::{bundle_files, bundle_only_used, BundleOptions, BundleReport};
pub use cli::{Cli, Commands};
pub use module_tree::{flatten_tree, resolve_module_tree, ModuleNode, NodeKind};
pub use unbundler::{unbundle_files, UnbundleReport};

/// Everything that can go wrong; no panics on user input.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("io error at `{path}`: {source}")]
    Io { path: std::path::PathBuf, #[source] source: std::io::Error },
    #[error("`{path}` is not under the bundle root `{root}`")]
    OutsideRoot { path: std::path::PathBuf, root: std::path::PathBuf },
    #[error("`{path}` line {line} looks like a bundle marker; the bundle could not be read back unambiguously")]
    MarkerInContent { path: std::path::PathBuf, line: usize },
    #[error("bundle line {line}: `{path}` would escape the output directory: {reason}")]
    Escape { line: usize, path: String, reason: String },
    #[error("bundle line {line}: `end of {found}` closes `{open}`")]
    Mismatched { line: usize, open: String, found: String },
    #[error("bundle ended while `{0}` was still open")]
    Unterminated(String),
    #[error("cannot parse `{path}` as Rust: {message}")]
    Parse { path: std::path::PathBuf, message: String },
    #[error("comment prefix must not be empty")]
    EmptyPrefix,
}

pub(crate) fn io_at(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> BundleError + '_ {
    move |source| BundleError::Io { path: path.to_path_buf(), source }
}
