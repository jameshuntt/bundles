use colored::*;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};
use walkdir::WalkDir;

pub fn bundle_files(
    root_dir: &Path,
    output_file: &Path,
    exts: &[String],
    comment_prefix: &str,
) -> io::Result<()> {
    let root_dir = root_dir.canonicalize()?;
    let mut out = File::create(output_file)?;
    let include_all = exts.iter().any(|e| e == "*");

    for entry in WalkDir::new(&root_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext_ok = include_all
            || path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
                .unwrap_or(false);

        if ext_ok {
            let rel_path = path.strip_prefix(&root_dir).unwrap();
            writeln!(out, "{} start of {}", comment_prefix, rel_path.display())?;
            let content = fs::read_to_string(path)?;
            write!(out, "{}", content)?;
            if !content.ends_with('\n') {
                writeln!(out)?;
            }
            writeln!(out, "{} end of {}\n", comment_prefix, rel_path.display())?;

            println!(
                "{} {}",
                "📦 Added:".bright_green(),
                rel_path.display().to_string().bright_white()
            );
        }
    }

    println!(
        "\n{} {}",
        "✅ Bundle complete →".bright_green().bold(),
        output_file.display().to_string().bright_yellow()
    );
    Ok(())
}

pub fn bundle_only_used(
    root_dir: &Path,
    output_file: &Path,
    _exts: &[String],
    comment_prefix: &str,
    used_files: &[std::path::PathBuf],
) -> io::Result<()> {
    let mut out = File::create(output_file)?;
    for path in used_files {
        let content = fs::read_to_string(path)?;
        let rel_path = path.strip_prefix(root_dir).unwrap_or(path);
        writeln!(out, "{} start of {}", comment_prefix, rel_path.display())?;
        write!(out, "{}", content)?;
        if !content.ends_with('\n') {
            writeln!(out)?;
        }
        writeln!(out, "{} end of {}\n", comment_prefix, rel_path.display())?;
    }
    Ok(())
}
