use crate::collect::ReferenceMode;
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Source code root folder.
    #[arg(short, long)]
    pub root: PathBuf,

    /// Production package to analyze in root folder.
    #[arg(short, long)]
    pub source: PathBuf,

    /// Directory containing tests in root folder.
    #[arg(short, long)]
    pub tests: PathBuf,

    /// Verbose level.
    #[arg(short, long)]
    pub verbose: bool,

    /// Print usage graph edges instead of direct usage statuses.
    #[arg(long)]
    pub graph: bool,
}

impl Cli {
    pub fn reference_mode(&self) -> ReferenceMode<'_> {
        if self.graph {
            ReferenceMode::Graph
        } else {
            ReferenceMode::Report {
                source: &self.source,
                tests: &self.tests,
            }
        }
    }

    pub const fn verbosity(&self) -> Verbosity {
        if self.verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Quiet
        }
    }

    pub fn resolve_paths(mut self) -> Result<Self, String> {
        self.root = resolve_directory(&self.root)?;
        self.source = resolve_directory(&self.root.join(&self.source))?;
        self.tests = resolve_directory(&self.root.join(&self.tests))?;

        if self.source.starts_with(&self.tests) {
            return Err("production source must not be inside the test directory".into());
        }

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

#[derive(Clone, Copy)]
pub enum Verbosity {
    Quiet,
    Verbose,
}

impl Verbosity {
    pub const fn is_verbose(self) -> bool {
        matches!(self, Self::Verbose)
    }
}
