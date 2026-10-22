use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Range, SymbolKind};

/// A canonical absolute file path and a qualified name, such as `App.run`.
#[derive(Debug)]
pub(crate) struct EntryPoint {
    pub path: PathBuf,
    pub qualname: String,
}

#[derive(Debug)]
pub(crate) struct FileAnalysis {
    pub path: PathBuf,
    pub symbols: Vec<SymbolAnalysis>,
}

#[derive(Debug)]
pub(crate) struct SymbolAnalysis {
    pub name: String,
    pub qualname: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub references: Option<Vec<Location>>,
}

impl SymbolAnalysis {
    /// Nested variables and attributes are graph leaves, not report candidates.
    pub(crate) fn is_reportable(&self) -> bool {
        self.is_scope() || !self.qualname.contains('.')
    }

    /// Functions, methods and classes own references inside their execution scope.
    pub(crate) const fn is_scope(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CLASS | SymbolKind::CONSTRUCTOR
        )
    }
}
