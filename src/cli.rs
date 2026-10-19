use crate::redaction::{LineRedaction, Redact};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Bundle a source tree into one annotated file, and back.
#[derive(Parser, Debug)]
#[command(name = "bundles", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Build this crate in release mode and install the `bundles` binary for
    /// the current user (plan is printed; nothing is written with --dry-run).
    Install {
        #[arg(long)]
        dry_run: bool,
    },
    /// Bundle files under a directory into one file.
    Bundle {
        #[arg(short, long)]
        root_dir: PathBuf,
        #[arg(short, long)]
        output_file: PathBuf,
        /// Extension to include (repeatable); none means every text file.
        #[arg(short, long = "ext")]
        extensions: Vec<String>,
        #[arg(short = 'c', long, default_value = "#")]
        comment_prefix: String,
        /// Directory name to skip anywhere (repeatable; adds to target, .git, node_modules).
        #[arg(long = "ignore")]
        ignore_dirs: Vec<String>,
        /// Bundle only the Rust modules reachable from this entry file.
        #[arg(long, conflicts_with = "stdin")]
        use_lib: Option<PathBuf>,
        /// Bundle exactly the files listed on stdin, in that order, one per
        /// line, relative to --root-dir: `path`, `path<TAB>note`, or
        /// `path<TAB>note<TAB>redact` where redact is `level` (the whole file)
        /// or `level:12-30,40-44` (those lines). An empty note is no note.
        #[arg(long)]
        stdin: bool,
    },
    /// Recreate the files of a bundle under a directory.
    Unbundle {
        #[arg(short, long)]
        bundled_file: PathBuf,
        #[arg(short, long)]
        output_dir: PathBuf,
        #[arg(short = 'c', long, default_value = "#")]
        comment_prefix: String,
    },
}

/// One line of a `bundle --stdin` list: the path, its note, its redaction.
pub fn parse_list_line(line: &str) -> Result<(String, Option<String>, Redact), String> {
    let mut cols = line.split('\t');
    let path = cols.next().unwrap_or_default().trim().to_string();
    if path.is_empty() {
        return Err(format!("`{line}`: no path"));
    }
    let note = cols.next().filter(|n| !n.is_empty()).map(str::to_string);
    let redact = match cols.next().map(str::trim).filter(|r| !r.is_empty()) {
        None => Redact::Keep,
        Some(r) => match r.split_once(':') {
            None => Redact::Whole { level: r.to_string() },
            Some((level, ranges)) => {
                let mut ws = Vec::new();
                for range in ranges.split(',') {
                    let (a, b) = range.split_once('-').ok_or_else(|| format!("`{range}`: a window is `start-end`"))?;
                    let (start, end) = (a.trim().parse().map_err(|_| format!("`{a}`: not a line"))?, b.trim().parse().map_err(|_| format!("`{b}`: not a line"))?);
                    ws.push(LineRedaction { start, end, level: level.to_string() });
                }
                Redact::Lines(ws)
            }
        },
    };
    if cols.next().is_some() {
        return Err(format!("`{line}`: at most three columns: path, note, redact"));
    }
    Ok((path, note, redact))
}
