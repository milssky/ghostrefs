use crate::Result;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::{Map, Value};

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
    pub fn load(root: &Path) -> Result<Self> {
        let canonical_root = root.canonicalize()?;
        let mut settings = load_settings(&canonical_root)?;
        // Keep ghostrefs-only options out of workspace/didChangeConfiguration.
        let options = settings
            .as_object_mut()
            .ok_or("bundled configuration must be a JSON object")?
            .remove("ghostrefs");
        let (ignore, production_entrypoints) = parse_options(&canonical_root, options)?;
        Ok(Self {
            lsp_settings: settings,
            ignore,
            production_entrypoints,
        })
    }
}

fn load_settings(root: &Path) -> Result<Value> {
    let path = root.join(CONFIG_FILE);
    let mut settings = serde_json::from_str(DEFAULT_SETTINGS)?;
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
    Ok(settings)
}

type ProjectOptions = (IgnoreRules, Vec<EntryPoint>);

fn parse_options(root: &Path, options: Option<Value>) -> Result<ProjectOptions> {
    let defaults = options.unwrap_or_else(|| Value::Object(Map::default()));
    let configuration = defaults
        .as_object()
        .ok_or("ghostrefs config must be an object")?;
    for key in configuration.keys() {
        if !matches!(key.as_str(), "ignore" | "production_entrypoints") {
            return Err(format!("unknown ghostrefs setting: {key}").into());
        }
    }
    let mut entrypoints = Vec::new();
    if let Some(declarations) = configuration.get("production_entrypoints") {
        let entrypoint_list = declarations
            .as_array()
            .ok_or("ghostrefs.production_entrypoints must be an array of objects")?;
        for entrypoint in entrypoint_list {
            entrypoints.push(parse_entry(root, entrypoint)?);
        }
    }
    let ignore = parse_ignore(root, configuration.get("ignore"))?;
    Ok((ignore, entrypoints))
}

fn parse_ignore(root: &Path, setting: Option<&Value>) -> Result<IgnoreRules> {
    let mut patterns = GlobSetBuilder::new();
    if let Some(pattern_list) = setting {
        let globs: Vec<String> = serde_json::from_value(pattern_list.clone())
            .map_err(|error| format!("ghostrefs.ignore must be an array of strings: {error}"))?;
        for pattern in globs {
            let glob = parse_glob(&pattern)?;
            let _ = patterns.add(glob);
        }
    }
    Ok(IgnoreRules {
        root: root.to_path_buf(),
        patterns: patterns.build()?,
    })
}

fn parse_entry(root: &Path, setting: &Value) -> Result<EntryPoint> {
    let entrypoint = setting
        .as_object()
        .ok_or("each production entrypoint must be an object with path and qualname")?;
    for key in entrypoint.keys() {
        if !matches!(key.as_str(), "path" | "qualname") {
            return Err(format!("unknown production entrypoint field: {key}").into());
        }
    }
    let path = entry_string(entrypoint, "path")?;
    let qualname = entry_string(entrypoint, "qualname")?;
    validate_entry_name(path, qualname)?;
    let resolved = resolve_entry_path(root, path)?;
    Ok(EntryPoint {
        path: resolved,
        qualname: qualname.to_owned(),
    })
}

fn validate_entry_name(path: &str, qualname: &str) -> Result<()> {
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
    Ok(())
}

fn resolve_entry_path(root: &Path, path: &str) -> Result<PathBuf> {
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|error| format!("cannot resolve production entrypoint file {path:?}: {error}"))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(
            format!("production entrypoint must point to a file inside root: {path:?}").into(),
        );
    }
    Ok(resolved)
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
        (Value::Object(current_settings), Value::Object(replacements)) => {
            for (key, setting) in replacements {
                match current_settings.get_mut(&key) {
                    Some(current) => merge(current, setting),
                    None => {
                        let _ = current_settings.insert(key, setting);
                    }
                }
            }
        }
        (current, replacement) => *current = replacement,
    }
}

fn parse_glob(pattern: &str) -> Result<Glob> {
    if pattern.is_empty()
        || Path::new(&pattern).is_absolute()
        || pattern.split('/').any(|part| part == "..")
    {
        return Err(
            format!("ignore pattern must be nonempty and relative to root: {pattern:?}").into(),
        );
    }
    let glob = GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(false)
        .build()
        .map_err(|error| format!("invalid ignore pattern {pattern:?}: {error}"))?;
    Ok(glob)
}

fn entry_string<'source>(
    entrypoint: &'source Map<String, Value>,
    key: &str,
) -> Result<&'source str> {
    entrypoint
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("production entrypoint {key} must be a string").into())
}
