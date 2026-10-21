use std::{
    collections::{HashMap, VecDeque},
    fmt,
    path::Path,
};

use async_lsp::lsp_types::{Location, Position, SymbolKind};

use crate::models::{EntryPoint, FileAnalysis, SymbolAnalysis};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug)]
pub enum Node<'a> {
    Module(&'a Path),
    Symbol {
        path: &'a Path,
        symbol: &'a SymbolAnalysis,
    },
    /// The referencing document was not collected, so its owner is unknown.
    Unresolved(&'a Location),
}

impl fmt::Display for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Module(path) => write!(f, "{}::<module>", path.display()),
            Self::Symbol { path, symbol } => write!(
                f,
                "{}:{}:{}::{}",
                path.display(),
                symbol.selection_range.start.line + 1,
                symbol.selection_range.start.character + 1,
                symbol.qualname,
            ),
            Self::Unresolved(location) => write!(
                f,
                "{}:{}:{}::<unresolved>",
                location.uri,
                location.range.start.line + 1,
                location.range.start.character + 1,
            ),
        }
    }
}

#[derive(Debug)]
pub struct Edge<'a> {
    pub from: NodeId,
    pub to: NodeId,
    pub location: Option<&'a Location>,
}

#[derive(Debug)]
pub struct Graph<'a> {
    pub nodes: Vec<Node<'a>>,
    /// Consumer-to-symbol edges. A missing location marks conservative class activation.
    pub edges: Vec<Edge<'a>>,
}

impl Graph<'_> {
    /// Resolve an exact qualified name within a collected file.
    pub fn resolve_entry(&self, entry: &EntryPoint) -> Result<NodeId, String> {
        let matches: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| match node {
                Node::Symbol { path, symbol }
                    if *path == entry.path && symbol.qualname == entry.qualname =>
                {
                    Some(NodeId(index))
                }
                _ => None,
            })
            .collect();

        match matches.as_slice() {
            [id] => Ok(*id),
            [] => Err(format!(
                "entry point not found: {}::{}",
                entry.path.display(),
                entry.qualname,
            )),
            _ => Err(format!(
                "ambiguous entry point {}::{}; candidates: {}",
                entry.path.display(),
                entry.qualname,
                matches
                    .iter()
                    .map(|id| self.nodes[id.0].to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
        }
    }
}

pub struct Connections {
    pub outgoing: Vec<Vec<NodeId>>,
    pub consumed: Vec<bool>,
}

impl Connections {
    pub fn new(graph: &Graph<'_>) -> Self {
        let mut result = Self {
            outgoing: vec![Vec::new(); graph.nodes.len()],
            consumed: vec![false; graph.nodes.len()],
        };
        for edge in &graph.edges {
            result.outgoing[edge.from.0].push(edge.to);
            if edge.from != edge.to {
                result.consumed[edge.to.0] = true;
            }
        }
        result
    }
}

pub struct Reachability {
    pub reached: Vec<bool>,
    parents: Vec<Option<NodeId>>,
}

impl Reachability {
    pub fn walk(outgoing: &[Vec<NodeId>], roots: &[NodeId]) -> Self {
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

    pub fn chain(&self, target: NodeId) -> Vec<NodeId> {
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
    for file in files {
        add_class_member_edges(&mut graph, &index[file.path.as_path()]);
    }
    graph
}

fn index_files<'a>(
    graph: &mut Graph<'a>,
    files: &'a [FileAnalysis],
) -> HashMap<&'a Path, FileNodes> {
    let mut index = HashMap::new();
    for file in files {
        let module = NodeId(graph.nodes.len());
        graph.nodes.push(Node::Module(&file.path));

        let symbols = file
            .symbols
            .iter()
            .map(|symbol| {
                let id = NodeId(graph.nodes.len());
                graph.nodes.push(Node::Symbol {
                    path: &file.path,
                    symbol,
                });
                id
            })
            .collect();

        index.insert(file.path.as_path(), FileNodes { module, symbols });
    }
    index
}

fn add_reference_edges<'a>(
    graph: &mut Graph<'a>,
    files: &'a [FileAnalysis],
    index: &HashMap<&Path, FileNodes>,
) {
    for file in files {
        let targets = &index[file.path.as_path()];
        for (symbol, &to) in file.symbols.iter().zip(&targets.symbols) {
            for location in symbol.references.iter().flatten() {
                let from = reference_owner(graph, index, location);

                graph.edges.push(Edge {
                    from,
                    to,
                    location: Some(location),
                });
            }
        }
    }
}

