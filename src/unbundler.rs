use colored::*;
use regex::Regex;
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::Path,
};

pub fn unbundle_files(
    bundled_file: &Path,
    output_dir: &Path,
    comment_prefix: &str,
) -> io::Result<()> {
    let start_re =
        Regex::new(&format!(r"^{}\s*start of (.+)$", regex::escape(comment_prefix))).unwrap();
    let end_re =
        Regex::new(&format!(r"^{}\s*end of (.+)$", regex::escape(comment_prefix))).unwrap();

    let file = File::open(bundled_file)?;
    let reader = BufReader::new(file);
    let mut current_path: Option<std::path::PathBuf> = None;
    let mut buffer = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Some(cap) = start_re.captures(&line) {
            current_path = Some(output_dir.join(cap[1].trim()));
            buffer.clear();
        } else if end_re.is_match(&line) {
            if let Some(path) = current_path.take() {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = File::create(&path)?;
                for l in &buffer {
                    writeln!(out, "{}", l)?;
                }
                println!(
                    "{} {}",
                    "✅ Wrote:".bright_blue(),
                    path.display().to_string().bright_white()
                );
                buffer.clear();
            }
        } else if current_path.is_some() {
            buffer.push(line);
        }
    }

    println!(
        "\n{} {}",
        "🎉 Unbundling complete →".bright_green().bold(),
        output_dir.display().to_string().bright_yellow()
    );
    Ok(())
}
