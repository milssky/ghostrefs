use std::path::Path;

use crate::{
    config::IgnoreRules,
    graph::{Connections, Graph, Node, NodeId, Reachability},
    models::{EntryPoint, SymbolAnalysis},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageStatus {
    TestOnly,
    UsedInProduction,
    UsedOutsideScope,
    Unreferenced,
    Unreachable,
    Unknown,
}

pub struct SymbolUsage<'a> {
    pub node: NodeId,
    pub path: &'a Path,
    pub symbol: &'a SymbolAnalysis,
    pub status: UsageStatus,
}

pub struct Report<'a> {
    pub usages: Vec<SymbolUsage<'a>>,
    pub production_roots: Vec<NodeId>,
    pub unknown_roots: Vec<NodeId>,
    tests: Reachability,
}

impl Report<'_> {
    pub fn test_chain(&self, target: NodeId) -> Vec<NodeId> {
        self.tests.chain(target)
    }
}

/// Conservative roots are production modules, main functions, and unconsumed
/// top-level declarations. Configured entry points add explicitly known roots.
pub fn analyze<'a>(
    graph: &Graph<'a>,
    source: &Path,
    tests: &Path,
    ignore: &IgnoreRules,
    configured_entries: &[EntryPoint],
) -> Result<Report<'a>, String> {
    let scope = AnalysisScope {
        source,
        tests,
        ignore,
    };
    let connections = Connections::new(graph);
    let roots = Roots::collect(graph, &scope, &connections.consumed, configured_entries)?;
    let reachability = UsageReachability::walk(&connections.outgoing, &roots);

    let usages = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let Node::Symbol { path, symbol } = node else {
                return None;
            };
            if !scope.includes_in_report(path, symbol) {
                return None;
            }
            Some(SymbolUsage {
                node: NodeId(index),
                path,
                symbol,
                status: reachability.status(
                    index,
                    symbol,
                    connections.consumed[index],
                    roots.explicit[index],
                ),
            })
        })
        .collect();

    Ok(Report {
        usages,
        production_roots: roots.production,
        unknown_roots: roots.unknown,
        tests: reachability.tests,
    })
}

struct AnalysisScope<'a> {
    source: &'a Path,
    tests: &'a Path,
    ignore: &'a IgnoreRules,
}

impl AnalysisScope<'_> {
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

    fn validate_entry(&self, entry: &EntryPoint) -> Result<(), String> {
        if !self.is_production(&entry.path) {
            return Err(format!(
                "production entrypoint must belong to source outside tests: {}::{}",
                entry.path.display(),
                entry.qualname
            ));
        }
        if self.ignore.matches(&entry.path) {
            return Err(format!(
                "production entrypoint is ignored: {}::{}",
                entry.path.display(),
                entry.qualname
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
        for entry in configured_entries {
            scope.validate_entry(entry)?;
            let id = graph.resolve_entry(entry)?;
            roots.explicit[id.0] = true;
            roots.production.push(id);
        }

        let mut inferred_entries = Vec::new();
        for (index, node) in graph.nodes.iter().enumerate() {
            let id = NodeId(index);
            if scope.is_ignored(node) {
                roots.unknown.push(id);
                continue;
            }
            if let Some(entry) = roots.classify_node(id, node, scope, consumed[index]) {
                inferred_entries.push(entry);
            }
        }
        // Resolve inferred entries by name as well, preserving ambiguity errors.
        for entry in &inferred_entries {
            roots.production.push(graph.resolve_entry(entry)?);
        }
        roots.production.sort_by_key(|id| id.0);
        roots.production.dedup();
        roots.unknown.sort_by_key(|id| id.0);
        roots.unknown.dedup();
        Ok(roots)
    }

    fn classify_node(
        &mut self,
        id: NodeId,
        node: &Node<'_>,
        scope: &AnalysisScope<'_>,
        consumed: bool,
    ) -> Option<EntryPoint> {
        match node {
            Node::Module(path) if path.starts_with(scope.tests) => self.tests.push(id),
            Node::Module(path) if scope.is_production(path) => self.production.push(id),
            Node::Module(_) => self.external.push(id),
            Node::Symbol { path, symbol } => {
                if symbol.references.is_none() {
                    self.unknown.push(id);
                }
                if path.starts_with(scope.tests) {
                    self.tests.push(id);
                } else if !path.starts_with(scope.source) {
                    self.external.push(id);
                } else if is_inferred_entry(symbol, consumed) {
                    return Some(EntryPoint {
                        path: path.to_path_buf(),
                        qualname: symbol.qualname.clone(),
                    });
                }
            }
            Node::Unresolved(location) => match location.uri.to_file_path() {
                Ok(path) if path.starts_with(scope.tests) => self.tests.push(id),
                // The owner is missing even though the file is in source.
                Ok(path) if scope.is_production(&path) => self.unknown.push(id),
                _ => self.external.push(id),
            },
        }
        None
    }
}

fn is_inferred_entry(symbol: &SymbolAnalysis, consumed: bool) -> bool {
    !symbol.qualname.contains('.')
        && symbol.references.is_some()
        && (!consumed || symbol.name == "main")
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

    fn status(
        &self,
        index: usize,
        symbol: &SymbolAnalysis,
        consumed: bool,
        explicit: bool,
    ) -> UsageStatus {
        if symbol.references.is_none() && !explicit {
            return UsageStatus::Unknown;
        }
        if self.production.reached[index] {
            let unreferenced =
                !consumed && symbol.name != "main" && !self.tests.reached[index] && !explicit;
            return if unreferenced {
                UsageStatus::Unreferenced
            } else {
                UsageStatus::UsedInProduction
            };
        }
        if self.external.reached[index] {
            return UsageStatus::UsedOutsideScope;
        }
        if self.unknown.reached[index] {
            return UsageStatus::Unknown;
        }
        if self.tests.reached[index] {
            return UsageStatus::TestOnly;
        }
        if !consumed {
            UsageStatus::Unreferenced
        } else {
            UsageStatus::Unreachable
        }
    }
}
