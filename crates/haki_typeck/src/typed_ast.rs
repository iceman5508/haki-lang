/// typed_ast.rs — Typed mirror of the Haki AST.
///
/// Every `TypedExpr` carries the inferred `Ty`. Statements and items
/// are structurally identical to the untyped AST but reference `TypedExpr`.
///
/// Design: we mirror only the parts that change (expressions); everything
/// else (identifiers, spans, type annotations, fn sigs) is borrowed directly
/// from `haki_ast` via re-export or cloned where needed.

use haki_ast::*;

// ── Resolved semantic type ────────────────────────────────────────────────────

/// A fully-resolved semantic type, produced by the type checker.
///
/// This is distinct from `haki_ast::Ty`, which is a syntactic type expression
/// (possibly containing unresolved names). `SemTy` has all names resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum SemTy {
    // ── Primitives ────────────────────────────────────────────────────────
    Int,
    Float,
    Bool,
    String,
    Void, // functions returning nothing

    // ── Optional wrapper ──────────────────────────────────────────────────
    /// `T?`
    Optional(Box<SemTy>),

    // ── Named user type ───────────────────────────────────────────────────
    /// A struct or class resolved by name.
    Named(std::string::String),

    // ── Generic instantiation ─────────────────────────────────────────────
    /// `Array<int>`, `Map<string, User>`
    Generic(std::string::String, Vec<SemTy>),

    // ── Tuple (multi-return) ──────────────────────────────────────────────
    Tuple(Vec<SemTy>),

    // ── Type variable (during inference) ─────────────────────────────────
    /// An unresolved type variable. Should not appear in the final typed AST.
    /// Used internally by the inference engine.
    Var(u32),

    // ── Never (panic / unreachable) ───────────────────────────────────────
    /// The type of `panic(...)` — satisfies any type requirement.
    Never,

    // ── First-class function type ─────────────────────────────────────────
    /// `fn(A, B) -> R` — a plain function pointer (no captured environment).
    Fn(Vec<SemTy>, Box<SemTy>),

    // ── Closure type ─────────────────────────────────────────────────────
    /// `fn[...](A, B) -> R` — a fat pointer `{ fn_ptr, env_ptr }`.
    /// Produced by `fn[captures](params) -> R { body }` literals with captures.
    /// At the IR level: heap-allocated `{ ptr fn, ptr env }`.
    Closure(Vec<SemTy>, Box<SemTy>),
}

impl SemTy {
    /// Is this type compatible with `null` assignment?
    pub fn is_optional(&self) -> bool {
        matches!(self, SemTy::Optional(_))
    }

    /// Is this a Chan<T> type?
    pub fn is_chan(&self) -> bool {
        matches!(self, SemTy::Generic(n, _) if n == "Chan")
    }

    /// Is this a TaskGroup<T> type?
    pub fn is_taskgroup(&self) -> bool {
        matches!(self, SemTy::Generic(n, _) if n == "TaskGroup")
    }

    /// Can a value of `other` be assigned to a slot expecting `self`?
    /// Handles: exact match, null→optional, Never (satisfies anything),
    /// and tuple element-wise coercion.
    pub fn is_assignable_from(&self, other: &SemTy) -> bool {
        if *other == SemTy::Never {
            return true; // `panic` satisfies any type
        }
        if self == other {
            return true;
        }
        // null (Optional(Void)) can go into any Optional
        let is_null = matches!(other, SemTy::Optional(inner) if **inner == SemTy::Void);
        if is_null && self.is_optional() {
            return true;
        }
        // T is assignable to T?
        if let SemTy::Optional(inner) = self {
            if inner.as_ref() == other {
                return true;
            }
            // null → T?
            if is_null {
                return true;
            }
        }
        // Tuple: element-wise assignability
        if let (SemTy::Tuple(expected), SemTy::Tuple(found)) = (self, other) {
            if expected.len() == found.len() {
                return expected.iter().zip(found.iter()).all(|(e, f)| e.is_assignable_from(f));
            }
        }
        // Bare generic name assignable to specialised generic:
        // `Map` assignable to `Map<string, int>`, `Array` to `Array<int>`.
        // This lets `Map()` / `Array()` constructor calls work when the
        // let-binding carries a full generic annotation.
        if let (SemTy::Generic(exp_name, _), SemTy::Named(found_name)) = (self, other) {
            if exp_name == found_name {
                return true;
            }
        }

        // Empty array literal `[]` infers as `Array<void>`. Allow it to be
        // assigned to any `Array<T>` — the annotation provides the real type.
        if let (SemTy::Generic(exp_name, _), SemTy::Generic(found_name, found_args)) = (self, other) {
            if exp_name == found_name && found_name == "Array" {
                if found_args.len() == 1 && found_args[0] == SemTy::Void {
                    return true; // empty array literal
                }
            }
        }

        // Closure is assignable to a plain Fn of the same signature.
        // `fn[x](int) -> bool` can be passed where `fn(int) -> bool` is expected.
        if let SemTy::Fn(ep, er) = self {
            if let SemTy::Closure(fp, fr) = other {
                return ep.len() == fp.len() && er == fr
                    && ep.iter().zip(fp.iter()).all(|(e, f)| e == f);
            }
        }
        false
    }

