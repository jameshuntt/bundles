use bundles::cli::parse_list_line;
use bundles::{bundle_files, bundle_only_used, bundle_set, flatten_tree, resolve_module_tree, unbundle_files, BundleOptions, Cli, Commands, FileEntry, Source};
use clap::Parser;
use installation::{build_binary, Installer, PathHook, Scope};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install { dry_run } => {
            let bin = env!("CARGO_PKG_NAME");
            let cwd = std::env::current_dir()?;
            let built = build_binary(&cwd, bin, true)?;
            let installer = Installer::new(bin, Scope::User)?;
            let plan = installer.plan(bin, &built)?;
            print!("{}", plan.describe());
            if dry_run {
                println!("dry run: nothing written");
                return Ok(());
            }
            let receipt = installer.install(&plan)?;
            println!("installed {}", receipt.destination.display());
            match installer.ensure_path_hook(None)? {
                PathHook::AlreadyOnPath => {}
                PathHook::Present { rc_file } => println!("PATH hook already in {}", rc_file.display()),
                PathHook::Added { rc_file } => println!("PATH hook added to {} (open a new shell)", rc_file.display()),
            }
        }
        Commands::Bundle { root_dir, output_file, comment_prefix, stdin: true, .. } => {
            let mut entries = Vec::new();
            for line in std::io::stdin().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let (path, note, redact) = parse_list_line(&line)?;
                let file = pathgen::checked_join(&root_dir, &path)?;
                let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                entries.push(FileEntry { path, source: Source::Bytes(bytes.into()), note, redact });
            }
            let report = bundle_set(entries, &output_file, &comment_prefix)?;
            for f in &report.included {
                println!("added   {f}");
            }
            for f in &report.skipped_binary {
                println!("skipped {f} (not text)");
            }
            for r in &report.redacted {
                println!("redacted {} ({})", r.path, r.marker.render(&comment_prefix));
            }
            println!("bundle complete: {} files -> {}", report.included.len(), output_file.display());
        }
        Commands::Bundle { root_dir, output_file, extensions, comment_prefix, ignore_dirs, use_lib, .. } => {
            let mut opts = BundleOptions { extensions, comment_prefix, ..Default::default() };
            opts.ignore_dirs.extend(ignore_dirs);
            let report = if let Some(entry) = use_lib {
                let tree = resolve_module_tree(&entry)?;
                let mut files = vec![entry.canonicalize()?];
                flatten_tree(&tree, &mut files);
                bundle_only_used(&root_dir, &output_file, &opts.comment_prefix, &files)?
            } else {
                bundle_files(&root_dir, &output_file, &opts)?
            };
            for f in &report.included {
                println!("added   {}", f.display());
            }
            for f in &report.skipped_binary {
                println!("skipped {} (not text)", f.display());
            }
            println!("bundle complete: {} files -> {}", report.included.len(), output_file.display());
        }
        Commands::Unbundle { bundled_file, output_dir, comment_prefix } => {
            let report = unbundle_files(&bundled_file, &output_dir, &comment_prefix)?;
            for f in &report.written {
                println!("wrote {}", f.display());
            }
            println!("unbundle complete: {} files -> {}", report.written.len(), output_dir.display());
        }
    }
    Ok(())
}
