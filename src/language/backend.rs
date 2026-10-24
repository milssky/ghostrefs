use std::path::Path;

use async_lsp::lsp_types::DocumentSymbolResponse;

use crate::{Result, lsp::Server, models::FileAnalysis};

/// Language rules implemented by each backend and selected before collection.
///
/// Backends normalize server declarations into [`FileAnalysis`]; graph construction
/// and usage analysis consume that model without applying language-specific rules.
pub trait Language: Sync {
    /// LSP language identifier sent in `textDocument/didOpen` for every selected file.
    fn language_id(&self) -> &'static str;

    /// Default server whose symbol responses this backend can normalize.
    fn default_server(&self) -> Server;

    /// Whether a file path belongs to this language, without reading its contents.
    /// Used both during scanning and to reject mixed-language collections.
    fn matches_path(&self, path: &Path) -> bool;

    /// Whether scanning should skip this directory and its entire subtree.
    /// Exclusions from all registered backends apply workspace-wide, before selection.
    fn excludes_directory(&self, path: &Path) -> bool;

    /// Validate a configured entrypoint name in the same file-relative format emitted
    /// in [`crate::models::SymbolAnalysis::qualname`], without checking its existence.
    /// Names are matched exactly, without normalization or a module/file prefix.
    /// For example, Python uses dot-separated nesting such as `App.run`, with
    /// nonempty components and no whitespace.
    fn is_valid_qualname(&self, name: &str) -> bool;

    /// Normalize one document-symbol response for the supplied canonical absolute path.
    /// Preserve that path in the result so reference URIs and entrypoints resolve it.
    ///
    /// A successful empty response produces an empty analysis. `None` means the
    /// server supplied no result, not that the file has no declarations: return an
    /// error rather than silently dropping the file's symbols. Reject response shapes
    /// that cannot be faithfully normalized (Python requires a hierarchy unless empty).
    ///
    /// Retain declarations needed as reference owners or implicit-edge endpoints even
    /// when they are not reportable. Populate every symbol's semantics, use LSP
    /// coordinates for all ranges, and initialize references to `None` for collection.
    /// See [`crate::models::SymbolSemantics::execution_range`] for ownership rules.
    /// Compute implicit edges against the final symbol vector before calling
    /// [`FileAnalysis::new`]; its order defines all file-local symbol indices.
    fn normalize_symbols(
        &self,
        response: Option<DocumentSymbolResponse>,
        path: &Path,
    ) -> Result<FileAnalysis>;
}
