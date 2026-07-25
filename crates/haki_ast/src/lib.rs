/// haki_ast — AST node definitions for the Haki programming language v0.1
///
/// Design notes:
///   - Every node carries a `Span` for precise error reporting.
///   - `Ty` is kept separate from `Expr` so the type-checker can operate on it
///     independently.
///   - The AST is an untyped/unresolved tree; type information is added by
///     `haki_typeck` in a later pass.

// ── Span ─────────────────────────────────────────────────────────────────────

/// Byte-offset span into the original source text.
/// `lo` is inclusive, `hi` is exclusive (like Rust's `..` range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    pub fn new(lo: u32, hi: u32) -> Self {
        Self { lo, hi }
    }

    /// A dummy span used for synthesised nodes (e.g. stdlib injections).
    pub fn dummy() -> Self {
        Self { lo: 0, hi: 0 }
    }

    /// Merge two spans into the smallest span that covers both.
    pub fn to(&self, other: Span) -> Span {
        Span {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }
}

// ── Identifiers ──────────────────────────────────────────────────────────────

/// A name with its source location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self { name: name.into(), span }
    }
}

// ── Type expressions ─────────────────────────────────────────────────────────

/// A syntactic type expression.
///
/// Examples:
///   `int`, `string`, `User`, `Array<T>`, `(string, Error?)`, `T?`
#[derive(Debug, Clone, PartialEq)]
pub struct Ty {
    pub kind: TyKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TyKind {
    /// A simple named type: `int`, `string`, `User`, `Self`, `T`
    Named(Ident),

    /// A generic application: `Array<T>`, `Map<string, int>`
    Generic(Ident, Vec<Ty>),

    /// An optional (nullable) type: `T?`
    Optional(Box<Ty>),

    /// A tuple type used for multi-return: `(string, Error?)`
    Tuple(Vec<Ty>),

    /// A first-class function type: `fn(A, B) -> R`
    /// Parameters are bare types (no names at the type level).
    /// Return type is None for void functions.
    Fn(Vec<Ty>, Option<Box<Ty>>),
}

// ── Generic type parameters ───────────────────────────────────────────────────

/// A single generic type parameter with optional protocol bounds.
///
/// Example: `T: Comparable & Printable`
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: Ident,
    /// Protocol names this type parameter must conform to.
    /// Empty = unconstrained.
    pub bounds: Vec<Ident>,
    pub span: Span,
}

// ── Function parameters ───────────────────────────────────────────────────────

/// A single function/method parameter.
///
/// Example: `path: string`, `item: T`
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: Ty,
    pub span: Span,
}

// ── Mutability ────────────────────────────────────────────────────────────────

/// Whether a binding is mutable (`let`) or immutable (`const`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mut {
    Const, // `const` — immutable
    Let,   // `let`   — mutable
}

// ── Field declarations (inside struct / class) ────────────────────────────────

/// A field declared inside a `struct` or `class` body.
///
/// Examples:
///   `const x: f64`
///   `let score: int`
///   `weak team: Team?`
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub mutability: Mut,
    pub is_weak: bool,
    pub name: Ident,
    pub ty: Ty,
    pub span: Span,
}

// ── Top-level items ───────────────────────────────────────────────────────────

/// A complete source file — the root of the AST.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub items: Vec<Item>,
    pub span: Span,
}

/// Any top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// `import "relative/path"` or `import "relative/path" as alias`
    Import {
        path:  String,
        alias: Option<String>,
        span:  Span,
    },
    /// `struct Foo<T> { ... }`
    Struct(StructDef),
    /// `class Foo extends Bar { ... }`
    Class(ClassDef),
    /// `enum Foo<T> { Ok(T), Err(Error), Pending }`
    Enum(EnumDef),
    /// `protocol Foo { fn method(arg: Type) -> RetTy }`
    Protocol(ProtocolDef),
    /// `impl Protocol for Type { fn method(...) { ... } }`
    Impl(ImplBlock),
    /// `fn name<T>(params) -> RetTy { ... }`
    Fn(FnDef),
}

// ── Enum ──────────────────────────────────────────────────────────────────────

/// `enum Color { Red, Green, Blue }` or `enum Result<T, E> { Ok(T), Err(E) }`
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// One variant of an enum. Payload is either positional or named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    /// Positional payload types, e.g. `Ok(T)` → `[T]`.
    /// Empty for unit variants, e.g. `Pending`.
    pub fields: Vec<Ty>,
    pub span: Span,
}

