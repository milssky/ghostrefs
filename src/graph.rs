use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    fmt as formatting,
    path::Path,
};

use async_lsp::lsp_types::{Location, Position};

use crate::{
    models::{EntryPoint, FileAnalysis, SymbolAnalysis},
    test_paths::TestPaths,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug)]
pub enum Node<'source> {
    Module(&'source Path),
    Symbol {
        path: &'source Path,
        symbol: &'source SymbolAnalysis,
    },
    /// The referencing document was not collected, so its owner is unknown.
    Unresolved(&'source Location),
}

impl formatting::Display for Node<'_> {
    #[expect(
        clippy::renamed_function_params,
        reason = "Use a descriptive formatter parameter name"
    )]
    fn fmt(&self, formatter: &mut formatting::Formatter<'_>) -> formatting::Result {
        match self {
            Self::Module(path) => write!(formatter, "{}::<module>", path.display()),
            Self::Symbol { path, symbol } => write!(
                formatter,
                "{}:{}:{}::{}",
                path.display(),
                symbol.selection_range.start.line.saturating_add(1),
                symbol.selection_range.start.character.saturating_add(1),
                symbol.qualname,
            ),
            Self::Unresolved(location) => write!(
                formatter,
                "{}:{}:{}::<unresolved>",
                location.uri,
                location.range.start.line.saturating_add(1),
                location.range.start.character.saturating_add(1),
            ),
        }
    }
}

#[derive(Debug)]
pub struct Edge<'source> {
    pub from: NodeId,
    pub target: NodeId,
    pub origin: EdgeOrigin<'source>,
}

#[derive(Debug, Clone, Copy)]
pub enum EdgeOrigin<'source> {
    Reference(&'source Location),
    Implicit(&'static str),
}

#[derive(Debug)]
pub struct Graph<'source> {
    pub nodes: Vec<Node<'source>>,
    /// Consumer-to-symbol edges from LSP references or language-inferred activation.
    pub edges: Vec<Edge<'source>>,
}

impl Graph<'_> {
    /// Resolve an exact qualified name within a collected file.
    pub fn resolve_entry(&self, entrypoint: &EntryPoint) -> Result<NodeId, String> {
        let matches = self.entry_candidates(entrypoint);

        match matches.as_slice() {
            [node_id] => Ok(*node_id),
            [] => Err(format!(
                "entry point not found: {}::{}",
                entrypoint.path.display(),
                entrypoint.qualname,
            )),
            _ => Err(format!(
                "ambiguous entry point {}::{}; candidates: {}",
                entrypoint.path.display(),
                entrypoint.qualname,
                matches
                    .iter()
                    .filter_map(|node_id| self.nodes.get(node_id.0).map(ToString::to_string))
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
        }
    }
    fn entry_candidates(&self, entrypoint: &EntryPoint) -> Vec<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| match node {
                Node::Symbol { path, symbol }
                    if *path == entrypoint.path && symbol.qualname == entrypoint.qualname =>
                {
                    Some(NodeId(index))
                }
                Node::Module(_) | Node::Symbol { .. } | Node::Unresolved(_) => None,
            })
            .collect()
    }
}

pub struct Connections {
    pub outgoing: Vec<Vec<NodeId>>,
    pub consumed: Vec<bool>,
}

impl Connections {
    pub fn new(graph: &Graph<'_>) -> Self {
        let mut connections = Self {
            outgoing: vec![Vec::new(); graph.nodes.len()],
            consumed: vec![false; graph.nodes.len()],
        };
        for edge in &graph.edges {
            if let Some(targets) = connections.outgoing.get_mut(edge.from.0) {
                targets.push(edge.target);
            }
            if edge.from != edge.target
                && let Some(consumed) = connections.consumed.get_mut(edge.target.0)
            {
                *consumed = true;
            }
        }
        connections
    }
}

pub struct Reachability {
    reached: Vec<bool>,
    parents: Vec<Option<NodeId>>,
}

impl Reachability {
    pub fn walk(outgoing: &[Vec<NodeId>], roots: &[NodeId]) -> Self {
        let mut reachability = Self {
            reached: vec![false; outgoing.len()],
            parents: vec![None; outgoing.len()],
        };
        let mut pending = VecDeque::new();
        for &root in roots {
            reachability.visit(root, None, &mut pending);
        }
        while let Some(from) = pending.pop_front() {
            for &target in outgoing.get(from.0).into_iter().flatten() {
                reachability.visit(target, Some(from), &mut pending);
            }
        }
        reachability
    }

    fn visit(&mut self, node: NodeId, parent: Option<NodeId>, pending: &mut VecDeque<NodeId>) {
        let Some(reached) = self.reached.get_mut(node.0) else {
            return;
        };
        if !*reached {
            *reached = true;
            if let Some(slot) = self.parents.get_mut(node.0) {
                *slot = parent;
            }
            pending.push_back(node);
        }
    }

    pub fn contains(&self, node: NodeId) -> bool {
        self.reached.get(node.0).copied().unwrap_or(false)
    }

    pub fn chain(&self, target: NodeId) -> Vec<NodeId> {
        if !self.contains(target) {
            return Vec::new();
        }
        let mut chain = vec![target];
        let mut current = target;
        while let Some(&Some(parent)) = self.parents.get(current.0) {
            chain.push(parent);
            current = parent;
        }
        chain.reverse();
        chain
    }
}

