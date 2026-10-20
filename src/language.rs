use std::path::{Path, PathBuf};

use crate::models::ParsedFile;

mod python;

const DEFAULT_LSP_SETTINGS: &str = include_str!("../ghostrefs-lsp.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
}

pub struct LspConfig {
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub language_id: &'static str,
    pub settings: serde_json::Value,
}

impl Language {
    /// Detect a shared language, rejecting empty or mixed file lists.
    pub fn detect(files: &[PathBuf]) -> Result<Self, String> {
        let first = files
            .first()
            .ok_or_else(|| "no supported source files found".to_owned())?;
        let language = Self::from_path(first)
            .ok_or_else(|| format!("unsupported language: {}", first.display()))?;

        for file in files {
            if Self::from_path(file) != Some(language) {
                return Err(format!(
                    "expected files of one language ({language:?}): {}",
                    file.display()
                ));
            }
        }

        Ok(language)
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "py" => Some(Self::Python),
            _ => None,
        }
    }

    pub fn parse_file(self, path: &Path) -> Result<ParsedFile, ParseError> {
        match self {
            Self::Python => python::parse_file(path).map_err(ParseError::Python),
        }
    }

    pub fn lsp_config(self) -> Result<LspConfig, serde_json::Error> {
        Ok(match self {
            Self::Python => LspConfig {
                command: "pyright-langserver",
                args: &["--stdio"],
                language_id: "python",
                settings: serde_json::from_str(DEFAULT_LSP_SETTINGS)?,
            },
        })
    }
}

#[derive(Debug)]
pub enum ParseError {
    Python(python::ParseError),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Python(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Python(error) => Some(error),
        }
    }
}
