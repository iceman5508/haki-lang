/// mono_ast.rs — The monomorphized (concrete) AST.
///
/// All type parameters have been eliminated. Every name is concrete.
/// Generic types like `Stack<int>` become `Stack__int`.
/// This is the final AST shape that `haki_codegen` operates on.

use std::collections::HashMap;
use haki_ast::{BinaryOp, Binding, EnumDef, Ident, Mut, Span, UnaryOp};
use haki_typeck::typed_ast::SemTy;

// ── Concrete type ──────────────────────────────────────────────────────────────

/// A monomorphized type — structurally identical to `SemTy` but guaranteed
/// to contain no `SemTy::Named("T")` type variables. Every `Named` refers
/// to a real concrete type.
///
/// We reuse `SemTy` directly rather than defining a new enum, because
/// `SemTy` already covers all needed concrete forms. The contract is:
/// after monomorphization, no `SemTy::Var(_)` or opaque `Named("T")`
/// appears in the output.
pub type ConcrTy = SemTy;

// ── Monomorphized program ─────────────────────────────────────────────────────

/// The fully monomorphized program — the output of the mono pass.
#[derive(Debug, Clone)]
pub struct MonoProgram {
    pub structs: Vec<MonoStruct>,
    pub classes: Vec<MonoClass>,
    pub fns: Vec<MonoFn>,
    pub impls: Vec<MonoImpl>,
    /// Enum definitions carried through for codegen discriminant lookup.
    pub enum_defs: HashMap<String, EnumDef>,
    /// Extern function declarations — emitted as Wasm imports.
    pub extern_fns: Vec<haki_ast::ExternFnDef>,
    /// Top-level const declarations: (name, type, value_expr).
    pub global_consts: Vec<(String, SemTy, MonoExpr)>,
}

impl MonoProgram {
    pub fn new() -> Self {
        Self {
            structs: Vec::new(),
            classes: Vec::new(),
            fns: Vec::new(),
            impls: Vec::new(),
            enum_defs: HashMap::new(),
            extern_fns: Vec::new(),
            global_consts: Vec::new(),
        }
    }
}

impl Default for MonoProgram {
    fn default() -> Self {
        Self::new()
    }
}

// ── Concrete struct ───────────────────────────────────────────────────────────

/// A monomorphized struct.
///
/// Name may be mangled: `Stack__int` for `Stack<int>`.
/// All field types are concrete.
#[derive(Debug, Clone)]
pub struct MonoStruct {
    /// Mangled or original name, e.g. `Stack__int` or `Point`.
    pub name: String,
    pub fields: Vec<MonoField>,
    pub methods: Vec<MonoFn>,
    pub span: Span,
}

// ── Concrete class ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoClass {
    pub name: String,
    pub superclass: Option<String>,
    pub fields: Vec<MonoField>,
    pub methods: Vec<MonoFn>,
    pub span: Span,
}

// ── Concrete field ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoField {
    pub name: String,
    pub ty: ConcrTy,
    pub mutability: Mut,
    pub is_weak: bool,
    pub span: Span,
}

// ── Concrete impl block ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoImpl {
    pub protocol: String,
    pub target: String,
    pub methods: Vec<MonoFn>,
    pub span: Span,
}

// ── Concrete function ─────────────────────────────────────────────────────────

/// A fully concrete function — no type params, all types resolved.
#[derive(Debug, Clone)]
pub struct MonoFn {
    pub name: String,
    pub params: Vec<MonoParam>,
    pub return_ty: ConcrTy,
    pub body: MonoBlock,
    pub span: Span,
    /// Captured variables for closures: (name, type, is_weak).
    /// Empty for plain function pointers.
    pub captures: Vec<(String, ConcrTy, bool, bool)>,  // (name, ty, weak, mutable)
    /// Attributes from the source declaration (threaded through for codegen).
    pub attributes: Vec<haki_ast::Attribute>,
}

#[derive(Debug, Clone)]
pub struct MonoParam {
    pub name: String,
    pub ty: ConcrTy,
    pub span: Span,
}

