use std::path::{Path, PathBuf};

use super::{backend::Language, python::Python};

const LANGUAGES: &[&dyn Language] = &[&Python];

/// Select one backend and validate the remaining files against it.
pub fn detect(files: &[PathBuf]) -> Result<&'static dyn Language, String> {
    let first = files
        .first()
        .ok_or_else(|| "no supported source files found".to_owned())?;
    let language =
        from_path(first).ok_or_else(|| format!("unsupported language: {}", first.display()))?;
    for file in files {
        if !language.matches_path(file) {
            return Err(format!(
                "expected files of one language ({}): {}",
                language.language_id(),
                file.display()
            ));
        }
    }
    Ok(language)
}

pub fn from_path(path: &Path) -> Option<&'static dyn Language> {
    LANGUAGES
        .iter()
        .copied()
        .find(|language| language.matches_path(path))
}

pub fn supports_path(path: &Path) -> bool {
    LANGUAGES.iter().any(|language| language.matches_path(path))
}

pub fn excludes_directory(path: &Path) -> bool {
    LANGUAGES
        .iter()
        .any(|language| language.excludes_directory(path))
}
