use std::path::{Path, PathBuf};

use async_lsp::lsp_types::{Location, Position, Range, SymbolKind};

use crate::Result;

/// A canonical absolute file path and a qualified name, such as `App.run`.
#[derive(Debug)]
pub struct EntryPoint {
    /// Canonical absolute path identifying the collected file.
    pub path: PathBuf,
    /// Exact file-relative name in the backend's format; see [`SymbolAnalysis::qualname`].
    pub qualname: String,
}

/// Normalized declarations and inferred activation edges for one collected file.
/// Declaration order is frozen at construction so edge indices remain valid.
#[derive(Debug)]
pub struct FileAnalysis {
    path: PathBuf,
    symbols: Vec<SymbolAnalysis>,
    implicit_edges: Vec<ImplicitEdge>,
}

impl FileAnalysis {
    /// Freeze the final symbol order and validate all implicit-edge endpoints.
    ///
    /// `path` must be the canonical absolute path used by reference URIs. Each edge
    /// index addresses `symbols` in this file, including non-reportable declarations.
    /// Names, ranges and semantics must already satisfy the backend contract; this
    /// constructor only validates edge bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if either endpoint of any edge is outside `symbols`.
    pub fn new(
        path: PathBuf,
        symbols: Vec<SymbolAnalysis>,
        implicit_edges: Vec<ImplicitEdge>,
    ) -> Result<Self> {
        for (edge_index, edge) in implicit_edges.iter().enumerate() {
            let invalid = [("from", edge.from), ("target", edge.target)]
                .into_iter()
                .find(|(_, index)| index.0 >= symbols.len());
            if let Some((endpoint, index)) = invalid {
                return Err(format!(
                    "invalid implicit edge {edge_index} ({}) in {}: {endpoint} symbol index {} is out of bounds for {} symbols",
                    edge.reason,
                    path.display(),
                    index.0,
                    symbols.len(),
                )
                .into());
            }
        }
        Ok(Self {
            path,
            symbols,
            implicit_edges,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn symbols(&self) -> &[SymbolAnalysis] {
        &self.symbols
    }

    pub fn implicit_edges(&self) -> &[ImplicitEdge] {
        &self.implicit_edges
    }

    /// Update reference results in symbol order without changing declarations.
    pub fn reference_slots_mut(&mut self) -> impl Iterator<Item = ReferenceSlot<'_>> {
        self.symbols
            .iter_mut()
            .map(|symbol| (symbol.selection_range.start, &mut symbol.references))
    }
}

/// A reference query position and its mutable result, in declaration order.
pub type ReferenceSlot<'symbol> = (Position, &'symbol mut Option<Vec<Location>>);

/// A position in the final symbol list passed to `FileAnalysis::new`.
/// Indices belong to that file; they are not graph node IDs or source offsets.
/// Compute them after filtering/reordering declarations; both edge endpoints must
/// be strictly less than the final symbol count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolIndex(pub usize);

/// Activation between declarations in the same file's frozen symbol order.
/// An edge `from -> target` means reaching `from` also keeps `target` alive,
/// even when the server reports no explicit reference between them.
#[derive(Debug, Clone, Copy)]
pub struct ImplicitEdge {
    /// Declaration whose activation consumes the target.
    pub from: SymbolIndex,
    /// Declaration activated by the source.
    pub target: SymbolIndex,
    /// Human-readable explanation included in graph diagnostics.
    pub reason: &'static str,
}

/// A declaration retained for graph construction, whether or not it is reportable.
/// All ranges use the same LSP document coordinates as server reference locations.
#[derive(Debug)]
pub struct SymbolAnalysis {
    /// File-relative qualified name in the backend's entrypoint syntax, without a
    /// module/file prefix (for example, Python's `App.run` or `outer.inner`).
    /// Resolution uses exact string equality; duplicate names are retained and
    /// cause an ambiguity error when selected as an entrypoint.
    pub qualname: String,
    /// LSP declaration kind; language-specific usage rules belong in `semantics`.
    pub kind: SymbolKind,
    /// Full declaration extent. For overlapping execution ranges, graph ownership
    /// prefers the declaration with the latest start, then the earliest end.
    pub range: Range,
    /// Declared name's range within `range`; its start is the LSP reference-query
    /// position and the declaration's displayed location.
    pub selection_range: Range,
    /// Incoming reference locations, excluding the declaration itself.
    /// Initially `None`; after a query, `None` means the server supplied no result
    /// and usage remains unknown, whereas `Some(vec![])` is a known empty result.
    /// Collection may also mark queries proven unnecessary as `Some(vec![])`.
    pub references: Option<Vec<Location>>,
    /// Backend decisions consumed directly by the language-independent analysis.
    pub semantics: SymbolSemantics,
}

/// Language rules normalized before graph construction and usage analysis.
#[derive(Debug, Clone, Copy)]
pub struct SymbolSemantics {
    /// Eligible for findings when the file is in production scope and not ignored.
    /// False still retains the declaration in the graph.
    pub reportable: bool,
    /// Eligible for conservative production-root inference when reference results
    /// are known and the declaration is unconsumed or marked as an entrypoint.
    pub top_level: bool,
    /// Treat a top-level production declaration as an inferred entrypoint even if
    /// consumed. This flag alone does not make a nested declaration a root.
    pub entrypoint: bool,
    /// Half-open range `[start, end)` whose reference starts execute in this scope.
    /// Must be contained in the declaration's `range`, using the same LSP coordinates.
    /// Exclude portions that execute in the enclosing scope, such as Python decorators.
    ///
    /// Nested scopes may overlap: ownership prefers the containing candidate with
    /// the latest declaration start, then the earliest declaration end. Use properly
    /// nested declaration ranges so this selects the innermost scope. `None` means
    /// this declaration never owns references; they belong to another enclosing
    /// execution range, or to the file's module if no range contains them.
    pub execution_range: Option<Range>,
}
