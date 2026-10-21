use std::{collections::VecDeque, path::Path};

use crate::{
    config::IgnoreRules,
    graph::{Graph, Node, NodeId},
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
    let mut outgoing = vec![Vec::new(); graph.nodes.len()];
    let mut consumed = vec![false; graph.nodes.len()];
    for edge in &graph.edges {
        outgoing[edge.from.0].push(edge.to);
        if edge.from != edge.to {
            consumed[edge.to.0] = true;
        }
    }

    let mut production_roots = Vec::new();
    let mut test_roots = Vec::new();
    let mut external_roots = Vec::new();
    let mut unknown_roots = Vec::new();
    let mut entries = Vec::new();
    let mut explicit_roots = vec![false; graph.nodes.len()];

    for entry in configured_entries {
        if !is_production(&entry.path, source, tests) {
            return Err(format!(
                "production entrypoint must belong to source outside tests: {}::{}",
                entry.path.display(),
                entry.qualname
            ));
        }
        if ignore.matches(&entry.path) {
            return Err(format!(
                "production entrypoint is ignored: {}::{}",
                entry.path.display(),
                entry.qualname
            ));
        }
        let id = graph.resolve_entry(entry)?;
        explicit_roots[id.0] = true;
        production_roots.push(id);
    }

    for (index, node) in graph.nodes.iter().enumerate() {
        let id = NodeId(index);
        let ignored = match node {
            Node::Module(path) | Node::Symbol { path, .. } => ignore.matches(path),
            Node::Unresolved(location) => location
                .uri
                .to_file_path()
                .is_ok_and(|path| ignore.matches(&path)),
        };
        if ignored {
            unknown_roots.push(id);
            continue;
        }
        match node {
            Node::Module(path) if path.starts_with(tests) => test_roots.push(id),
            Node::Module(path) if is_production(path, source, tests) => production_roots.push(id),
            Node::Symbol { path, symbol } => {
                if symbol.references.is_none() {
                    unknown_roots.push(id);
                }
                if path.starts_with(tests) {
                    test_roots.push(id);
                } else if !path.starts_with(source) {
                    external_roots.push(id);
                } else if is_production(path, source, tests)
                    && !symbol.qualname.contains('.')
                    && symbol.references.is_some()
                    && (!consumed[index] || symbol.name == "main")
                {
                    entries.push(EntryPoint {
                        path: path.to_path_buf(),
                        qualname: symbol.qualname.clone(),
                    });
                }
            }
            Node::Unresolved(location) => {
                match location.uri.to_file_path() {
                    Ok(path) if path.starts_with(tests) => test_roots.push(id),
                    // The owner is missing even though the file is in source.
                    Ok(path) if is_production(&path, source, tests) => unknown_roots.push(id),
                    _ => external_roots.push(id),
                }
            }
            Node::Module(_) => external_roots.push(id),
        }
    }

    for entry in &entries {
        production_roots.push(graph.resolve_entry(entry)?);
    }
    production_roots.sort_by_key(|id| id.0);
    production_roots.dedup();
    unknown_roots.sort_by_key(|id| id.0);
    unknown_roots.dedup();
    let production = Reachability::walk(&outgoing, &production_roots);
    let test_reach = Reachability::walk(&outgoing, &test_roots);
    let external = Reachability::walk(&outgoing, &external_roots);
    let unknown = Reachability::walk(&outgoing, &unknown_roots);

    let usages = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let Node::Symbol { path, symbol } = node else {
                return None;
            };
            if !is_production(path, source, tests) || ignore.matches(path) {
                return None;
            }
            // Local variables and parameters remain in the graph but do not make
            // useful dead-code findings. Nested functions, classes and methods do.
            if symbol.qualname.contains('.') && !is_declaration(symbol.kind) {
                return None;
            }
            let status = if symbol.references.is_none() && !explicit_roots[index] {
                UsageStatus::Unknown
            } else if production.reached[index] {
                if !consumed[index]
                    && symbol.name != "main"
                    && !test_reach.reached[index]
                    && !explicit_roots[index]
                {
                    UsageStatus::Unreferenced
                } else {
                    UsageStatus::UsedInProduction
                }
            } else if external.reached[index] {
                UsageStatus::UsedOutsideScope
            } else if unknown.reached[index] {
                UsageStatus::Unknown
            } else if test_reach.reached[index] {
                UsageStatus::TestOnly
            } else if !consumed[index] {
                UsageStatus::Unreferenced
            } else {
                UsageStatus::Unreachable
            };
            Some(SymbolUsage {
                node: NodeId(index),
                path,
                symbol,
                status,
            })
        })
        .collect();

    Ok(Report {
        usages,
        production_roots,
        unknown_roots,
        tests: test_reach,
    })
}

fn is_production(path: &Path, source: &Path, tests: &Path) -> bool {
    path.starts_with(source) && !path.starts_with(tests)
}

fn is_declaration(kind: async_lsp::lsp_types::SymbolKind) -> bool {
    use async_lsp::lsp_types::SymbolKind;
    matches!(
        kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CLASS | SymbolKind::CONSTRUCTOR
    )
}

struct Reachability {
    reached: Vec<bool>,
    parents: Vec<Option<NodeId>>,
}

impl Reachability {
    fn walk(outgoing: &[Vec<NodeId>], roots: &[NodeId]) -> Self {
        let mut result = Self {
            reached: vec![false; outgoing.len()],
            parents: vec![None; outgoing.len()],
        };
        let mut pending = VecDeque::new();
        for &root in roots {
            if !result.reached[root.0] {
                result.reached[root.0] = true;
                pending.push_back(root);
            }
        }
        while let Some(from) = pending.pop_front() {
            for &to in &outgoing[from.0] {
                if !result.reached[to.0] {
                    result.reached[to.0] = true;
                    result.parents[to.0] = Some(from);
                    pending.push_back(to);
                }
            }
        }
        result
    }

    fn chain(&self, target: NodeId) -> Vec<NodeId> {
        if !self.reached[target.0] {
            return Vec::new();
        }
        let mut chain = vec![target];
        let mut current = target;
        while let Some(parent) = self.parents[current.0] {
            chain.push(parent);
            current = parent;
        }
        chain.reverse();
        chain
    }
}