    /// Human-readable display (for error messages).
    pub fn display(&self) -> std::string::String {
        match self {
            SemTy::Int    => "int".into(),
            SemTy::Float  => "f64".into(),
            SemTy::Bool   => "bool".into(),
            SemTy::String => "string".into(),
            SemTy::Void   => "void".into(),
            SemTy::Never  => "never".into(),
            SemTy::Var(n) => format!("?{n}"),
            SemTy::Named(n) => n.clone(),
            SemTy::Optional(inner) => format!("{}?", inner.display()),
            SemTy::Generic(name, args) => {
                let args_str = args.iter()
                    .map(|a| a.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{args_str}>")
            }
            SemTy::Tuple(tys) => {
                let inner = tys.iter()
                    .map(|t| t.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({inner})")
            }
            SemTy::Fn(params, ret) => {
                let ps = params.iter().map(|p| p.display()).collect::<Vec<_>>().join(", ");
                if **ret == SemTy::Void { format!("fn({ps})") }
                else { format!("fn({ps}) -> {}", ret.display()) }
            }
            SemTy::Closure(params, ret) => {
                let ps = params.iter().map(|p| p.display()).collect::<Vec<_>>().join(", ");
                if **ret == SemTy::Void { format!("fn[...]({ps})") }
                else { format!("fn[...]({ps}) -> {}", ret.display()) }
            }
        }
    }
}

// ── Typed expressions ─────────────────────────────────────────────────────────

/// An expression annotated with its inferred semantic type.
#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: SemTy,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedExprKind {
    Int(i64),
    Float(f64),
    String(std::string::String),
    Bool(bool),
    Null,
    Ident(Ident),
    Unary(UnaryOp, Box<TypedExpr>),
    Binary(BinaryOp, Box<TypedExpr>, Box<TypedExpr>),
    Field(Box<TypedExpr>, Ident),
    OptionalField(Box<TypedExpr>, Ident),
    OptionalMethodCall(Box<TypedExpr>, Ident, Vec<TypedExpr>),
    MethodCall(Box<TypedExpr>, Ident, Vec<TypedExpr>),
    Call(Box<TypedExpr>, Vec<TypedExpr>),
    NamedCall(Box<TypedExpr>, Vec<TypedNamedArg>),
    Index(Box<TypedExpr>, Box<TypedExpr>),
    If(Box<TypedIfExpr>),
    Match(Box<TypedMatchExpr>),
    Block(Box<TypedBlock>),
    Array(Vec<TypedExpr>),
    Assign(Box<TypedExpr>, Box<TypedExpr>),
    /// `async expr` — produces `Task<T>`
    Async(Box<TypedExpr>),
    /// `fn(x: A) -> R { body }` or `fn[self, x](args) -> R { body }` — closure literal.
    /// `captures`: (name, type, is_weak) for each captured variable.
    FnLiteral(TypedFnDef, Vec<(Ident, SemTy, bool, bool)>),  // (id, ty, weak, mutable)
}

#[derive(Debug, Clone)]
pub struct TypedNamedArg {
    pub name: Ident,
    pub value: TypedExpr,
    pub span: Span,
}

// ── Typed statements ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedStmt {
    pub kind: TypedStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedStmtKind {
    Let(TypedLetStmt),
    Return(TypedReturnStmt),
    Select(TypedSelectStmt),
    Yield(Box<TypedExpr>),
    Defer(Box<TypedExpr>),
    Continue,
    Break,
    If(TypedIfExpr),
    For(TypedForStmt),
    While(TypedWhileStmt),
    Match(TypedMatchExpr),
    Panic(Box<TypedExpr>),
    Expr(Box<TypedExpr>),
}

/// Typed select statement — multiplex over channels.
#[derive(Debug, Clone)]
pub struct TypedSelectStmt {
    pub arms:    Vec<TypedSelectArm>,
    pub timeout: Option<(Box<TypedExpr>, TypedBlock)>,
    pub span:    Span,
}

#[derive(Debug, Clone)]
pub struct TypedSelectArm {
    pub binding:  haki_ast::Ident,
    pub binding_ty: SemTy,
    pub channel:  Box<TypedExpr>,
    pub body:     TypedBlock,
    pub span:     Span,
}

#[derive(Debug, Clone)]
pub struct TypedLetStmt {
    pub mutability: Mut,
    /// Bindings paired with their resolved types.
    pub bindings: Vec<(Binding, SemTy)>,
    pub init: Box<TypedExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedReturnStmt {
    pub values: Vec<TypedExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedForStmt {
    pub index_var: Option<Ident>,
    pub var: Ident,
    pub var_ty: SemTy,
    pub iter: Box<TypedExpr>,
    pub body: TypedBlock,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedWhileStmt {
    pub cond: Box<TypedExpr>,
    pub body: TypedBlock,
    pub span: Span,
}

// ── Typed if / match ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypedIfExpr {
    pub cond: Box<TypedExpr>,
    pub then_block: TypedBlock,
    pub else_branch: Option<TypedElseBranch>,
    pub ty: SemTy, // type produced by yield (or Void for statement use)
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedElseBranch {
    Block(TypedBlock),
    If(Box<TypedIfExpr>),
}

#[derive(Debug, Clone)]
pub struct TypedMatchExpr {
    pub scrutinee: Box<TypedExpr>,
    pub arms: Vec<TypedMatchArm>,
    pub ty: SemTy,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedMatchArm {
    pub pattern: MatchPattern,
    /// For class match: one binding (the object).
    /// For enum payload: one per payload field.
    /// For unit variant / wildcard: empty.
    pub bindings: Vec<Ident>,
    /// Types of the bindings, in order.
    pub binding_tys: Vec<SemTy>,
    /// Optional guard condition: `case x if x > 0 { ... }`
    pub guard: Option<TypedExpr>,
    pub body: TypedBlock,
    pub span: Span,
}

// ── Typed top-level items ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypedSourceFile {
    pub items: Vec<TypedItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedItem {
    pub kind: TypedItemKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedItemKind {
    /// Resolved import — carried through for the mono engine to ignore.
    Import { path: String, alias: String },
    Struct(TypedStructDef),
    Class(TypedClassDef),
    /// Enums carry no typed body — the AST EnumDef is sufficient.
    Enum(EnumDef),
    Protocol(ProtocolDef),
    Impl(TypedImplBlock),
    Fn(TypedFnDef),
    /// `extern "js" fn name(params) -> RetTy` — no body, emits Wasm import.
    ExternFn(ExternFnDef),
    /// `const NAME = value` at file scope.
    GlobalConst { name: haki_ast::Ident, ty: SemTy, value: TypedExpr },
}

#[derive(Debug, Clone)]
pub struct TypedStructDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<TypedFnDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedClassDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub superclass: Option<Ident>,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<TypedFnDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedImplBlock {
    pub protocol: Ident,
    pub target: Ident,
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<TypedFnDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypedFnDef {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_ty: SemTy,
    pub body: TypedBlock,
    /// Attributes from the source declaration (e.g. `@deprecated`, `@inline`).
    pub attributes: Vec<haki_ast::Attribute>,
    pub span: Span,
}