struct FileNodes {
    module: NodeId,
    symbols: Vec<NodeId>,
}

/// Build a graph over the collected files, retaining symbols with no references.
/// File paths must use the same absolute paths as reference URIs.
pub fn build(files: &[FileAnalysis]) -> Graph<'_> {
    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let index = index_files(&mut graph, files);
    add_reference_edges(&mut graph, files, &index);
    add_implicit_edges(&mut graph, files, &index);
    graph
}

fn index_files<'source>(
    graph: &mut Graph<'source>,
    files: &'source [FileAnalysis],
) -> HashMap<&'source Path, FileNodes> {
    let mut index = HashMap::new();
    for file in files {
        let module = NodeId(graph.nodes.len());
        graph.nodes.push(Node::Module(file.path()));

        let symbols = file
            .symbols()
            .iter()
            .map(|symbol| {
                let node_id = NodeId(graph.nodes.len());
                graph.nodes.push(Node::Symbol {
                    path: file.path(),
                    symbol,
                });
                node_id
            })
            .collect();

        let _ = index.insert(file.path(), FileNodes { module, symbols });
    }
    index
}

fn add_implicit_edges(
    graph: &mut Graph<'_>,
    files: &[FileAnalysis],
    index: &HashMap<&Path, FileNodes>,
) {
    for file in files {
        let Some(nodes) = index.get(file.path()) else {
            continue;
        };
        graph
            .edges
            .extend(file.implicit_edges().iter().filter_map(|edge| {
                Some(Edge {
                    from: *nodes.symbols.get(edge.from.0)?,
                    target: *nodes.symbols.get(edge.target.0)?,
                    origin: EdgeOrigin::Implicit(edge.reason),
                })
            }));
    }
}

fn add_reference_edges<'source>(
    graph: &mut Graph<'source>,
    files: &'source [FileAnalysis],
    index: &HashMap<&Path, FileNodes>,
) {
    for file in files {
        let Some(targets) = index.get(file.path()) else {
            continue;
        };
        for (symbol, &target) in file.symbols().iter().zip(&targets.symbols) {
            for location in symbol.references.iter().flatten() {
                let from = reference_owner(graph, index, location);

                graph.edges.push(Edge {
                    from,
                    target,
                    origin: EdgeOrigin::Reference(location),
                });
            }
        }
    }
}

fn reference_owner<'source>(
    graph: &mut Graph<'source>,
    index: &HashMap<&Path, FileNodes>,
    location: &'source Location,
) -> NodeId {
    let owner = location.uri.to_file_path().ok().and_then(|path| {
        let file_nodes = index.get(path.as_path())?;
        Some(owner_at(graph, file_nodes, location.range.start))
    });
    owner.unwrap_or_else(|| {
        let node_id = NodeId(graph.nodes.len());
        graph.nodes.push(Node::Unresolved(location));
        node_id
    })
}

fn owner_at(graph: &Graph<'_>, file: &FileNodes, position: Position) -> NodeId {
    file.symbols
        .iter()
        .filter_map(|&node_id| {
            let Some(Node::Symbol { symbol, .. }) = graph.nodes.get(node_id.0) else {
                return None;
            };
            let range = symbol.semantics.execution_range?;
            if !(range.start <= position && position < range.end) {
                return None;
            }
            Some((node_id, symbol.range))
        })
        .max_by_key(|(_, range)| (range.start, Reverse(range.end)))
        .map_or(file.module, |(node_id, _)| node_id)
}

/// References add incoming edges. Starting with reportable production symbols,
/// search their ancestors until no new consumers are discovered. A declaration
/// outside this closure cannot affect any reported status or test chain.
pub fn reference_targets(
    files: &[FileAnalysis],
    source: &Path,
    tests: &TestPaths,
) -> Vec<Vec<bool>> {
    let graph = build(files);
    let roots = report_roots(&graph, source, tests);
    let mut incoming = vec![Vec::new(); graph.nodes.len()];
    for edge in &graph.edges {
        if let Some(consumers) = incoming.get_mut(edge.target.0) {
            consumers.push(edge.from);
        }
    }
    let ancestors = Reachability::walk(&incoming, &roots);
    symbol_mask(files, &graph, &ancestors)
}

fn report_roots(graph: &Graph<'_>, source: &Path, tests: &TestPaths) -> Vec<NodeId> {
    graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node {
            Node::Symbol { path, symbol }
                if path.starts_with(source)
                    && !tests.contains(path)
                    && symbol.semantics.reportable =>
            {
                Some(NodeId(index))
            }
            Node::Module(_) | Node::Symbol { .. } | Node::Unresolved(_) => None,
        })
        .collect()
}

fn symbol_mask(
    files: &[FileAnalysis],
    graph: &Graph<'_>,
    ancestors: &Reachability,
) -> Vec<Vec<bool>> {
    // Graph construction retains file order and declaration order. Unresolved
    // nodes are appended after the collected modules and declarations.
    let mut symbols = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node, Node::Symbol { .. }));
    files
        .iter()
        .map(|file| {
            file.symbols()
                .iter()
                .map(|_| {
                    symbols
                        .next()
                        .is_some_and(|(index, _)| ancestors.contains(NodeId(index)))
                })
                .collect()
        })
        .collect()
}
