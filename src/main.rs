mod cli;
mod bundler;
mod unbundler;
mod module_tree;
// mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use installation::{InstallOptions, build_and_install};
use std::io;

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install => {
            let _ = build_and_install(InstallOptions::default("bundler"))
                .expect("install failed");
        }
        Commands::Bundle {
            root_dir,
            output_file,
            extensions,
            comment_prefix,
            use_lib,
        } => {
            let exts: Vec<String> =
                extensions.split(',').map(|s| s.trim().to_string()).collect();

            if let Some(lib_path) = use_lib {
                let tree = module_tree::resolver::resolve_module_tree(&lib_path);
                let mut files = Vec::new();
                module_tree::resolver::flatten_tree(&tree, &mut files);
                bundler::bundle_only_used(&root_dir, &output_file, &exts, &comment_prefix, &files)?;
            } else {
                bundler::bundle_files(&root_dir, &output_file, &exts, &comment_prefix)?;
            }
        }

        Commands::Unbundle {
            bundled_file,
            output_dir,
            comment_prefix,
        } => {
            unbundler::unbundle_files(&bundled_file, &output_dir, &comment_prefix)?;
        }
    }

    Ok(())
}
