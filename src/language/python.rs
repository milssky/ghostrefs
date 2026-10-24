//! Python symbol semantics, independent of the LSP transport and graph traversal.

use std::path::Path;

use async_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse, Range, SymbolKind};

use crate::{
    Result,
    lsp::Server,
    models::{FileAnalysis, ImplicitEdge, SymbolAnalysis, SymbolIndex, SymbolSemantics},
};

use super::backend::Language;

pub struct Python;

impl Language for Python {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn default_server(&self) -> Server {
        Server::Pyright
    }

    fn matches_path(&self, path: &Path) -> bool {
        path.extension().is_some_and(|extension| extension == "py")
    }

    fn excludes_directory(&self, path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| matches!(name.to_str(), Some(".venv" | "venv" | "__pycache__")))
    }

    fn is_valid_qualname(&self, name: &str) -> bool {
        name.split('.')
            .all(|part| !part.is_empty() && !part.chars().any(char::is_whitespace))
    }

    fn normalize_symbols(
        &self,
        response: Option<DocumentSymbolResponse>,
        path: &Path,
    ) -> Result<FileAnalysis> {
        let symbols = flatten_symbols(nested_declarations(response, path)?);
        let implicit_edges = class_member_edges(&symbols);
        FileAnalysis::new(path.to_path_buf(), symbols, implicit_edges)
    }
}

fn class_member_edges(symbols: &[SymbolAnalysis]) -> Vec<ImplicitEdge> {
    // A class reference can construct an instance or expose any direct member.
    // LSP references omit implicit __init__, descriptors and dynamic dispatch.
    let mut edges = Vec::new();
    for (from, class) in symbols.iter().enumerate() {
        if class.kind != SymbolKind::CLASS {
            continue;
        }
        for (target, member) in symbols.iter().enumerate() {
            if member
                .qualname
                .rsplit_once('.')
                .is_some_and(|(parent, _)| parent == class.qualname)
            {
                edges.push(ImplicitEdge {
                    from: SymbolIndex(from),
                    target: SymbolIndex(target),
                    reason: "class member",
                });
            }
        }
    }
    edges
}

const fn is_scope(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CLASS | SymbolKind::CONSTRUCTOR
    )
}

fn nested_declarations(
    response: Option<DocumentSymbolResponse>,
    path: &Path,
) -> Result<Vec<DocumentSymbol>> {
    match response {
        Some(DocumentSymbolResponse::Nested(declarations)) => Ok(declarations),
        Some(DocumentSymbolResponse::Flat(declarations)) if declarations.is_empty() => {
            Ok(Vec::new())
        }
        Some(DocumentSymbolResponse::Flat(_)) => Err(format!(
            "expected hierarchical document symbols for {}",
            path.display()
        )
        .into()),
        None => Err(format!(
            "language server returned no document-symbol result for {}",
            path.display()
        )
        .into()),
    }
}

/// Preserve declaration order while flattening nested names in depth-first order.
fn flatten_symbols(declarations: Vec<DocumentSymbol>) -> Vec<SymbolAnalysis> {
    let mut pending: Vec<_> = declarations
        .into_iter()
        .rev()
        .map(|symbol| (symbol, String::new()))
        .collect();
    let mut symbols = Vec::new();
    while let Some((symbol, parent)) = pending.pop() {
        let flattened = flatten_symbol(&symbol, &parent);
        if let Some(children) = symbol.children {
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (child, flattened.qualname.clone())),
            );
        }
        symbols.push(flattened);
    }
    symbols
}

fn flatten_symbol(symbol: &DocumentSymbol, parent: &str) -> SymbolAnalysis {
    let qualname = if parent.is_empty() {
        symbol.name.clone()
    } else {
        format!("{parent}.{}", symbol.name)
    };
    let semantics = symbol_semantics(symbol, &qualname);
    SymbolAnalysis {
        qualname,
        kind: symbol.kind,
        range: symbol.range,
        selection_range: symbol.selection_range,
        references: None,
        semantics,
    }
}

fn symbol_semantics(symbol: &DocumentSymbol, qualname: &str) -> SymbolSemantics {
    SymbolSemantics {
        reportable: is_scope(symbol.kind) || !qualname.contains('.'),
        top_level: !qualname.contains('.'),
        entrypoint: symbol.name == "main",
        // Pyright includes decorators in declaration ranges. They execute
        // in the enclosing scope, before the decorated name.
        execution_range: is_scope(symbol.kind).then_some(Range {
            start: symbol.selection_range.start.max(symbol.range.start),
            end: symbol.range.end,
        }),
    }
}
