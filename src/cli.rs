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
        #[arg(long)]
        use_lib: Option<PathBuf>,
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
