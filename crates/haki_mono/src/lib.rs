/// haki_mono — Monomorphization engine for Haki v0.1.
///
/// Takes a `TypedSourceFile` from `haki_typeck` and produces a `MonoProgram`
/// — a fully concrete, flat list of items with no remaining type parameters.
///
/// Algorithm:
///   1. All non-generic items are immediately emitted as roots.
///   2. A worklist tracks generic instantiations discovered during the walk.
///      Each entry is a (name, type-arg-vector) pair — the "specialization key".
///   3. For each worklist entry, the original generic body is cloned and all
///      type-variable occurrences are substituted with the concrete `SemTy`.
///   4. The resulting concrete item is given a mangled name and emitted.
///      Generic struct/class types instantiated in field or variable positions
///      receive the same treatment.
///   5. When the worklist is empty, the `MonoProgram` is complete.
///
/// Output: `MonoProgram` — consumed by `haki_codegen`.

pub mod mono_ast;
pub mod subst;
pub mod engine;

pub use mono_ast::MonoProgram;
pub use engine::MonoEngine;

use haki_typeck::TypedSourceFile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MonoError {
    #[error("unknown generic function `{name}` referenced at instantiation")]
    UnknownGenericFn { name: String },

    #[error("unknown generic type `{name}` referenced at instantiation")]
    UnknownGenericType { name: String },

    #[error("type argument count mismatch for `{name}`: expected {expected}, found {found}")]
    TypeArgCountMismatch { name: String, expected: usize, found: usize },
}

pub type MonoResult<T> = Result<T, MonoError>;

/// Entry point: monomorphize a type-checked source file.
pub fn monomorphize(file: &TypedSourceFile) -> MonoResult<MonoProgram> {
    MonoEngine::new(file).run()
}
