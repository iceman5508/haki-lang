/// commands.rs — CLI command implementations for hakic pkg.
///
/// hakic pkg add <url>           Add a dependency
/// hakic pkg add <url> as <name> Add with explicit alias
/// hakic pkg install             Install all dependencies
/// hakic pkg update [name]       Update one or all dependencies
/// hakic pkg list                List installed packages
/// hakic pkg remove <name>       Remove a dependency

use std::path::{Path, PathBuf};
use crate::{
    HakiJson, HakiLock,
    cache, git,
    lock::LockedDep,
    manifest::DepUrl,
    PkgError, PkgResult,
};

// ── hakic pkg add ─────────────────────────────────────────────────────────────

/// Add a dependency to the project.
///
/// url: the dependency URL (with optional #ref fragment)
/// alias: optional explicit alias; if None, derived from repo name
/// project_dir: directory containing haki.json
pub fn cmd_add(url: &str, alias: Option<&str>, project_dir: &Path) -> PkgResult<()> {
    if !git::git_available() {
        return Err(PkgError::Git(
            "git not found on PATH — install git to use hakic pkg".into()
        ));
    }

    let mut manifest = HakiJson::read(project_dir)?;
    let dep_url = DepUrl::parse(url);

    // Resolve alias: explicit > repo name
    let alias = alias
        .map(|s| s.to_string())
        .unwrap_or_else(|| dep_url.repo_name());

    cache::validate_alias(&alias)?;

    // Check for conflicts
    if manifest.dependencies.contains_key(&alias) {
        eprintln!("warning: overwriting existing dependency '{alias}'");
    }

    // Fetch and lock the dependency
    let locked = fetch_and_lock(&alias, &dep_url)?;

    // Update manifest and lock
    manifest.add_dep(&alias, url);
    manifest.write(project_dir)?;

    let mut lock = HakiLock::read(project_dir)?;
    lock.lock(&alias, locked.clone());
    lock.write(project_dir)?;

    eprintln!("  added   {alias} @ {} ({})", locked.git_ref, &locked.commit[..8]);
    Ok(())
}

// ── hakic pkg install ─────────────────────────────────────────────────────────

/// Install all dependencies listed in haki.json.
/// Skips packages already cached at the locked commit.
pub fn cmd_install(project_dir: &Path) -> PkgResult<()> {
    if !git::git_available() {
        return Err(PkgError::Git(
            "git not found on PATH — install git to use hakic pkg".into()
        ));
    }

    let manifest = HakiJson::read(project_dir)?;
    let mut lock = HakiLock::read(project_dir)?;

    if manifest.dependencies.is_empty() {
        eprintln!("no dependencies to install");
        return Ok(());
    }

    let mut installed = 0;
    let mut skipped  = 0;

    for (alias, url) in &manifest.dependencies {
        cache::validate_alias(alias)?;
        let dep_url = DepUrl::parse(url);

        // If already locked and cached, skip
        if let Some(locked) = lock.get(alias) {
            if cache::is_cached(alias, &locked.commit)? {
                eprintln!("  cached  {alias} @ {} ({})", locked.git_ref, &locked.commit[..8]);
                skipped += 1;
                continue;
            }
        }

        // Fetch and lock
        let locked = fetch_and_lock(alias, &dep_url)?;
        eprintln!("  fetched {alias} @ {} ({})", locked.git_ref, &locked.commit[..8]);
        lock.lock(alias.clone(), locked);
        installed += 1;
    }

    // Remove stale lock entries
    let stale: Vec<String> = lock
        .stale_entries(&manifest.dependencies)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for alias in &stale {
        lock.unlock(alias);
        eprintln!("  removed stale lock entry: {alias}");
    }

    lock.write(project_dir)?;

    eprintln!("\n{installed} installed, {skipped} cached");
    Ok(())
}

// ── hakic pkg update ──────────────────────────────────────────────────────────

/// Update one or all dependencies to their latest commit.
pub fn cmd_update(alias: Option<&str>, project_dir: &Path) -> PkgResult<()> {
    if !git::git_available() {
        return Err(PkgError::Git(
            "git not found on PATH — install git to use hakic pkg".into()
        ));
    }

    let manifest = HakiJson::read(project_dir)?;
    let mut lock = HakiLock::read(project_dir)?;

    let to_update: Vec<(String, String)> = if let Some(a) = alias {
        let url = manifest.dependencies.get(a).ok_or_else(|| {
            PkgError::NotFound(format!("'{a}' not in haki.json"))
        })?;
        vec![(a.to_string(), url.clone())]
    } else {
        manifest.dependencies.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };

    for (alias, url) in to_update {
        let dep_url = DepUrl::parse(&url);
        let locked = fetch_and_lock(&alias, &dep_url)?;
        eprintln!("  updated {alias} → {} ({})", locked.git_ref, &locked.commit[..8]);
        lock.lock(alias, locked);
    }

    lock.write(project_dir)?;
    Ok(())
}

// ── hakic pkg list ────────────────────────────────────────────────────────────

