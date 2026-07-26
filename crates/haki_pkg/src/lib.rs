/// haki_pkg — Package manager for Haki.
///
/// Owns all package manager logic:
///   - Manifest types (HakiJson, HakiLock)
///   - JSON parsing and serialization
///   - Git interop (clone, checkout, resolve commit)
///   - Cache path management (~/.haki/pkg/<name>@<commit>/)
///   - Import resolution (pkg/ namespace → cache path)
///   - CLI command dispatch (add, install, update, list)

pub mod manifest;
pub mod lock;
pub mod cache;
pub mod git;
pub mod resolver;
pub mod commands;

pub use manifest::HakiJson;
pub use lock::HakiLock;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum PkgError {
    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("lock error: {0}")]
    Lock(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("package not found: {0}")]
    NotFound(String),

    #[error("dependency conflict: {0}")]
    Conflict(String),
}

pub type PkgResult<T> = Result<T, PkgError>;