// ── Match arm (updated for enum variants) ────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDef>,
    /// Methods defined inside the struct body (syntactic sugar — equivalent to
    /// a separate `impl` block in the later passes).
    pub methods: Vec<FnDef>,
    pub span: Span,
}

// ── Class ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    /// Optional `extends BaseClass`.
    pub superclass: Option<Ident>,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<FnDef>,
    pub span: Span,
}

// ── Protocol ──────────────────────────────────────────────────────────────────

/// A protocol is signature-only in v0.1 — no default implementations.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDef {
    pub name: Ident,
    /// Method signatures without bodies (required to implement).
    pub methods: Vec<FnSig>,
    /// Method definitions with default bodies (optional to override).
    pub default_methods: Vec<FnDef>,
    pub span: Span,
}

/// A function signature without a body — used inside protocols.
#[derive(Debug, Clone, PartialEq)]
pub struct FnSig {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<ReturnTy>,
    pub span: Span,
}

// ── Impl block ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    /// The protocol being implemented, e.g. `Comparable`.
    pub protocol: Ident,
    /// The type implementing it, e.g. `Temperature`.
    pub target: Ident,
    /// Optional type parameters on the target, e.g. `impl Foo for Bar<T>`.
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<FnDef>,
    pub span: Span,
}

// ── Function definitions ──────────────────────────────────────────────────────

/// A function (or method) definition with a body.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<ReturnTy>,
    pub body: Block,
    pub span: Span,
}

/// The return type annotation on a function.
///
/// A function may return:
///   - nothing         (no `->`)
///   - a single type   `-> string`
///   - a tuple         `-> (string, Error?)`
#[derive(Debug, Clone, PartialEq)]
pub enum ReturnTy {
    Single(Ty),
    Tuple(Vec<Ty>),
}

// ── Statements ────────────────────────────────────────────────────────────────

/// A block is a sequence of statements inside `{ }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `const x: T = expr` or `let x = expr`
    /// Multi-binding: `const a, b = expr` (for multi-return destructuring)
    Let(LetStmt),

    /// `return expr` or `return expr, expr` (multi-return)
    Return(ReturnStmt),

    /// `yield expr` — produces a value from the enclosing block expression.
    Yield(Box<Expr>),

    /// `if cond { } else { }` used as a statement (no `yield` at top level).
    If(IfExpr),

    /// `for x in expr { }` loop.
    For(ForStmt),

    /// `while expr { }` loop.
    While(WhileStmt),

    /// `match expr { Pattern { } ... }`
    Match(MatchExpr),

    /// `panic("message")`
    Panic(Box<Expr>),

    /// `defer expr` — defers a call until the enclosing scope exits.
    /// The expression is evaluated at the defer site but executed on scope exit.
    Defer(Box<Expr>),
    /// `continue` — skip to next loop iteration.
    Continue,
    /// `break` — exit the current loop.
    Break,

    /// A bare expression used as a statement (e.g. a function call whose
    /// return value is intentionally discarded via `_ = expr`).
    Expr(Box<Expr>),
}

/// Variable binding statement.
///
/// Examples:
///   `const x: int = 42`
///   `let name = "Alice"`
///   `const data, err = readFile("config.json")`
///   `const data, _ = readFile("cache.json")`
///   `_ = logEvent("ping")`
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub mutability: Mut,
    /// The list of binding targets (may be `_` for discard).
    /// Multi-binding supports multi-return destructuring.
    pub bindings: Vec<Binding>,
    /// Optional explicit type annotation (only valid for single binding).
    pub ty: Option<Ty>,
    pub init: Box<Expr>,
    pub span: Span,
}

/// A single name (or wildcard) in a binding position.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// A named binding: `x`, `err`, `data`
    Name(Ident),
    /// An explicit discard: `_`
    Discard(Span),
}

/// Return statement.
///
/// Examples:
///   `return`
///   `return 42`
///   `return null, err`
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    /// Empty = bare `return` (void function).
    pub values: Vec<Expr>,
    pub span: Span,
}

/// `for x in collection { body }` or `for i, x in collection { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    /// Optional index variable (`i` in `for i, x in arr { }`).
    pub index_var: Option<Ident>,
    pub var: Ident,
    pub iter: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

