use clap::Parser;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse().resolve_paths()?;
    let config = config::Config::load(&cli.root)?;
    let files = scan::scan_workspace(&cli.root, &cli.source, &cli.tests)?;

    if cli.verbose {
        eprintln!("Collecting LSP symbols from {} files", files.len());
    }

    let files = collect::run(
        &cli.root,
        &files,
        &config.lsp_settings,
        if cli.graph {
            collect::ReferenceMode::Graph
        } else {
            collect::ReferenceMode::Report {
                source: &cli.source,
                tests: &cli.tests,
            }
        },
        cli.verbose,
    )?;
    let graph = graph::build(&files);
    if cli.graph {
        report::print_graph(&graph);
        return Ok(());
    }

    let report = analysis::analyze(
        &graph,
        &cli.source,
        &cli.tests,
        &config.ignore,
        &config.production_entrypoints,
    )?;
    report::print_report(&report, &graph, cli.verbose);

    Ok(())
}
