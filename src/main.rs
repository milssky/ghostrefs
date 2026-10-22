#![expect(clippy::print_stderr, reason = "CLI progress is written to stderr")]

//! Find production code kept alive only by tests.

use std::{error::Error as StdError, result::Result as StdResult};

type Error = Box<dyn StdError>;
type Result<T, E = Error> = StdResult<T, E>;

use clap::Parser as _;

mod analysis;
mod cli;
mod collect;
mod config;
mod graph;
mod language;
mod lsp;
mod models;
mod report;
mod scan;

fn main() -> Result<()> {
    let cli = cli::Cli::parse().resolve_paths()?;
    let config = config::Config::load(&cli.root)?;
    let files = collect_workspace(&cli, &config)?;
    let graph = graph::build(&files);
    if cli.graph {
        report::print_graph(&graph);
        return Ok(());
    }

    let report = analysis::analyze(&graph, &cli.source, &cli.tests, &config)?;
    report::print_report(&report, &graph, cli.verbosity());

    Ok(())
}

fn collect_workspace(cli: &cli::Cli, config: &config::Config) -> Result<Vec<models::FileAnalysis>> {
    let paths = scan::scan_workspace(&cli.root, &cli.source, &cli.tests)?;

    if cli.verbose {
        eprintln!("Collecting LSP symbols from {} files", paths.len());
    }

    collect::run(cli, &paths, &config.lsp_settings, cli.reference_mode())
}
