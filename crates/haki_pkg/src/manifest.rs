/// manifest.rs — haki.json parsing and serialization.
///
/// haki.json format:
/// {
///   "name": "myapp",
///   "version": "1.0.0",
///   "dependencies": {
///     "utils": "https://github.com/user/haki-utils",
///     "http":  "https://github.com/user/haki-http#v2.1.0",
///     "auth":  "https://github.com/user/haki-auth#experimental-branch"
///   }
/// }

use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::{PkgError, PkgResult};

pub const MANIFEST_FILE: &str = "haki.json";

/// The parsed content of a haki.json file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HakiJson {
    /// Package name — lowercase, hyphens allowed.
    pub name: String,

    /// Semantic version string, e.g. "1.0.0".
    pub version: String,

    /// Dependencies: alias → URL (with optional #ref fragment).
    /// BTreeMap for deterministic serialization order.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

impl HakiJson {
    /// Create a new manifest for a fresh project.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".into(),
            dependencies: BTreeMap::new(),
        }
    }

    /// Read and parse haki.json from a directory.
    pub fn read(dir: &Path) -> PkgResult<Self> {
        let path = dir.join(MANIFEST_FILE);
        if !path.exists() {
            return Err(PkgError::Manifest(format!(
                "no haki.json found in {}",
                dir.display()
            )));
        }
        let src = std::fs::read_to_string(&path)?;
        let manifest: Self = serde_json::from_str(&src)
            .map_err(|e| PkgError::Manifest(format!("invalid haki.json: {e}")))?;
        Ok(manifest)
    }

    /// Write haki.json to a directory (pretty-printed).
    pub fn write(&self, dir: &Path) -> PkgResult<()> {
        let path = dir.join(MANIFEST_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json + "\n")?;
        Ok(())
    }

    /// Add or update a dependency. Returns true if it was newly added.
    pub fn add_dep(&mut self, alias: impl Into<String>, url: impl Into<String>) -> bool {
        let alias = alias.into();
        let is_new = !self.dependencies.contains_key(&alias);
        self.dependencies.insert(alias, url.into());
        is_new
    }

    /// Remove a dependency. Returns true if it existed.
    pub fn remove_dep(&mut self, alias: &str) -> bool {
        self.dependencies.remove(alias).is_some()
    }
}

/// A parsed dependency URL — base URL + optional ref (branch/tag/commit).
#[derive(Debug, Clone)]
pub struct DepUrl {
    /// The base git URL, e.g. "https://github.com/user/repo"
    pub url: String,

    /// Optional ref fragment, e.g. "v2.1.0" from "#v2.1.0"
    /// If None, the default branch (main/master) is used.
    pub git_ref: Option<String>,
}

impl DepUrl {
    /// Parse a dependency URL string, splitting off the #fragment if present.
    pub fn parse(raw: &str) -> Self {
        if let Some(idx) = raw.find('#') {
            let (url, fragment) = raw.split_at(idx);
            Self {
                url: url.to_string(),
                git_ref: Some(fragment[1..].to_string()), // strip leading #
            }
        } else {
            Self {
                url: raw.to_string(),
                git_ref: None,
            }
        }
    }

    /// The repo name — last path component of the URL without .git suffix.
    pub fn repo_name(&self) -> String {
        self.url
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_no_ref() {
        let d = DepUrl::parse("https://github.com/user/haki-utils");
        assert_eq!(d.url, "https://github.com/user/haki-utils");
        assert_eq!(d.git_ref, None);
        assert_eq!(d.repo_name(), "haki-utils");
    }

    #[test]
    fn test_parse_url_with_tag() {
        let d = DepUrl::parse("https://github.com/user/haki-http#v2.1.0");
        assert_eq!(d.url, "https://github.com/user/haki-http");
        assert_eq!(d.git_ref, Some("v2.1.0".into()));
    }

    #[test]
    fn test_parse_url_with_branch() {
        let d = DepUrl::parse("https://github.com/user/haki-auth#experimental");
        assert_eq!(d.git_ref, Some("experimental".into()));
    }

    #[test]
    fn test_manifest_roundtrip() {
        let dir = std::env::temp_dir().join("haki_manifest_test");
        std::fs::create_dir_all(&dir).unwrap();

        let mut m = HakiJson::new("testpkg");
        m.add_dep("utils", "https://github.com/user/haki-utils");
        m.add_dep("http",  "https://github.com/user/haki-http#v1.0.0");
        m.write(&dir).unwrap();

        let m2 = HakiJson::read(&dir).unwrap();
        assert_eq!(m2.name, "testpkg");
        assert_eq!(m2.dependencies.len(), 2);
        assert_eq!(m2.dependencies["utils"], "https://github.com/user/haki-utils");
        assert_eq!(m2.dependencies["http"],  "https://github.com/user/haki-http#v1.0.0");
    }
}