/// `while cond { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub cond: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // ── Literals ──────────────────────────────────────────────
    /// Integer literal: `42`
    Int(i64),
    /// Floating-point literal: `3.14`
    Float(f64),
    /// String literal: `"hello"`
    String(String),
    /// Boolean literal: `true` / `false`
    Bool(bool),
    /// The `null` keyword.
    Null,

    // ── Binding reference ──────────────────────────────────────
    /// A variable or name reference: `x`, `score`, `Self`
    Ident(Ident),

    // ── Unary operations ───────────────────────────────────────
    Unary(UnaryOp, Box<Expr>),

    // ── Binary operations ──────────────────────────────────────
    Binary(BinaryOp, Box<Expr>, Box<Expr>),

    // ── Field access ───────────────────────────────────────────
    /// `expr.field`
    Field(Box<Expr>, Ident),

    // ── Method call ────────────────────────────────────────────
    /// `expr.method(args)`
    MethodCall(Box<Expr>, Ident, Vec<Expr>),

    // ── Function / constructor call ────────────────────────────
    /// `name(args)` — also covers struct/class construction
    Call(Box<Expr>, Vec<Expr>),

    // ── Index ──────────────────────────────────────────────────
    /// `expr[index]`
    Index(Box<Expr>, Box<Expr>),

    // ── If expression ──────────────────────────────────────────
    /// `if cond { yield ... } else { yield ... }`
    /// Used both as a statement and as a block expression.
    If(Box<IfExpr>),

    // ── Match expression ───────────────────────────────────────
    Match(Box<MatchExpr>),

    // ── Block expression ───────────────────────────────────────
    /// A `{ ... }` block whose value is produced by an inner `yield`.
    Block(Box<Block>),

    // ── Named-argument call (struct/class construction) ────────
    /// `User(name: "Alice", score: 0)` — labelled arguments
    NamedCall(Box<Expr>, Vec<NamedArg>),

    // ── Array literal ──────────────────────────────────────────
    /// `[1, 2, 3]`
    Array(Vec<Expr>),

    // ── Assignment ─────────────────────────────────────────────
    /// `x = expr` (only valid for `let` bindings; typeck enforces)
    Assign(Box<Expr>, Box<Expr>),

    // ── Async call ─────────────────────────────────────────────
    /// `async expr` — call-site async modifier.
    /// Wraps any call expression and produces a `Task<T>` where
    /// `T` is the return type of the underlying function.
    /// The call is dispatched to the language-managed thread pool.
    Async(Box<Expr>),

    // ── Function literal ────────────────────────────────────────
    /// `fn(x: A, y: B) -> R { body }` — anonymous function value.
    /// Anonymous function literal, optionally with an explicit capture list.
    ///
    /// No captures:  `fn(x: int) -> int { return x * 2 }`
    /// With captures: `fn[self, count](x: int) -> int { return x + count }`
    /// Weak capture:  `fn[weak self](req) -> Response { ... }`
    FnLiteral {
        /// Explicit captures: `[weak self, count, ...]`
        /// Empty = no captures (plain function pointer, existing v0.4.5 behaviour).
        captures:  Vec<Capture>,
        params:    Vec<Param>,
        return_ty: Option<ReturnTy>,
        body:      Box<Block>,
    },
}

/// One item in a closure capture list: `weak self` or `count`.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub name: Ident,
    pub weak: bool,
    pub span: Span,
}

/// A named argument in a call: `name: expr`
#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

// ── If expression ─────────────────────────────────────────────────────────────

/// `if cond { then_block } else { else_block }`
/// `else` branch is optional when used as a statement.
/// Both branches must `yield` when used as an expression.
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

/// The `else` part of an `if` — either a plain block or another `if`.
#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    /// `else { ... }`
    Block(Block),
    /// `else if cond { ... }`
    If(Box<IfExpr>),
}

// ── Match expression ──────────────────────────────────────────────────────────

/// `match expr { Pattern name { body } ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// A single arm inside a `match`.
///
/// Class hierarchy match:  `NetworkError e { yield handleNetwork(e) }`
/// Enum unit variant:      `Pending { yield "waiting" }`
/// Enum payload variant:   `Ok(value) { yield value }`
/// Wildcard:               `_ { yield "other" }`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// The variant or type name: `Ok`, `NetworkError`, `_`
    pub pattern: Ident,
    /// Bound variable names — one per payload field.
    /// For class match: `[e]` (the bound object).
    /// For enum unit: `[]`.
    /// For enum payload: `[value]` or `[x, y]` etc.
    pub bindings: Vec<Ident>,
    pub body: Block,
    pub span: Span,
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-expr`
    Neg,
    /// `!expr`
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
}
