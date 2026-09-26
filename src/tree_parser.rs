use std::{fs, fmt, io};
use std::error::Error;
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

use crate::models::{ParsedFile, Position, SourceSpan, Symbol, ScopeKind, SymbolKind, Scope};


#[derive(Debug)]
pub enum ParseError {
    ReadFile {
        path: PathBuf,
        source: io::Error,
    },
    ConfigureLanguage {
        source: tree_sitter::LanguageError,
    },
    ParseFailed {
        path: PathBuf,
    },
    InvalidSyntax {
        path: PathBuf,
    }
}


impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f,  "failed to read {}: {source}", path.display())
            }
            Self::ConfigureLanguage { source } => {
                write!(f, "failed to configure Python parser: {source}")
            }
            Self::ParseFailed { path } => {
                write!(f, "failed to parse {}", path.display())
            }
            Self::InvalidSyntax { path } => {
                write!(f, "invalid Python syntax in {}", path.display())
            }
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ConfigureLanguage { source } => Some(source),
            Self::ParseFailed { .. } | Self::InvalidSyntax { .. } => None,
        }
    }
}


pub fn parse_file(path: &Path) -> Result<ParsedFile, ParseError> {
    let source = fs::read_to_string(path).map_err(|source| {
        ParseError::ReadFile { 
            path: path.to_owned(), 
            source,
        }
    })?;
    parse_source(path.to_owned(), &source)
    
}

pub fn parse_source(path: PathBuf, source: &str) -> Result<ParsedFile, ParseError> {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("failed to load Python grammar");

    let tree = parser
        .parse(&source, None)
        .expect("failed to parse");

    let root = tree.root_node();
    if root.has_error() {
        return Err(ParseError::InvalidSyntax { path })
    }

    let mut symbols = Vec::new();
    let mut scope = Vec::new();

    collect_symbols(root, source, &mut scope, &mut symbols);

    Ok(
        ParsedFile { path, symbols: symbols, references: Vec::new() }
    )
}


fn source_span(node: tree_sitter::Node<'_>) -> SourceSpan {
    let range = node.range();
    SourceSpan { 
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        start: Position {
            row: range.start_point.row,
            column: range.start_point.column,
        },
        end: Position { 
            row: range.end_point.row, 
            column: range.end_point.column, 
        }, 
    }
}

fn collect_symbols(
    node: tree_sitter::Node<'_>,
    source: &str,
    scope: &mut Vec<Scope>,
    symbols: &mut Vec<Symbol>,
) {
    let scope_kind = match node.kind() {
        "class_definition" => Some(ScopeKind::Class),
        "function_definition" => Some(ScopeKind::Function),
        _ => None,
    };

    if let Some(scope_kind) = scope_kind {
        let name_node = node
            .child_by_field_name("name")
            .expect("definition must have a name");

        let name = &source[name_node.byte_range()];

        let qualname = scope
            .iter()
            .map(|scope| scope.name.as_str())
            .chain(std::iter::once(name))
            .collect::<Vec<_>>()
            .join(".");

        let kind = match scope_kind {
            ScopeKind::Class => SymbolKind::Class,
            ScopeKind::Function
                if scope.last().is_some_and(|parent| {
                    parent.kind == ScopeKind::Class
                }) =>
            {
                SymbolKind::Method
            }
            ScopeKind::Function => SymbolKind::Function,
        };

        symbols.push(Symbol {
            name: name.to_owned(),
            qualname,
            kind,
            // Для отчёта удобнее позиция имени, а не всего тела.
            span: source_span(name_node),
        });

        scope.push(Scope {
            name: name.to_owned(),
            kind: scope_kind,
        });

        if let Some(body) = node.child_by_field_name("body") {
            collect_symbols(body, source, scope, symbols);
        }

        scope.pop();
        return;
    }

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_symbols(child, source, scope, symbols);
    }
}