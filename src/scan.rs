use crate::Result;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::language::Language;

/// Include workspace consumers and any source/test directories outside it.
pub(crate) fn scan_workspace(root: &Path, source: &Path, tests: &Path) -> Result<Vec<PathBuf>> {
    let mut files = scan_dir(root)?;
    if !source.starts_with(root) {
        files.extend(scan_dir(source)?);
    }
    if !tests.starts_with(root) {
        files.extend(scan_dir(tests)?);
    }
    if !files
        .iter()
        .any(|path| path.starts_with(source) && !path.starts_with(tests))
    {
        return Err("no supported production source files found".into());
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Find supported source files without reading or parsing their contents.
pub(crate) fn scan_dir(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for directory_entry in fs::read_dir(directory)? {
        let path = directory_entry?.path();

        if path.is_dir() {
            let excluded = path.file_name().is_some_and(|name| {
                matches!(
                    name.to_str(),
                    // TODO: Looks lile shit.
                    Some(".git" | ".venv" | "venv" | "__pycache__" | "target")
                )
            });
            if !path.is_symlink() && !excluded {
                files.extend(scan_dir(&path)?);
            }
        } else if path.is_file() && Language::from_path(&path).is_some() {
            files.push(path.canonicalize()?);
        } else {
            // Unsupported files are outside the language scan.
        }
    }

    Ok(files)
}
