/// git.rs — Git subprocess interop for hakic pkg.
///
/// All git operations are done by shelling out to the system `git` binary.
/// No libgit2 dependency — git is universally available and this keeps
/// the hakic binary small.

use std::path::Path;
use std::process::Command;
use crate::{PkgError, PkgResult};

/// Clone a repository into a target directory.
/// If a ref (branch or tag) is specified, pass it via --branch.
/// Uses --depth=1 for efficiency unless a specific commit is needed.
pub fn clone(url: &str, git_ref: Option<&str>, dest: &Path) -> PkgResult<()> {
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    cmd.arg("--depth=1");

    if let Some(r) = git_ref {
        cmd.arg("--branch").arg(r);
    }

    cmd.arg(url);
    cmd.arg(dest);

    run_git(&mut cmd, &format!("git clone {url}"))?;
    Ok(())
}

/// Get the current HEAD commit SHA of a local repository.
pub fn head_commit(repo: &Path) -> PkgResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Err(PkgError::Git(format!(
            "git rev-parse failed in {}",
            repo.display()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the default branch name of a remote repository (main or master).
pub fn default_branch(url: &str) -> PkgResult<String> {
    let output = Command::new("git")
        .args(["ls-remote", "--symref", url, "HEAD"])
        .output()?;

    if !output.status.success() {
        return Ok("main".into()); // safe default
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output looks like: "ref: refs/heads/main\tHEAD"
    for line in stdout.lines() {
        if line.starts_with("ref: refs/heads/") {
            if let Some(branch) = line
                .strip_prefix("ref: refs/heads/")
                .and_then(|s| s.split('\t').next())
            {
                return Ok(branch.to_string());
            }
        }
    }

    Ok("main".into())
}

/// Check if git is available on PATH.
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map_or(false, |o| o.status.success())
}

/// Fetch the latest changes in a cloned repo (for update).
pub fn fetch(repo: &Path) -> PkgResult<()> {
    run_git(
        Command::new("git").args(["fetch", "--depth=1"]).current_dir(repo),
        "git fetch",
    )
}

/// Reset a repo to the latest of its tracked remote branch (for update).
pub fn pull(repo: &Path) -> PkgResult<()> {
    run_git(
        Command::new("git").args(["pull", "--ff-only"]).current_dir(repo),
        "git pull",
    )
}

/// Run a git command and convert failure to PkgError::Git.
fn run_git(cmd: &mut Command, desc: &str) -> PkgResult<()> {
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PkgError::Git(format!(
            "{desc} failed: {}",
            stderr.trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_available() {
        // git should be present in the build environment
        assert!(git_available(), "git not found on PATH");
    }
}
