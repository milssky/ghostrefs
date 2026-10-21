use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::language::Language;

/// Find supported source files without reading or parsing their contents.
pub fn scan_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(path)? {
        let path = entry?.path();

        if path.is_dir() {
            let excluded = path.file_name().is_some_and(|name| {
                matches!(
                    name.to_str(),
                    Some(".git" | ".venv" | "venv" | "__pycache__")
                )
            });
            if !path.is_symlink() && !excluded {
                files.extend(scan_dir(&path)?);
            }
        } else if path.is_file() && Language::from_path(&path).is_some() {
            files.push(path.canonicalize()?);
        }
    }

    Ok(files)
}
