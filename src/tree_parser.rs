use std::{fs, fmt, io};
use std::error::Error;
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

use crate::models::{ParsedFile, Position, Reference, ReferenceKind, Scope, ScopeKind, SourceSpan, Symbol, SymbolKind};


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

struct ParseContext<'a> {
    source: &'a str,
    scope: Vec<Scope>,
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
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
        .map_err(|source| ParseError::ConfigureLanguage { source })?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::ParseFailed { path: path.clone() })?;

    let root = tree.root_node();
    if root.has_error() {
        return Err(ParseError::InvalidSyntax { path })
    }

    let mut ctx = ParseContext { 
        source,
        scope: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
    };

    collect_items(root, &mut ctx);

    Ok(
        ParsedFile {
            path,
            symbols: ctx.symbols,
            references: ctx.references
        }
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


fn collect_definition(
    node: tree_sitter::Node<'_>,
    ctx: &mut ParseContext<'_>,
    scope_kind: ScopeKind,
) {
    let name_node = node
        .child_by_field_name("name")
        .expect("definition must have a name");

    let name = &ctx.source[name_node.byte_range()];

    let qualname = ctx.scope
        .iter()
        .map(|scope| scope.name.as_str())
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join(".");

    let kind = match scope_kind {
        ScopeKind::Class => SymbolKind::Class,
        ScopeKind::Function
            if ctx.scope.last().is_some_and(|parent| {
                parent.kind == ScopeKind::Class
            }) =>
        {
            SymbolKind::Method
        }
        ScopeKind::Function => SymbolKind::Function,
    };

    ctx.symbols.push(Symbol {
        name: name.to_owned(),
        qualname,
        kind,
        span: source_span(name_node),
    });

    ctx.scope.push(Scope {
        name: name.to_owned(),
        kind: scope_kind,
    });

    if let Some(body) = node.child_by_field_name("body") {
        collect_items(body, ctx);
    }

    ctx.scope.pop();    
}


fn collect_children(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_items(child, ctx);
    }
}

fn collect_call(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    let Some(function) = node.child_by_field_name("function") else {
        return;
    };

    let name = &ctx.source[function.byte_range()];

    ctx.references.push(Reference {
        name: name.to_owned(), 
        kind: ReferenceKind::Call, 
        span: source_span(function) 
    });
}


fn collect_items(
    node: tree_sitter::Node<'_>,
    ctx: &mut ParseContext<'_>,
) {
     match node.kind() {
        "class_definition" => {
            collect_definition(node, ctx, ScopeKind::Class);
        }
        "function_definition" => {
            collect_definition(node, ctx, ScopeKind::Function);
        }
        "call" => {
            collect_call(node, ctx);
            collect_children(node, ctx);
        },
        // TODO: collect imports? 
        _ => {
            collect_children(node, ctx);
        },
    };

}