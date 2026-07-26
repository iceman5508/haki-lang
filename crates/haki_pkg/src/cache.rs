/// cache.rs — ~/.haki/pkg cache management.
///
/// Cache layout:
///   ~/.haki/pkg/
///     utils@a3f92c1d/      ← <alias>@<short-commit>/
///       strings.haki
///       math.haki
///     http@b7e41f2a/
///       server.haki
///       router.haki
///
/// Each package is stored at <name>@<commit> so multiple versions of the
/// same package can coexist without collision across projects.

use std::path::{Path, PathBuf};
use crate::{PkgError, PkgResult};

/// The root of the global Haki package cache.
pub fn cache_root() -> PkgResult<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".haki").join("pkg"))
}

/// The cache directory for a specific locked dependency.
/// Path: ~/.haki/pkg/<alias>@<short-commit>/
pub fn pkg_dir(alias: &str, commit: &str) -> PkgResult<PathBuf> {
    let short = &commit[..commit.len().min(8)];
    Ok(cache_root()?.join(format!("{alias}@{short}")))
}

/// Check whether a package is already cached at the given commit.
pub fn is_cached(alias: &str, commit: &str) -> PkgResult<bool> {
    Ok(pkg_dir(alias, commit)?.exists())
}

/// Ensure the cache root directory exists.
pub fn ensure_cache_root() -> PkgResult<PathBuf> {
    let root = cache_root()?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

/// List all cached packages (name@commit entries).
pub fn list_cached() -> PkgResult<Vec<String>> {
    let root = cache_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut entries = vec![];
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
    }
    entries.sort();
    Ok(entries)
}

/// Remove a specific cached package version.
pub fn remove_cached(alias: &str, commit: &str) -> PkgResult<()> {
    let dir = pkg_dir(alias, commit)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Resolve a pkg/ import path to an absolute filesystem path.
///
/// Given:
///   - import_path: "pkg/utils/strings"
///   - lock: the current project's haki.lock
///
/// Returns the absolute path: ~/.haki/pkg/utils@a3f92c1d/strings.haki
pub fn resolve_pkg_import(
    import_path: &str,
    lock: &crate::HakiLock,
) -> PkgResult<PathBuf> {
    // Strip "pkg/" prefix
    let rest = import_path
        .strip_prefix("pkg/")
        .ok_or_else(|| PkgError::NotFound(format!("not a pkg/ import: {import_path}")))?;

    // First component is the package alias
    let (alias, module_path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    // Look up in lock file
    let locked = lock.get(alias).ok_or_else(|| {
        PkgError::NotFound(format!(
            "package '{alias}' not found in haki.lock — run `hakic pkg install`"
        ))
    })?;

    let pkg_path = pkg_dir(alias, &locked.commit)?;

    let file_path = if module_path.is_empty() {
        // import "pkg/utils" → utils/mod.haki or utils.haki
        let mod_file = pkg_path.join("mod.haki");
        if mod_file.exists() {
            mod_file
        } else {
            pkg_path.with_extension("haki")
        }
    } else {
        pkg_path.join(format!("{module_path}.haki"))
    };

    Ok(file_path)
}

/// Cross-platform home directory resolution.
fn home_dir() -> PkgResult<PathBuf> {
    // std::env::home_dir is deprecated; use env vars directly
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home));
    }
    // Windows
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return Ok(PathBuf::from(userprofile));
    }
    // Windows fallback
    if let (Ok(drive), Ok(path)) = (
        std::env::var("HOMEDRIVE"),
        std::env::var("HOMEPATH"),
    ) {
        return Ok(PathBuf::from(format!("{drive}{path}")));
    }
    Err(PkgError::Cache("cannot determine home directory".into()))
}

/// Validate a package alias (lowercase, hyphens/underscores, no spaces).
pub fn validate_alias(alias: &str) -> PkgResult<()> {
    if alias.is_empty() {
        return Err(PkgError::Manifest("package alias cannot be empty".into()));
    }
    if !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(PkgError::Manifest(format!(
            "invalid package alias '{alias}': use only letters, numbers, hyphens, underscores"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkg_dir_path() {
        let dir = pkg_dir("utils", "a3f92c1d8b5e4f2a").unwrap();
        let s = dir.to_string_lossy();
        assert!(s.contains("utils@a3f92c1"));
    }

    #[test]
    fn test_validate_alias_valid() {
        assert!(validate_alias("utils").is_ok());
        assert!(validate_alias("my-pkg").is_ok());
        assert!(validate_alias("haki_utils").is_ok());
    }

    #[test]
    fn test_validate_alias_invalid() {
        assert!(validate_alias("").is_err());
        assert!(validate_alias("my pkg").is_err());
        assert!(validate_alias("pkg/utils").is_err());
    }

    #[test]
    fn test_resolve_pkg_import() {
        use crate::lock::{HakiLock, LockedDep};

        let mut lock = HakiLock::new();
        lock.lock("utils", LockedDep {
            url:     "https://github.com/user/haki-utils".into(),
            git_ref: "main".into(),
            commit:  "a3f92c1d00000000".into(),
        });

        let path = resolve_pkg_import("pkg/utils/strings", &lock).unwrap();
        let s = path.to_string_lossy();
        assert!(s.contains("utils@a3f92c1d"));
        assert!(s.ends_with("strings.haki"));
    }
}
