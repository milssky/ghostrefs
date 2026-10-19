use std::path::Path;

use crate::models::ParsedFile;

mod python;

#[derive(Debug, Clone, Copy)]
pub enum Language {
    Python,
}

impl Language {
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
