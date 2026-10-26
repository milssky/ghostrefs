#![expect(
    clippy::print_stdout,
    reason = "This module renders CLI reports to stdout"
)]
#![expect(
    clippy::use_debug,
    reason = "Verbose reports expose LSP diagnostic details"
)]

use crate::{
    analysis::{Report, SymbolUsage, UsageStatus},
    cli::Verbosity,
    graph::{EdgeOrigin, Graph},
};

pub fn print_graph(graph: &Graph<'_>) {
    for edge in &graph.edges {
        let (Some(from), Some(target)) =
            (graph.nodes.get(edge.from.0), graph.nodes.get(edge.target.0))
        else {
            continue;
        };
        print!("{from} -> {target}");
        print_edge_origin(edge.origin);
    }
}

fn print_edge_origin(origin: EdgeOrigin<'_>) {
    match origin {
        EdgeOrigin::Reference(location) => println!(
            " [reference {}:{}]",
            location.range.start.line.saturating_add(1),
            location.range.start.character.saturating_add(1)
        ),
        EdgeOrigin::Implicit(reason) => println!(" [{reason}]"),
    }
}

pub fn print_report(report: &Report<'_>, graph: &Graph<'_>, verbosity: Verbosity) {
    if verbosity.is_verbose() {
        print_roots(report, graph);
    }
    let candidates: Vec<_> = report
        .usages
        .iter()
        .filter(|usage| usage.status == UsageStatus::TestOnly)
        .collect();
    for usage in &candidates {
        print_usage(usage);
        if verbosity.is_verbose() {
            print_details(usage, report, graph);
        }
    }
    let findings = candidates.len();
    println!(
        "TestOnly candidates: {findings} ({} production symbols analyzed)",
        report.usages.len()
    );
}

fn print_usage(usage: &SymbolUsage<'_>) {
    let symbol = usage.symbol;
    println!(
        "{}:{}:{} {} ({:?})",
        usage.path.display(),
        symbol.selection_range.start.line.saturating_add(1),
        symbol.selection_range.start.character.saturating_add(1),
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
            .filter_map(|node_id| graph.nodes.get(node_id.0).map(ToString::to_string))
            .collect::<Vec<_>>()
            .join(" -> ")
    );
}

fn print_roots(report: &Report<'_>, graph: &Graph<'_>) {
    for &root in &report.production_roots {
        if let Some(node) = graph.nodes.get(root.0) {
            println!("Production root: {node}");
        }
    }
    for &root in &report.unknown_roots {
        if let Some(node) = graph.nodes.get(root.0) {
            println!("Unknown root: {node}");
        }
    }
}