fn reference_owner<'a>(
    graph: &mut Graph<'a>,
    index: &HashMap<&Path, FileNodes>,
    location: &'a Location,
) -> NodeId {
    let owner = location.uri.to_file_path().ok().and_then(|path| {
        let file_nodes = index.get(path.as_path())?;
        Some(owner_at(graph, file_nodes, location.range.start))
    });
    owner.unwrap_or_else(|| {
        let id = NodeId(graph.nodes.len());
        graph.nodes.push(Node::Unresolved(location));
        id
    })
}

// A class reference can construct an instance or expose any of its methods.
// Activate direct members conservatively: LSP references do not describe
// implicit __init__, descriptors, or dynamic method dispatch.
fn add_class_member_edges(graph: &mut Graph<'_>, file: &FileNodes) {
    for &from in &file.symbols {
        let Node::Symbol { symbol: class, .. } = &graph.nodes[from.0] else {
            continue;
        };
        if class.kind != SymbolKind::CLASS {
            continue;
        }
        for &to in &file.symbols {
            let Node::Symbol { symbol: member, .. } = &graph.nodes[to.0] else {
                continue;
            };
            if member
                .qualname
                .rsplit_once('.')
                .is_some_and(|(parent, _)| parent == class.qualname)
            {
                graph.edges.push(Edge {
                    from,
                    to,
                    location: None,
                });
            }
        }
    }
}

fn owner_at(graph: &Graph<'_>, file: &FileNodes, position: Position) -> NodeId {
    file.symbols
        .iter()
        .filter_map(|&id| {
            let Node::Symbol { symbol, .. } = &graph.nodes[id.0] else {
                return None;
            };
            // Variables and parameters are targets, not execution scopes.
            if !symbol.is_scope()
                || !(symbol.range.start <= position && position < symbol.range.end)
                // Pyright includes decorators in the declaration range. They
                // execute in the enclosing scope, before the decorated name.
                || position < symbol.selection_range.start
            {
                return None;
            }
            Some((id, symbol.range))
        })
        .max_by_key(|(_, range)| (range.start, std::cmp::Reverse(range.end)))
        .map_or(file.module, |(id, _)| id)
}

/// References add incoming edges. Starting with reportable production symbols,
/// search their ancestors until no new consumers are discovered. A declaration
/// outside this closure cannot affect any reported status or test chain.
pub fn reference_targets(files: &[FileAnalysis], source: &Path, tests: &Path) -> Vec<Vec<bool>> {
    let graph = build(files);
    let roots: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node {
            Node::Symbol { path, symbol }
                if path.starts_with(source)
                    && !path.starts_with(tests)
                    && symbol.is_reportable() =>
            {
                Some(NodeId(index))
            }
            _ => None,
        })
        .collect();
    let mut incoming = vec![Vec::new(); graph.nodes.len()];
    for edge in &graph.edges {
        incoming[edge.to.0].push(edge.from);
    }
    let ancestors = Reachability::walk(&incoming, &roots);
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
            file.symbols
                .iter()
                .map(|_| {
                    let (index, _) = symbols
                        .next()
                        .expect("graph contains every collected symbol");
                    ancestors.reached[index]
                })
                .collect()
        })
        .collect()
}
