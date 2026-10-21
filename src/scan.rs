use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::language::Language;

/// Include workspace consumers and any source/test directories outside it.
pub fn scan_workspace(
    root: &Path,
    source: &Path,
    tests: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
pub fn scan_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(path)? {
        let path = entry?.path();

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
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("ghostrefs-scan-{}-{unique}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            path
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn package_copies_do_not_become_workspace_consumers() {
        let workspace = Workspace::new();
        let source = workspace.write("app/services.py", "def helper(): pass\n");
        let tests = workspace.write(
            "tests/test_services.py",
            "from app.services import helper\n",
        );
        let consumer = workspace.write("scripts/consumer.py", "from app.services import helper\n");
        workspace.write(
            "target/package/old-release/tests/test_services.py",
            "from app.services import helper\n",
        );
        workspace.write(
            "target/package/old-release/app/services.py",
            "def helper(): pass\n",
        );

        let files = scan_workspace(
            &workspace.0,
            &workspace.0.join("app"),
            &workspace.0.join("tests"),
        )
        .unwrap();
        let mut expected = vec![source, tests, consumer];
        expected.sort();
        assert_eq!(files, expected);
    }
}