/// List all installed packages.
pub fn cmd_list(project_dir: &Path) -> PkgResult<()> {
    let manifest = HakiJson::read(project_dir)?;
    let lock = HakiLock::read(project_dir)?;

    if manifest.dependencies.is_empty() {
        eprintln!("no dependencies");
        return Ok(());
    }

    eprintln!("{} dependencies:\n", manifest.dependencies.len());
    for (alias, url) in &manifest.dependencies {
        let dep_url = DepUrl::parse(url);
        if let Some(locked) = lock.get(alias) {
            let cached = cache::is_cached(alias, &locked.commit)?;
            let status = if cached { "✓" } else { "✗ not cached" };
            eprintln!(
                "  {status}  {alias:<20} {} @ {}",
                dep_url.url,
                &locked.commit[..8]
            );
        } else {
            eprintln!("  ?  {alias:<20} {} (not installed)", dep_url.url);
        }
    }
    Ok(())
}

// ── hakic pkg remove ──────────────────────────────────────────────────────────

/// Remove a dependency from the project.
pub fn cmd_remove(alias: &str, project_dir: &Path) -> PkgResult<()> {
    let mut manifest = HakiJson::read(project_dir)?;
    let mut lock = HakiLock::read(project_dir)?;

    if !manifest.remove_dep(alias) {
        return Err(PkgError::NotFound(format!("'{alias}' not in haki.json")));
    }

    lock.unlock(alias);

    manifest.write(project_dir)?;
    lock.write(project_dir)?;

    eprintln!("  removed {alias}");
    Ok(())
}

// ── hakic pkg init ────────────────────────────────────────────────────────────

/// Initialize a new haki.json in the current directory.
pub fn cmd_init(name: Option<&str>, project_dir: &Path) -> PkgResult<()> {
    let manifest_path = project_dir.join(crate::manifest::MANIFEST_FILE);
    if manifest_path.exists() {
        return Err(PkgError::Manifest(
            "haki.json already exists in this directory".into()
        ));
    }

    let pkg_name = name.unwrap_or_else(|| {
        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("myapp")
    });

    let manifest = HakiJson::new(pkg_name);
    manifest.write(project_dir)?;
    eprintln!("  created haki.json for '{pkg_name}'");
    Ok(())
}

// ── Internal: fetch and lock ──────────────────────────────────────────────────

/// Clone a dependency to a temp dir, resolve the commit, move to cache.
fn fetch_and_lock(alias: &str, dep_url: &DepUrl) -> PkgResult<LockedDep> {
    let cache_root = cache::ensure_cache_root()?;

    // Clone to a temp location first
    let temp_dir = cache_root.join(format!(".tmp-{alias}"));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }

    git::clone(&dep_url.url, dep_url.git_ref.as_deref(), &temp_dir)?;

    // Resolve the actual commit SHA
    let commit = git::head_commit(&temp_dir)?;

    // Determine the ref label
    let git_ref = dep_url.git_ref.clone().unwrap_or_else(|| {
        git::default_branch(&dep_url.url).unwrap_or_else(|_| "main".into())
    });

    // Move to the final cache location: <alias>@<short-commit>
    let final_dir = cache::pkg_dir(alias, &commit)?;
    if final_dir.exists() {
        // Already cached at this exact commit — discard the temp clone
        std::fs::remove_dir_all(&temp_dir)?;
    } else {
        std::fs::rename(&temp_dir, &final_dir)
            .or_else(|_| {
                // rename fails across filesystems — fall back to copy+delete
                copy_dir_all(&temp_dir, &final_dir)?;
                std::fs::remove_dir_all(&temp_dir)?;
                Ok::<(), PkgError>(())
            })?;
    }

    Ok(LockedDep { url: dep_url.url.clone(), git_ref, commit })
}

/// Recursively copy a directory (fallback when rename crosses filesystems).
fn copy_dir_all(src: &Path, dst: &Path) -> PkgResult<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// `hakic pkg publish` — publish package to pkg.haki-lang.org registry.
///
/// In v3.7 this is a stub that validates the manifest and shows what would be published.
/// Full registry integration ships in v4.0 when pkg.haki-lang.org is live.
pub fn cmd_publish(project_dir: &Path) -> PkgResult<()> {
    let manifest = crate::manifest::HakiJson::read(project_dir)?;

    // Validate required fields
    manifest.validate_for_publish()
        .map_err(|e| crate::PkgError::Manifest(e))?;

    eprintln!("hakic pkg: validating {}@{}", manifest.name, manifest.version);
    eprintln!();
    eprintln!("  name:        {}", manifest.name);
    eprintln!("  version:     {}", manifest.version);
    eprintln!("  description: {}", manifest.description);
    eprintln!("  license:     {}", manifest.license);
    if !manifest.author.is_empty() {
        eprintln!("  author:      {}", manifest.author);
    }
    if !manifest.repository.is_empty() {
        eprintln!("  repository:  {}", manifest.repository);
    }
    if !manifest.keywords.is_empty() {
        eprintln!("  keywords:    {}", manifest.keywords.join(", "));
    }
    eprintln!("  deps:        {}", manifest.dependencies.len());
    eprintln!();
    eprintln!("pkg.haki-lang.org registry launches with v4.0.");
    eprintln!("Run `hakic pkg publish` again after v4.0 to push to the registry.");
    eprintln!();
    eprintln!("✓  manifest valid — ready for v4.0 registry submission");

    Ok(())
}
