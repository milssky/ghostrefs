use std::error::Error;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};
use tree_sitter::Parser;

use crate::models::ParsedFile;

#[derive(Debug)]
pub enum ParseError {
    ReadFile { path: PathBuf, source: io::Error },
    ConfigureLanguage { source: tree_sitter::LanguageError },
    ParseFailed { path: PathBuf },
    InvalidSyntax { path: PathBuf },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
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
    let source = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path.to_owned(),
        source,
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
        return Err(ParseError::InvalidSyntax { path });
    }

    let (symbols, references) = super::collector::collect(root, source);

    Ok(ParsedFile {
        path,
        symbols,
        references,
    })
}
