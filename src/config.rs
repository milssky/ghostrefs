use std::{
    fs, io,
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

use crate::models::EntryPoint;

const DEFAULT_SETTINGS: &str = include_str!("../ghostrefs-lsp.json");
const CONFIG_FILE: &str = "ghostrefs-lsp.json";

pub struct Config {
    pub lsp_settings: Value,
    pub ignore: IgnoreRules,
    pub production_entrypoints: Vec<EntryPoint>,
}

pub struct IgnoreRules {
    root: PathBuf,
    patterns: GlobSet,
}

impl Config {
    /// Project settings override bundled defaults; nested objects are merged.
    pub fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let root = root.canonicalize()?;
        let path = root.join(CONFIG_FILE);
        let mut settings: Value = serde_json::from_str(DEFAULT_SETTINGS)?;

        match fs::read_to_string(&path) {
            Ok(text) => {
                let overrides: Value = serde_json::from_str(&text)
                    .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
                if !overrides.is_object() {
                    return Err(format!("config {} must be a JSON object", path.display()).into());
                }
                merge(&mut settings, overrides);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("cannot read config {}: {error}", path.display()).into());
            }
        }

        // Keep ghostrefs-only options out of workspace/didChangeConfiguration.
        let ghostrefs = settings
            .as_object_mut()
            .ok_or("bundled configuration must be a JSON object")?
            .remove("ghostrefs");
        let mut patterns = GlobSetBuilder::new();
        let mut production_entrypoints = Vec::new();
        if let Some(options) = ghostrefs {
            let options = options
                .as_object()
                .ok_or("ghostrefs config must be an object")?;
            for key in options.keys() {
                if !matches!(key.as_str(), "ignore" | "production_entrypoints") {
                    return Err(format!("unknown ghostrefs setting: {key}").into());
                }
            }
            if let Some(entries) = options.get("production_entrypoints") {
                let entries = entries
                    .as_array()
                    .ok_or("ghostrefs.production_entrypoints must be an array of objects")?;
                for entry in entries {
                    production_entrypoints.push(parse_entry(&root, entry)?);
                }
            }
            if let Some(ignore) = options.get("ignore") {
                let globs: Vec<String> =
                    serde_json::from_value(ignore.clone()).map_err(|error| {
                        format!("ghostrefs.ignore must be an array of strings: {error}")
                    })?;
                for pattern in globs {
                    if pattern.is_empty()
                        || Path::new(&pattern).is_absolute()
                        || pattern.split('/').any(|part| part == "..")
                    {
                        return Err(format!(
                            "ignore pattern must be nonempty and relative to root: {pattern:?}"
                        )
                        .into());
                    }
                    let glob = GlobBuilder::new(&pattern)
                        .literal_separator(true)
                        .backslash_escape(false)
                        .build()
                        .map_err(|error| format!("invalid ignore pattern {pattern:?}: {error}"))?;
                    patterns.add(glob);
                }
            }
        }

        Ok(Self {
            lsp_settings: settings,
            production_entrypoints,
            ignore: IgnoreRules {
                root,
                patterns: patterns.build()?,
            },
        })
    }
}

fn parse_entry(root: &Path, value: &Value) -> Result<EntryPoint, Box<dyn std::error::Error>> {
    let entry = value
        .as_object()
        .ok_or("each production entrypoint must be an object with path and qualname")?;
    for key in entry.keys() {
        if !matches!(key.as_str(), "path" | "qualname") {
            return Err(format!("unknown production entrypoint field: {key}").into());
        }
    }
    let path = entry
        .get("path")
        .and_then(Value::as_str)
        .ok_or("production entrypoint path must be a string")?;
    let qualname = entry
        .get("qualname")
        .and_then(Value::as_str)
        .ok_or("production entrypoint qualname must be a string")?;
    if path.is_empty() || Path::new(path).is_absolute() {
        return Err(format!(
            "production entrypoint path must be nonempty and relative to root: {path:?}"
        )
        .into());
    }
    if qualname
        .split('.')
        .any(|part| part.is_empty() || part.chars().any(char::is_whitespace))
    {
        return Err(format!("invalid production entrypoint qualname: {qualname:?}").into());
    }
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|error| format!("cannot resolve production entrypoint file {path:?}: {error}"))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(
            format!("production entrypoint must point to a file inside root: {path:?}").into(),
        );
    }
    Ok(EntryPoint {
        path: resolved,
        qualname: qualname.to_owned(),
    })
}

impl IgnoreRules {
    /// Patterns match file paths relative to the analyzed project root.
    pub fn matches(&self, path: &Path) -> bool {
        path.strip_prefix(&self.root)
            .is_ok_and(|relative| self.patterns.is_match(relative))
    }
}

fn merge(settings: &mut Value, overrides: Value) {
    match (settings, overrides) {
        (Value::Object(settings), Value::Object(overrides)) => {
            for (key, value) in overrides {
                match settings.get_mut(&key) {
                    Some(current) => merge(current, value),
                    None => {
                        settings.insert(key, value);
                    }
                }
            }
        }
        (settings, overrides) => *settings = overrides,
    }
}
