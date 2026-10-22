use std::path::Path;

use crate::{
    config::{Config, IgnoreRules},
    graph::{Connections, Graph, Node, NodeId, Reachability},
    models::{EntryPoint, SymbolAnalysis},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageStatus {
    TestOnly,
    UsedInProduction,
    UsedOutsideScope,
    Unreferenced,
    Unreachable,
    Unknown,
}

pub(crate) struct SymbolUsage<'source> {
    pub node: NodeId,
    pub path: &'source Path,
    pub symbol: &'source SymbolAnalysis,
    pub status: UsageStatus,
}

pub(crate) struct Report<'source> {
    pub usages: Vec<SymbolUsage<'source>>,
    pub production_roots: Vec<NodeId>,
    pub unknown_roots: Vec<NodeId>,
    pub tests: Reachability,
}

impl Report<'_> {
    pub(crate) fn test_chain(&self, target: NodeId) -> Vec<NodeId> {
        self.tests.chain(target)
    }
}

/// Conservative roots are production modules, main functions, and unconsumed
/// top-level declarations. Configured entry points add explicitly known roots.
pub(crate) fn analyze<'source>(
    graph: &Graph<'source>,
    source: &Path,
    tests: &Path,
    config: &Config,
) -> Result<Report<'source>, String> {
    let scope = AnalysisScope {
        source,
        tests,
        ignore: &config.ignore,
    };
    let connections = Connections::new(graph);
    let roots = Roots::collect(
        graph,
        &scope,
        &connections.consumed,
        &config.production_entrypoints,
    )?;
    let reachability = UsageReachability::walk(&connections.outgoing, &roots);

    let usages = collect_usages(graph, &scope, (&connections, &roots), &reachability);

    Ok(Report {
        usages,
        production_roots: roots.production,
        unknown_roots: roots.unknown,
        tests: reachability.tests,
    })
}

fn collect_usages<'source>(
    graph: &Graph<'source>,
    scope: &AnalysisScope<'_>,
    metadata: (&Connections, &Roots),
    reachability: &UsageReachability,
) -> Vec<SymbolUsage<'source>> {
    let (connections, roots) = metadata;
    graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let (path, symbol) = scope.report_symbol(node)?;
            Some(SymbolUsage {
                node: NodeId(index),
                path,
                symbol,
                status: reachability.status(
                    NodeId(index),
                    symbol,
                    SymbolFlags::new(index, connections, roots),
                ),
            })
        })
        .collect()
}

type ReportSymbol<'source> = (&'source Path, &'source SymbolAnalysis);

struct AnalysisScope<'source> {
    source: &'source Path,
    tests: &'source Path,
    ignore: &'source IgnoreRules,
}

