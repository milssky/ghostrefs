use crate::{collect::ReferenceMode, test_paths::TestPaths};
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

    /// Test files, directories, or quoted glob patterns relative to root.
    #[arg(short, long, required = true, num_args = 1..)]
    pub tests: Vec<String>,

    /// Include analysis roots, references, and test chains in the report.
    #[arg(short, long)]
    pub verbose: bool,

    /// Print usage graph edges instead of direct usage statuses.
    #[arg(long)]
    pub graph: bool,
}

pub struct ResolvedCli {
    pub root: PathBuf,
    pub source: PathBuf,
    pub tests: TestPaths,
    pub verbose: bool,
    pub graph: bool,
}

impl ResolvedCli {
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
}

impl Cli {
    pub fn resolve_paths(self) -> Result<ResolvedCli, String> {
        let root = resolve_directory(&self.root)?;
        let source = resolve_directory(&root.join(&self.source))?;
        let tests = TestPaths::resolve(&root, &self.tests)?;
        if tests.contains(&source) {
            return Err("production source must not be inside a test directory".into());
        }
        Ok(ResolvedCli {
            root,
            source,
            tests,
            verbose: self.verbose,
            graph: self.graph,
        })
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
