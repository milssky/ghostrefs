use crate::Result;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{language, test_paths::TestPaths};

/// Include workspace consumers and any source/test directories outside it.
pub fn scan_workspace(root: &Path, source: &Path, tests: &TestPaths) -> Result<Vec<PathBuf>> {
    let mut files = scan_dir(root)?;
    if !source.starts_with(root) {
        files.extend(scan_dir(source)?);
    }
    for path in tests.paths().iter().filter(|path| !path.starts_with(root)) {
        files.extend(scan_test_path(path)?);
    }
    if !files
        .iter()
        .any(|path| path.starts_with(source) && !tests.contains(path))
    {
        return Err("no supported production source files found".into());
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Find supported source files without reading or parsing their contents.
pub fn scan_dir(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for directory_entry in fs::read_dir(directory)? {
        let path = directory_entry?.path();

        if path.is_dir() {
            if !path.is_symlink() && !excludes_directory(&path) {
                files.extend(scan_dir(&path)?);
            }
        } else if path.is_file() && language::supports_path(&path) {
            files.push(path.canonicalize()?);
        } else {
            // Unsupported files are outside the language scan.
        }
    }

    Ok(files)
}

/// Shared traversal exclusions for source scanning and test glob expansion.
pub fn excludes_directory(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == ".git" || name == "target")
        || language::excludes_directory(path)
}

fn scan_test_path(path: &Path) -> io::Result<Vec<PathBuf>> {
    if path.is_dir() {
        return scan_dir(path);
    }
    if language::supports_path(path) {
        return Ok(vec![path.to_path_buf()]);
    }
    Ok(Vec::new())
}
