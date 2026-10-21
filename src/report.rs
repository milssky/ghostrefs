use crate::{
    analysis::{Report, SymbolUsage, UsageStatus},
    graph::Graph,
};

pub fn print_graph(graph: &Graph<'_>) {
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
}

pub fn print_report(report: &Report<'_>, graph: &Graph<'_>, verbose: bool) {
    if verbose {
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
        .filter(|usage| usage.status == UsageStatus::TestOnly)
    {
        print_usage(usage);
        if verbose {
            print_details(usage, report, graph);
        }
    }

    let findings = report
        .usages
        .iter()
        .filter(|usage| usage.status == UsageStatus::TestOnly)
        .count();
    println!(
        "TestOnly candidates: {findings} ({} production symbols analyzed)",
        report.usages.len()
    );
}

fn print_usage(usage: &SymbolUsage<'_>) {
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
}

fn print_details(usage: &SymbolUsage<'_>, report: &Report<'_>, graph: &Graph<'_>) {
    let symbol = usage.symbol;
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