impl AnalysisScope<'_> {
    fn report_symbol<'source>(&self, node: &Node<'source>) -> Option<ReportSymbol<'source>> {
        match node {
            Node::Symbol { path, symbol } if self.includes_in_report(path, symbol) => {
                Some((path, symbol))
            }
            Node::Module(_) | Node::Symbol { .. } | Node::Unresolved(_) => None,
        }
    }

    fn is_production(&self, path: &Path) -> bool {
        path.starts_with(self.source) && !path.starts_with(self.tests)
    }

    fn is_ignored(&self, node: &Node<'_>) -> bool {
        match node {
            Node::Module(path) | Node::Symbol { path, .. } => self.ignore.matches(path),
            Node::Unresolved(location) => location
                .uri
                .to_file_path()
                .is_ok_and(|path| self.ignore.matches(&path)),
        }
    }

    fn includes_in_report(&self, path: &Path, symbol: &SymbolAnalysis) -> bool {
        // Local variables and parameters remain in the graph. Nested declarations
        // and top-level variables can still be useful dead-code findings.
        self.is_production(path) && !self.ignore.matches(path) && symbol.is_reportable()
    }

    fn validate_entry(&self, entrypoint: &EntryPoint) -> Result<(), String> {
        if !self.is_production(&entrypoint.path) {
            return Err(format!(
                "production entrypoint must belong to source outside tests: {}::{}",
                entrypoint.path.display(),
                entrypoint.qualname
            ));
        }
        if self.ignore.matches(&entrypoint.path) {
            return Err(format!(
                "production entrypoint is ignored: {}::{}",
                entrypoint.path.display(),
                entrypoint.qualname
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Roots {
    production: Vec<NodeId>,
    tests: Vec<NodeId>,
    external: Vec<NodeId>,
    unknown: Vec<NodeId>,
    explicit: Vec<bool>,
}

impl Roots {
    fn collect(
        graph: &Graph<'_>,
        scope: &AnalysisScope<'_>,
        consumed: &[bool],
        configured_entries: &[EntryPoint],
    ) -> Result<Self, String> {
        let mut roots = Self {
            explicit: vec![false; graph.nodes.len()],
            ..Self::default()
        };
        roots.add_configured(graph, scope, configured_entries)?;
        roots.add_inferred(graph, scope, consumed)?;
        roots.production.sort_by_key(|node_id| node_id.0);
        roots.production.dedup();
        roots.unknown.sort_by_key(|node_id| node_id.0);
        roots.unknown.dedup();
        Ok(roots)
    }

    fn add_configured(
        &mut self,
        graph: &Graph<'_>,
        scope: &AnalysisScope<'_>,
        configured_entries: &[EntryPoint],
    ) -> Result<(), String> {
        for entrypoint in configured_entries {
            scope.validate_entry(entrypoint)?;
            let node_id = graph.resolve_entry(entrypoint)?;
            if let Some(explicit) = self.explicit.get_mut(node_id.0) {
                *explicit = true;
            }
            self.production.push(node_id);
        }

        Ok(())
    }

    fn add_inferred(
        &mut self,
        graph: &Graph<'_>,
        scope: &AnalysisScope<'_>,
        consumed: &[bool],
    ) -> Result<(), String> {
        let mut inferred_entries = Vec::new();
        for (index, node) in graph.nodes.iter().enumerate() {
            let node_id = NodeId(index);
            if let Some(entrypoint) = self.classify_node(
                (node_id, node),
                scope,
                SymbolFlags {
                    consumed: consumed.get(index).copied().unwrap_or(false),
                    explicit: false,
                },
            ) {
                inferred_entries.push(entrypoint);
            }
        }
        // Resolve inferred entrypoints by name as well, preserving ambiguity errors.
        for entrypoint in &inferred_entries {
            self.production.push(graph.resolve_entry(entrypoint)?);
        }
        Ok(())
    }

    fn classify_node(
        &mut self,
        indexed_node: (NodeId, &Node<'_>),
        scope: &AnalysisScope<'_>,
        flags: SymbolFlags,
    ) -> Option<EntryPoint> {
        let (node_id, node) = indexed_node;
        if scope.is_ignored(node) {
            self.unknown.push(node_id);
            return None;
        }
        match node {
            Node::Module(path) if path.starts_with(scope.tests) => self.tests.push(node_id),
            Node::Module(path) if scope.is_production(path) => self.production.push(node_id),
            Node::Module(_) => self.external.push(node_id),
            Node::Symbol { path, symbol } => {
                return self.classify_symbol((node_id, symbol), path, (scope, flags));
            }
            Node::Unresolved(location) => match location.uri.to_file_path() {
                Ok(path) if path.starts_with(scope.tests) => self.tests.push(node_id),
                // The owner is missing even though the file is in source.
                Ok(path) if scope.is_production(&path) => self.unknown.push(node_id),
                _ => self.external.push(node_id),
            },
        }
        None
    }
    fn classify_symbol(
        &mut self,
        indexed_symbol: (NodeId, &SymbolAnalysis),
        path: &Path,
        context: (&AnalysisScope<'_>, SymbolFlags),
    ) -> Option<EntryPoint> {
        let (node_id, symbol) = indexed_symbol;
        let (scope, flags) = context;
        if symbol.references.is_none() {
            self.unknown.push(node_id);
        }
        if path.starts_with(scope.tests) {
            self.tests.push(node_id);
        } else if !path.starts_with(scope.source) {
            self.external.push(node_id);
        } else if is_inferred_entry(symbol, flags) {
            return Some(EntryPoint {
                path: path.to_path_buf(),
                qualname: symbol.qualname.clone(),
            });
        } else {
            // Other production declarations are reached through their consumers.
        }
        None
    }
}

fn is_inferred_entry(symbol: &SymbolAnalysis, flags: SymbolFlags) -> bool {
    !symbol.qualname.contains('.')
        && symbol.references.is_some()
        && (!flags.consumed || symbol.name == "main")
}

struct UsageReachability {
    production: Reachability,
    tests: Reachability,
    external: Reachability,
    unknown: Reachability,
}

impl UsageReachability {
    fn walk(outgoing: &[Vec<NodeId>], roots: &Roots) -> Self {
        Self {
            production: Reachability::walk(outgoing, &roots.production),
            tests: Reachability::walk(outgoing, &roots.tests),
            external: Reachability::walk(outgoing, &roots.external),
            unknown: Reachability::walk(outgoing, &roots.unknown),
        }
    }

    fn status(&self, node: NodeId, symbol: &SymbolAnalysis, flags: SymbolFlags) -> UsageStatus {
        if symbol.references.is_none() && !flags.explicit {
            return UsageStatus::Unknown;
        }
        if self.production.contains(node) {
            return self.production_status(node, symbol, flags);
        }
        if self.external.contains(node) {
            return UsageStatus::UsedOutsideScope;
        }
        if self.unknown.contains(node) {
            return UsageStatus::Unknown;
        }
        if self.tests.contains(node) {
            return UsageStatus::TestOnly;
        }
        if flags.consumed {
            UsageStatus::Unreachable
        } else {
            UsageStatus::Unreferenced
        }
    }

    fn production_status(
        &self,
        node: NodeId,
        symbol: &SymbolAnalysis,
        flags: SymbolFlags,
    ) -> UsageStatus {
        if !flags.consumed && symbol.name != "main" && !self.tests.contains(node) && !flags.explicit
        {
            UsageStatus::Unreferenced
        } else {
            UsageStatus::UsedInProduction
        }
    }
}

#[derive(Clone, Copy)]
struct SymbolFlags {
    consumed: bool,
    explicit: bool,
}

impl SymbolFlags {
    fn new(index: usize, connections: &Connections, roots: &Roots) -> Self {
        Self {
            consumed: connections.consumed.get(index).copied().unwrap_or(false),
            explicit: roots.explicit.get(index).copied().unwrap_or(false),
        }
    }
}
