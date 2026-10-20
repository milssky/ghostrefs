use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Source code root folder
    #[arg(short, long)]
    pub root: PathBuf,

    /// Production package to analyze in root folder
    #[arg(short, long)]
    pub source: PathBuf,

    /// Directory containing tests in root folder
    #[arg(short, long)]
    pub tests: PathBuf,

    /// Verbose level
    #[arg(short, long)]
    pub verbose: bool,
}

impl Cli {
    pub fn resolve_paths(mut self) -> Result<Self, String> {
        self.root = resolve_directory(&self.root)?;
        self.source = resolve_directory(&self.root.join(&self.source))?;
        self.tests = resolve_directory(&self.root.join(&self.tests))?;

        Ok(self)
    }
}

fn resolve_directory(path: &Path) -> Result<PathBuf, String> {
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("Invalid directory {}: {error}", path.display()))?;

    if !resolved.is_dir() {
        return Err(format!("Not a directory: {}", resolved.display()));
    }

    Ok(resolved)
}
