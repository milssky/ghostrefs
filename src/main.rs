use clap::Parser;

mod analysis;
mod cli;
mod config;
mod graph;
mod language;
mod lsp;
mod models;
mod scan;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse().resolve_paths()?;
    let config = config::Config::load(&cli.root)?;
    // Open the workspace, including consumers outside source/tests. Otherwise
    // Pyright may omit references in documents it has never analyzed.
    let mut files = scan::scan_dir(&cli.root)?;
    if !cli.source.starts_with(&cli.root) {
        files.extend(scan::scan_dir(&cli.source)?);
    }
    if !cli.tests.starts_with(&cli.root) {
        files.extend(scan::scan_dir(&cli.tests)?);
    }
    if !files
        .iter()
        .any(|path| path.starts_with(&cli.source) && !path.starts_with(&cli.tests))
    {
        return Err("no supported production source files found".into());
    }
    files.sort();
    files.dedup();

    if cli.verbose {
        eprintln!("Collecting LSP symbols from {} files", files.len());
    }

    let files = lsp::run(&cli.root, &files, &config.lsp_settings)?;
    let graph = graph::build(&files);
    if cli.graph {
        for edge in &graph.edges {
            print!("{} -> {}", graph.nodes[edge.from.0], graph.nodes[edge.to.0]);
            if let Some(location) = edge.location {
                println!(
                    " [reference {}:{}]",
                    location.range.start.line + 1,
                    location.range.start.character + 1
                );
            } else {
                println!(" [class member]");
            }
        }
        return Ok(());
    }

    let report = analysis::analyze(
        &graph,
        &cli.source,
        &cli.tests,
        &config.ignore,
        &config.production_entrypoints,
    )?;
    if cli.verbose {
        for &root in &report.production_roots {
            println!("Production root: {}", graph.nodes[root.0]);
        }
        for &root in &report.unknown_roots {
            println!("Unknown root: {}", graph.nodes[root.0]);
        }
    }
    for usage in report
        .usages
        .iter()
        .filter(|usage| usage.status == analysis::UsageStatus::TestOnly)
    {
        let symbol = usage.symbol;
        println!(
            "{:?} {}:{}:{} {} ({:?})",
            usage.status,
            usage.path.display(),
            symbol.selection_range.start.line + 1,
            symbol.selection_range.start.character + 1,
            symbol.qualname,
            symbol.kind,
        );
        if cli.verbose {
            println!(
                "  range: {:?}, references: {:#?}",
                symbol.range, symbol.references
            );
            let chain = report.test_chain(usage.node);
            println!(
                "  test chain: {}",
                chain
                    .iter()
                    .map(|id| graph.nodes[id.0].to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }
    }

    let findings = report
        .usages
        .iter()
        .filter(|usage| usage.status == analysis::UsageStatus::TestOnly)
        .count();
    println!(
        "TestOnly candidates: {findings} ({} production symbols analyzed)",
        report.usages.len()
    );

    Ok(())
}