// ── Concrete block / statements ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoBlock {
    pub stmts: Vec<MonoStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MonoStmt {
    pub kind: MonoStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MonoStmtKind {
    Let(MonoLetStmt),
    Return(MonoReturnStmt),
    Yield(Box<MonoExpr>),
    Defer(Box<MonoExpr>),
    Continue,
    Break,
    If(MonoIf),
    For(MonoFor),
    Select(MonoSelect),
    While(MonoWhile),
    Match(MonoMatch),
    Panic(Box<MonoExpr>),
    Expr(Box<MonoExpr>),
}

#[derive(Debug, Clone)]
pub struct MonoLetStmt {
    pub mutability: Mut,
    /// Bindings paired with their concrete types.
    pub bindings: Vec<(Binding, ConcrTy)>,
    pub init: Box<MonoExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MonoReturnStmt {
    pub values: Vec<MonoExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MonoSelect {
    /// (binding_ident, binding_ty, channel_expr, body)
    pub arms:    Vec<(haki_ast::Ident, ConcrTy, MonoExpr, MonoBlock)>,
    pub timeout: Option<(Box<MonoExpr>, MonoBlock)>,
    pub span:    Span,
}

#[derive(Debug, Clone)]
pub struct MonoFor {
    pub index_var: Option<Ident>,  // `i` in `for i, v in arr`
    pub var: Ident,
    pub var_ty: ConcrTy,
    pub iter: Box<MonoExpr>,
    pub body: MonoBlock,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MonoWhile {
    pub cond: Box<MonoExpr>,
    pub body: MonoBlock,
    pub span: Span,
}

// ── Concrete if / match ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoIf {
    pub cond: Box<MonoExpr>,
    pub then_block: MonoBlock,
    pub else_branch: Option<MonoElse>,
    pub ty: ConcrTy,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MonoElse {
    Block(MonoBlock),
    If(Box<MonoIf>),
}

#[derive(Debug, Clone)]
pub struct MonoMatch {
    pub scrutinee: Box<MonoExpr>,
    pub arms: Vec<MonoArm>,
    pub ty: ConcrTy,
    pub span: Span,
    /// What kind of match this is — drives codegen strategy.
    pub kind: MonoMatchKind,
}

/// The kind of match expression — determines how codegen emits it.
#[derive(Debug, Clone, PartialEq)]
pub enum MonoMatchKind {
    /// Enum discriminant switch (LLVM: switch on i64 tag; C: switch(tag))
    Enum,
    /// Class hierarchy dispatch (pointer type check)
    Class,
    /// Integer literal match (LLVM: switch on i64; C: switch(int))
    Int,
    /// String literal match (LLVM/C: if-else strcmp chain)
    String,
}

/// The pattern in a mono match arm.
#[derive(Debug, Clone)]
pub enum MonoPattern {
    /// Named pattern: variant name, class name, or "_" wildcard.
    Named(String),
    /// Integer literal pattern.
    Int(i64),
    /// String literal pattern.
    String(String),
}

#[derive(Debug, Clone)]
pub struct MonoArm {
    pub pattern: MonoPattern,
    pub bindings: Vec<Ident>,
    pub binding_tys: Vec<ConcrTy>,
    /// Optional guard: `case x if x > 0 { ... }`
    pub guard: Option<MonoExpr>,
    pub body: MonoBlock,
    pub span: Span,
}

// ── Concrete expression ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MonoExpr {
    pub kind: MonoExprKind,
    pub ty: ConcrTy,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MonoExprKind {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
    Var(String),             // resolved variable reference
    Unary(UnaryOp, Box<MonoExpr>),
    Binary(BinaryOp, Box<MonoExpr>, Box<MonoExpr>),
    Field(Box<MonoExpr>, String),
    /// `recv?.field` — null-safe field access
    OptionalField(Box<MonoExpr>, String),
    /// `recv?.method(args)` — null-safe method call
    OptionalMethodCall(Box<MonoExpr>, String, Vec<MonoExpr>),
    /// Call a concrete (possibly mangled) function.
    Call(String, Vec<MonoExpr>),
    /// Construct a type with named arguments.
    Construct(String, Vec<MonoNamedArg>),
    Index(Box<MonoExpr>, Box<MonoExpr>),
    If(Box<MonoIf>),
    Match(Box<MonoMatch>),
    Block(Box<MonoBlock>),
    Array(Vec<MonoExpr>),
    Assign(Box<MonoExpr>, Box<MonoExpr>),
    /// `async expr` — dispatches call to thread pool, produces Task<T>
    Async(Box<MonoExpr>),
}

#[derive(Debug, Clone)]
pub struct MonoNamedArg {
    pub name: String,
    pub value: MonoExpr,
    pub span: Span,
}
