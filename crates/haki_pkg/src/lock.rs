/// lock.rs — haki.lock parsing and serialization.
///
/// haki.lock format:
/// {
///   "utils": {
///     "url":    "https://github.com/user/haki-utils",
///     "ref":    "main",
///     "commit": "a3f92c1d8b5e..."
///   },
///   "http": {
///     "url":    "https://github.com/user/haki-http",
///     "ref":    "v2.1.0",
///     "commit": "b7e41f2a9c3d..."
///   }
/// }
///
/// The lock file is the single source of truth for reproducible builds.
/// The compiler reads it at compile time to resolve pkg/ imports.

use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::{PkgError, PkgResult};

pub const LOCK_FILE: &str = "haki.lock";

/// A single locked dependency entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedDep {
    /// The base git URL.
    pub url: String,

    /// The ref that was resolved (branch name, tag, or "main").
    #[serde(rename = "ref")]
    pub git_ref: String,

    /// The exact commit SHA that was checked out.
    /// This is the canonical identifier used in the cache path.
    pub commit: String,
}

impl LockedDep {
    /// The short commit hash (first 8 chars) used in cache directory names.
    pub fn short_commit(&self) -> &str {
        &self.commit[..self.commit.len().min(8)]
    }
}

/// The parsed content of a haki.lock file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HakiLock {
    /// Map of alias → locked dependency.
    /// BTreeMap for deterministic serialization order.
    #[serde(flatten)]
    pub packages: BTreeMap<String, LockedDep>,
}

impl HakiLock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read and parse haki.lock from a directory.
    /// Returns an empty lock if the file doesn't exist yet.
    pub fn read(dir: &Path) -> PkgResult<Self> {
        let path = dir.join(LOCK_FILE);
        if !path.exists() {
            return Ok(Self::new());
        }
        let src = std::fs::read_to_string(&path)?;
        let lock: Self = serde_json::from_str(&src)
            .map_err(|e| PkgError::Lock(format!("invalid haki.lock: {e}")))?;
        Ok(lock)
    }

    /// Write haki.lock to a directory (pretty-printed, sorted).
    pub fn write(&self, dir: &Path) -> PkgResult<()> {
        let path = dir.join(LOCK_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json + "\n")?;
        Ok(())
    }

    /// Add or update a locked dependency.
    pub fn lock(&mut self, alias: impl Into<String>, dep: LockedDep) {
        self.packages.insert(alias.into(), dep);
    }

    /// Remove a locked dependency.
    pub fn unlock(&mut self, alias: &str) {
        self.packages.remove(alias);
    }

    /// Look up a locked dependency by alias.
    pub fn get(&self, alias: &str) -> Option<&LockedDep> {
        self.packages.get(alias)
    }

    /// Check if a dependency is locked.
    pub fn is_locked(&self, alias: &str) -> bool {
        self.packages.contains_key(alias)
    }

    /// Find any aliases that are in the lock but not in the manifest deps.
    /// Used to detect stale lock entries.
    pub fn stale_entries<'a>(
        &'a self,
        manifest_deps: &'a BTreeMap<String, String>,
    ) -> Vec<&'a str> {
        self.packages
            .keys()
            .filter(|k| !manifest_deps.contains_key(*k))
            .map(|k| k.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_roundtrip() {
        let dir = std::env::temp_dir().join("haki_lock_test");
        std::fs::create_dir_all(&dir).unwrap();

        let mut lock = HakiLock::new();
        lock.lock("utils", LockedDep {
            url:     "https://github.com/user/haki-utils".into(),
            git_ref: "main".into(),
            commit:  "a3f92c1d8b5e4f2a9c3d7e1b6f8a0c2d4e5f6a7b".into(),
        });
        lock.lock("http", LockedDep {
            url:     "https://github.com/user/haki-http".into(),
            git_ref: "v2.1.0".into(),
            commit:  "b7e41f2a9c3d5e7f1b3d5f7a9c1e3f5a7b9c1d3e".into(),
        });
        lock.write(&dir).unwrap();

        let lock2 = HakiLock::read(&dir).unwrap();
        assert_eq!(lock2.packages.len(), 2);

        let utils = lock2.get("utils").unwrap();
        assert_eq!(utils.git_ref, "main");
        assert_eq!(utils.short_commit(), "a3f92c1d");

        let http = lock2.get("http").unwrap();
        assert_eq!(http.git_ref, "v2.1.0");
    }

    #[test]
    fn test_stale_detection() {
        let mut lock = HakiLock::new();
        lock.lock("utils", LockedDep {
            url: "https://github.com/u/r".into(),
            git_ref: "main".into(),
            commit: "abc123".into(),
        });
        lock.lock("old", LockedDep {
            url: "https://github.com/u/old".into(),
            git_ref: "main".into(),
            commit: "def456".into(),
        });

        let mut deps = BTreeMap::new();
        deps.insert("utils".into(), "https://github.com/u/r".into());
        // "old" is in lock but not manifest

        let stale = lock.stale_entries(&deps);
        assert_eq!(stale, vec!["old"]);
    }

    #[test]
    fn test_empty_lock_on_missing_file() {
        let dir = std::env::temp_dir().join("haki_lock_missing");
        std::fs::create_dir_all(&dir).unwrap();
        // Don't write a lock file — should return empty
        let lock = HakiLock::read(&dir).unwrap();
        assert_eq!(lock.packages.len(), 0);
    }
}
