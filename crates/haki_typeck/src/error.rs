/// error.rs — All type error variants for Haki v0.1.
///
/// Design principles from CLAUDE.md:
///   - Protocol constraint violations reported at the CALL SITE with named hint.
///   - Errors are precise: span + human message + optional hint.
///   - Ignored return values are hard errors (use `_` to discard explicitly).

use haki_ast::Span;
use thiserror::Error;

pub type TypeResult<T> = Result<T, TypeError>;

#[derive(Debug, Error)]
pub enum TypeError {
    // ── Name resolution ───────────────────────────────────────────────────
    #[error("{span:?}: unknown type `{name}`")]
    UnknownType { name: String, span: Span },

    #[error("{span:?}: unknown variable `{name}`")]
    UnknownVar { name: String, span: Span },

    #[error("{span:?}: unknown function `{name}`")]
    UnknownFn { name: String, span: Span },

    #[error("{span:?}: `{ty}` has no field `{field}`")]
    NoSuchField { ty: String, field: String, span: Span },

    #[error("{span:?}: `{ty}` has no method `{method}`")]
    NoSuchMethod { ty: String, method: String, span: Span },

    // ── Type mismatches ───────────────────────────────────────────────────
    #[error("{span:?}: type mismatch — expected `{expected}`, found `{found}`")]
    TypeMismatch { expected: String, found: String, span: Span },

    #[error("{span:?}: cannot apply `{op}` to `{ty}`")]
    InvalidUnary { op: String, ty: String, span: Span },

    #[error("{span:?}: cannot apply `{op}` to `{lhs}` and `{rhs}`")]
    InvalidBinary { op: String, lhs: String, rhs: String, span: Span },

    // ── Multi-return / binding ────────────────────────────────────────────
    #[error("{span:?}: binding count mismatch — {bindings} bindings for {values} return values")]
    BindingCountMismatch { bindings: usize, values: usize, span: Span },

    #[error("{span:?}: return value ignored — use `_` to discard explicitly")]
    IgnoredReturnValue { span: Span },

    // ── Mutability ────────────────────────────────────────────────────────
    #[error("{span:?}: cannot assign to `const` binding `{name}`")]
    AssignToConst { name: String, span: Span },

    #[error("{span:?}: cannot assign to immutable field `{field}`")]
    AssignToConstField { field: String, span: Span },

    // ── Protocol conformance ──────────────────────────────────────────────
    #[error("{span:?}: `{ty}` does not conform to `{protocol}` — missing method `{method}`")]
    MissingProtocolMethod { ty: String, protocol: String, method: String, span: Span },

    #[error("{span:?}: `{ty}` does not conform to `{protocol}` (required by bound on `{param}`)")]
    ProtocolConstraintViolation { ty: String, protocol: String, param: String, span: Span },

    // ── Generics ──────────────────────────────────────────────────────────
    #[error("{span:?}: wrong number of type arguments to `{name}` — expected {expected}, found {found}")]
    WrongTypeArgCount { name: String, expected: usize, found: usize, span: Span },

    // ── Control flow ──────────────────────────────────────────────────────
    #[error("{span:?}: `return` outside of a function")]
    ReturnOutsideFunction { span: Span },

    #[error("{span:?}: `yield` outside of a block expression")]
    YieldOutsideBlock { span: Span },

    #[error("{span:?}: `if` used as expression but missing `else` branch")]
    IfExprMissingElse { span: Span },

    #[error("{span:?}: both branches of `if` expression must yield the same type (then: `{then_ty}`, else: `{else_ty}`)")]
    IfBranchTypeMismatch { then_ty: String, else_ty: String, span: Span },

    // ── Null / optional ───────────────────────────────────────────────────
    #[error("{span:?}: cannot assign `null` to non-optional type `{ty}`")]
    NullToNonOptional { ty: String, span: Span },

    #[error("{span:?}: wrong number of arguments to `{name}` — expected {expected}, found {found}")]
    WrongArgCount { name: String, expected: usize, found: usize, span: Span },

    // ── Impl / duplicate ──────────────────────────────────────────────────
    #[error("{span:?}: duplicate impl of `{protocol}` for `{ty}`")]
    DuplicateImpl { protocol: String, ty: String, span: Span },

    #[error("{span:?}: `{ty}` is already defined")]
    DuplicateType { ty: String, span: Span },

    // ── Weak references ───────────────────────────────────────────────────
    #[error("{span:?}: `weak` fields must have an optional type (`T?`)")]
    WeakFieldMustBeOptional { span: Span },

    #[error("{span:?}: `weak` can only be applied to class (reference) types, not `{ty}`")]
    WeakOnValueType { ty: String, span: Span },
}
