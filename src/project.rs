use std::fs;
use std::io;
use std::path::Path;

use crate::language::{Language, ParseError};
use crate::models::{ParsedDir, ParsedFile};


#[derive(Debug)]
pub enum ProjectError {
    Io(io::Error),
    Parse(ParseError),
}

impl From<io::Error> for ProjectError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ParseError> for ProjectError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Parse(error) => write!(f, "{error}")
        }
    }
}

pub fn parse_dir(path: &Path) -> Result<ParsedDir, ProjectError> {
    let mut parsed_files_buf = Vec::<ParsedFile>::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        // NOTE: is_dir() follows symlinks. It may cause
        // infinite recursion.
        if path.is_dir() {
            let nested = parse_dir(&path)?;
            parsed_files_buf.extend(nested.files);
        } else if let Some(language) = Language::from_path(&path) {
            parsed_files_buf.push(language.parse_file(&path)?);
        }
    }
    Ok(ParsedDir{
        path: path.to_path_buf(),
        files: parsed_files_buf,
    })
}
