/// resolver.rs — pkg/ import namespace resolver.
///
/// Hooks into the Haki compiler's module resolution.
/// When the compiler encounters `import "pkg/utils/strings"`,
/// it calls resolve_import() which maps it to the absolute cache path.
///
/// Resolution algorithm:
///   1. Strip "pkg/" prefix → "utils/strings"
///   2. First component is package alias → "utils"
///   3. Remaining path is module path within package → "strings"
///   4. Look up alias in haki.lock → commit = "a3f92c1d"
///   5. Return ~/.haki/pkg/utils@a3f92c1d/strings.haki

use std::path::{Path, PathBuf};
use crate::{HakiLock, PkgError, PkgResult};
use crate::cache;

/// Check if an import path is a pkg/ import.
pub fn is_pkg_import(path: &str) -> bool {
    path.starts_with("pkg/")
}

/// Resolve a pkg/ import to an absolute filesystem path.
///
/// Reads haki.lock from the project directory to find the commit hash.
/// Returns the absolute path to the .haki file.
pub fn resolve_import(
    import_path: &str,
    project_dir: &Path,
) -> PkgResult<PathBuf> {
    let lock = HakiLock::read(project_dir)?;
    cache::resolve_pkg_import(import_path, &lock)
}

/// Resolve a pkg/ import using an already-loaded lock file.
/// More efficient when resolving multiple imports in one compilation.
pub fn resolve_with_lock(
    import_path: &str,
    lock: &HakiLock,
) -> PkgResult<PathBuf> {
    cache::resolve_pkg_import(import_path, lock)
}

/// Find the project root (directory containing haki.json) by
/// walking up from the given path.
pub fn find_project_root(from: &Path) -> Option<PathBuf> {
    let mut dir = if from.is_file() {
        from.parent()?.to_path_buf()
    } else {
        from.to_path_buf()
    };

    loop {
        if dir.join(crate::manifest::MANIFEST_FILE).exists() {
            return Some(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Validate that all pkg/ imports in a source file have locked dependencies.
/// Returns a list of missing aliases.
pub fn find_missing_deps(
    source: &str,
    lock: &HakiLock,
) -> Vec<String> {
    let mut missing = vec![];
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import") { continue; }
        // Extract the path from: import "pkg/..." or import "pkg/..." as alias
        if let Some(start) = trimmed.find('"') {
            if let Some(end) = trimmed[start + 1..].find('"') {
                let path = &trimmed[start + 1..start + 1 + end];
                if is_pkg_import(path) {
                    let alias = path
                        .strip_prefix("pkg/")
                        .and_then(|s| s.split('/').next())
                        .unwrap_or("");
                    if !alias.is_empty() && !lock.is_locked(alias) {
                        missing.push(alias.to_string());
                    }
                }
            }
        }
    }
    missing.dedup();
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::{HakiLock, LockedDep};

    fn make_lock() -> HakiLock {
        let mut lock = HakiLock::new();
        lock.lock("utils", LockedDep {
            url: "https://github.com/user/utils".into(),
            git_ref: "main".into(),
            commit: "a3f92c1d00000000".into(),
        });
        lock
    }

    #[test]
    fn test_is_pkg_import() {
        assert!(is_pkg_import("pkg/utils/strings"));
        assert!(is_pkg_import("pkg/http"));
        assert!(!is_pkg_import("./local"));
        assert!(!is_pkg_import("std/math"));
    }

    #[test]
    fn test_resolve_with_lock() {
        let lock = make_lock();
        let path = resolve_with_lock("pkg/utils/strings", &lock).unwrap();
        let s = path.to_string_lossy();
        assert!(s.contains("utils@a3f92c1d"));
        assert!(s.ends_with("strings.haki"));
    }

    #[test]
    fn test_missing_alias() {
        let lock = make_lock();
        let path = resolve_with_lock("pkg/missing/something", &lock);
        assert!(path.is_err());
        let msg = path.unwrap_err().to_string();
        assert!(msg.contains("missing"));
        assert!(msg.contains("haki.lock"));
    }

    #[test]
    fn test_find_missing_deps() {
        let lock = make_lock();
        let source = r#"
import "pkg/utils/strings" as strings
import "pkg/http/server" as http
import "./local" as local
"#;
        let missing = find_missing_deps(source, &lock);
        assert_eq!(missing, vec!["http"]);
    }
}
