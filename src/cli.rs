use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Generic multi-language bundler and unbundler
#[derive(Parser)]
#[command(name = "bundlr", version, about = "Bundle or unbundle any file type easily")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Install,
    
    /// Bundle files of specific type(s) from a directory into a single file.
    Bundle {
        #[arg(short, long)]
        root_dir: PathBuf,
        #[arg(short, long)]
        output_file: PathBuf,
        #[arg(short, long, default_value = "*")]
        extensions: String,
        #[arg(short = 'c', long, default_value = "#")]
        comment_prefix: String,
        #[arg(long)]
        use_lib: Option<PathBuf>,
    },

    /// Unbundle a previously bundled file
    Unbundle {
        #[arg(short, long)]
        bundled_file: PathBuf,
        #[arg(short, long)]
        output_dir: PathBuf,
        #[arg(short = 'c', long, default_value = "#")]
        comment_prefix: String,
    },
}
