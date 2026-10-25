use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use globset::{GlobBuilder, GlobMatcher};

use crate::scan;

/// Canonical test files and directory roots, shared by collection and analysis.
pub struct TestPaths {
    paths: Vec<PathBuf>,
}

impl TestPaths {
    pub fn resolve(root: &Path, selections: &[String]) -> Result<Self, String> {
        let mut paths = Vec::new();
        for selection in selections {
            if selection.is_empty() {
                return Err("test path must not be empty".into());
            }
            let path = root.join(selection);
            if !path.try_exists().map_err(|error| error.to_string())? {
                expand(root, selection, &mut paths)?;
                continue;
            }
            if !path.is_file() && !path.is_dir() {
                return Err(format!("not a test file or directory: {selection:?}"));
            }
            paths.push(path.canonicalize().map_err(|error| error.to_string())?);
        }
        paths.sort();
        paths.dedup();
        Ok(Self { paths })
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.paths.iter().any(|test| path.starts_with(test))
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

fn expand(root: &Path, pattern: &str, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let matcher = glob_matcher(pattern)?;
    let before = paths.len();
    expand_directory(root, root, &matcher, paths)
        .map_err(|error| format!("cannot expand test glob {pattern:?}: {error}"))?;
    if paths.len() == before {
        return Err(format!(
            "test glob matched no files or directories: {pattern:?}"
        ));
    }
    Ok(())
}

fn glob_matcher(pattern: &str) -> Result<GlobMatcher, String> {
    validate_pattern(pattern)?;
    // Normalize a leading './' just as for explicit relative paths.
    Ok(
        GlobBuilder::new(pattern.strip_prefix("./").unwrap_or(pattern))
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map_err(|error| format!("invalid test glob {pattern:?}: {error}"))?
            .compile_matcher(),
    )
}

fn validate_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() || !pattern.contains(['*', '?', '[', '{']) {
        return Err(format!("test path does not exist: {pattern:?}"));
    }
    if Path::new(pattern).is_absolute()
        || Path::new(pattern)
            .components()
            .any(|part| part == Component::ParentDir)
    {
        return Err(format!(
            "test glob must be relative to root without '..': {pattern:?}"
        ));
    }
    Ok(())
}

fn expand_directory(
    root: &Path,
    directory: &Path,
    matcher: &GlobMatcher,
    paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for directory_entry in fs::read_dir(directory)? {
        let path = directory_entry?.path();
        let is_directory = path.is_dir();
        if is_directory && (path.is_symlink() || scan::excludes_directory(&path)) {
            continue;
        }
        if matcher.is_match(path.strip_prefix(root).map_err(io::Error::other)?)
            && (is_directory || path.is_file())
        {
            paths.push(path.canonicalize()?);
            // A selected directory already includes its entire subtree.
        } else if is_directory {
            expand_directory(root, &path, matcher, paths)?;
        } else {
            // Nonmatching files do not affect test selection.
        }
    }
    Ok(())
}
