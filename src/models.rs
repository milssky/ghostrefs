use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Range, SymbolKind};

/// A canonical absolute file path and a qualified name, such as `App.run`.
#[derive(Debug)]
pub struct EntryPoint {
    pub path: PathBuf,
    pub qualname: String,
}

#[derive(Debug)]
pub struct FileAnalysis {
    pub path: PathBuf,
    pub symbols: Vec<SymbolAnalysis>,
}

#[derive(Debug)]
pub struct SymbolAnalysis {
    pub name: String,
    pub qualname: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub references: Option<Vec<Location>>,
}

impl SymbolAnalysis {
    /// Nested variables and attributes are graph leaves, not report candidates.
    pub fn is_reportable(&self) -> bool {
        self.is_scope() || !self.qualname.contains('.')
    }

    /// Functions, methods and classes own references inside their execution scope.
    pub const fn is_scope(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CLASS | SymbolKind::CONSTRUCTOR
        )
    }
}
