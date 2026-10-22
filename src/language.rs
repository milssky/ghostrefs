use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Language {
    Python,
}

pub(crate) struct LspConfig {
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub language_id: &'static str,
}

impl Language {
    /// Detect a shared language, rejecting empty or mixed file lists.
    pub(crate) fn detect(files: &[PathBuf]) -> Result<Self, String> {
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

    pub(crate) fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "py" => Some(Self::Python),
            _ => None,
        }
    }

    pub(crate) const fn lsp_config(self) -> LspConfig {
        match self {
            Self::Python => LspConfig {
                command: "pyright-langserver",
                args: &["--stdio"],
                language_id: "python",
            },
        }
    }
}
