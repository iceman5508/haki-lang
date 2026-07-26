/// lsp_index.rs — In-memory project index for the Haki LSP.
///
/// Maintains a per-file `FileState` with the parsed AST, typed AST, and
/// symbol table. Re-typechecks a single file on every change (26ms).
/// Module imports are resolved against cached module state for other files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use haki_ast::{SourceFile, Span};
use haki_parser::{ParseError, ParseResult};
use haki_typeck::{TypedSourceFile, ModuleSymbols};

// ── FileState ─────────────────────────────────────────────────────────────────

/// Everything the LSP knows about a single open file.
pub struct FileState {
    /// Current text content (what the editor sees).
    pub text: String,
    /// Parsed AST — always present, even when there are parse errors.
    pub ast: SourceFile,
    /// Parse errors from the most recent parse.
    pub parse_errors: Vec<ParseError>,
    /// Typed AST — `None` if typecheck failed.
    pub typed: Option<TypedSourceFile>,
    /// Typecheck error message — `Some` if typecheck failed.
    pub type_error: Option<String>,
    /// Module symbols exported by this file (used when other files import it).
    pub module_syms: Option<ModuleSymbols>,
    /// The symbol table used for this file's typecheck — kept for hover/definition.
    /// Contains all registered types, functions, and protocols including builtins.
    pub sym: Option<haki_typeck::SymbolTable>,
}

impl FileState {
    fn new_empty(text: String) -> Self {
        Self {
            text,
            ast: SourceFile { items: vec![], span: Span::new(0, 0) },
            parse_errors: vec![],
            typed: None,
            type_error: None,
            module_syms: None,
            sym: None,
        }
    }

    /// True if the file has no parse or type errors.
    pub fn is_clean(&self) -> bool {
        self.parse_errors.is_empty() && self.type_error.is_none()
    }
}

// ── ProjectIndex ──────────────────────────────────────────────────────────────

/// The LSP's in-memory view of the project.
///
/// `update_file` is called on every `textDocument/didChange` or
/// `textDocument/didOpen`. It re-parses and re-typechecks the changed file,
/// then updates the index.
pub struct ProjectIndex {
    files: HashMap<PathBuf, FileState>,
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self { files: HashMap::new() }
    }

    /// Parse and typecheck `text` as `path`, updating the index.
    /// Always succeeds — errors are stored in `FileState`.
    pub fn update_file(&mut self, path: &Path, text: String) {
        let mut state = FileState::new_empty(text.clone());

        // ── Parse (with error recovery) ──────────────────────────────────────
        let ParseResult { ast, errors } = haki_parser::parse_recovery(&text);
        state.ast = ast.clone();
        state.parse_errors = errors;

        // ── Typecheck (single-file, fast) ─────────────────────────────────
        if !state.ast.items.is_empty() || state.parse_errors.is_empty() {
            let mut sym = haki_typeck::SymbolTable::new();
            haki_stdlib::register_builtins(&mut sym);
            self.inject_imports(&ast, &mut sym);

            // Clone sym before consuming it — we keep it for hover/definition.
            // Also run collect on the snapshot so user-defined functions/types
            // are included (typecheck_with_sym calls collect internally on the
            // consumed copy, but our snapshot needs it too).
            let mut sym_snapshot = sym.clone();
            let _ = sym_snapshot.collect(&ast); // ignore errors — best effort

            match haki_typeck::typecheck_with_sym(&ast, sym) {
                Ok(typed) => {
                    if let Ok(syms) = haki_typeck::collect_module(&ast) {
                        state.module_syms = Some(syms);
                    }
                    state.sym   = Some(sym_snapshot);
                    state.typed = Some(typed);
                }
                Err(e) => {
                    // Keep sym even on typecheck failure — useful for partial hover
                    state.sym        = Some(sym_snapshot);
                    state.type_error = Some(e.to_string());
                }
            }
        }

        self.files.insert(path.to_path_buf(), state);
    }

    /// Get the current state for a file.
    pub fn get(&self, path: &Path) -> Option<&FileState> {
        self.files.get(path)
    }

    /// Inject cached module symbols for files imported by `ast` into `sym`.
    fn inject_imports(&self, ast: &SourceFile, sym: &mut haki_typeck::SymbolTable) {
        for item in &ast.items {
            if let haki_ast::ItemKind::Import { path: import_path, alias, .. } = &item.kind {
                // Resolve alias (same logic as the compiler driver)
                let alias_str = alias.clone().unwrap_or_else(|| {
                    std::path::Path::new(import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(import_path)
                        .to_string()
                });

                // Check if we have the imported file in the index
                let candidate = PathBuf::from(import_path).with_extension("haki");
                for (indexed_path, state) in &self.files {
                    if indexed_path.ends_with(&candidate) {
                        if let Some(ref mod_syms) = state.module_syms {
                            sym.modules.insert(alias_str.clone(), mod_syms.clone());
                        }
                        break;
                    }
                }
            }
        }
    }
}

// ── Span utilities for LSP position mapping ───────────────────────────────────

/// Convert a byte offset in `text` to a (line, col) pair (both 0-indexed).
pub fn offset_to_position(text: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let text_bytes = text.as_bytes();
    let clamped = offset.min(text_bytes.len());

    let mut line = 0u32;
    let mut line_start = 0usize;

    for i in 0..clamped {
        if text_bytes[i] == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }

    let col = (clamped - line_start) as u32;
    (line, col)
}

/// Convert a (line, col) pair (both 0-indexed) to a byte offset in `text`.
pub fn position_to_offset(text: &str, line: u32, col: u32) -> u32 {
    let mut current_line = 0u32;
    let mut offset = 0usize;

    for (i, ch) in text.char_indices() {
        if current_line == line {
            return (i + col as usize).min(text.len()) as u32;
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset = i + ch.len_utf8();
    }

    if current_line == line {
        return (offset + col as usize).min(text.len()) as u32;
    }

    text.len() as u32
}

// ── Scope resolver ────────────────────────────────────────────────────────────
//
// Walks the untyped AST with a scope stack to answer two questions:
//   1. What name is at byte offset `cursor`?
//   2. Where was that name defined (its definition Span)?
//
// Used by go-to-definition for local variables and function parameters.

use haki_ast::{ItemKind, StmtKind, ExprKind, Binding, Block, FnDef};

/// A single scope frame: maps name → definition span.
type ScopeFrame = std::collections::HashMap<String, Span>;

/// Result of scope resolution at a cursor position.
pub struct ScopeHit {
    /// The name under the cursor.
    pub name: String,
    /// Where that name was defined.
    pub def_span: Span,
}

/// Walk the AST and find the definition of the name at `cursor` offset.
/// Searches function bodies and method bodies. Returns `None` if the cursor
/// isn't on a locally-defined name, or if the name is a top-level definition
/// (handled separately by `find_definition_in_ast`).
pub fn resolve_local_definition(ast: &SourceFile, cursor: u32) -> Option<ScopeHit> {
    for item in &ast.items {
        match &item.kind {
            ItemKind::Fn(f) => {
                if let Some(hit) = resolve_in_fn(f, cursor) {
                    return Some(hit);
                }
            }
            ItemKind::Struct(s) => {
                for method in &s.methods {
                    if let Some(hit) = resolve_in_fn(method, cursor) {
                        return Some(hit);
                    }
                }
            }
            ItemKind::Class(c) => {
                for method in &c.methods {
                    if let Some(hit) = resolve_in_fn(method, cursor) {
                        return Some(hit);
                    }
                }
                // Also check impl blocks via the class body methods
            }
            ItemKind::Impl(i) => {
                for method in &i.methods {
                    if let Some(hit) = resolve_in_fn(method, cursor) {
                        return Some(hit);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve_in_fn(f: &FnDef, cursor: u32) -> Option<ScopeHit> {
    // Only search if cursor falls within this function's body span
    if cursor < f.body.span.lo || cursor > f.body.span.hi {
        return None;
    }

    // Seed the scope with function parameters
    let mut scopes: Vec<ScopeFrame> = vec![ScopeFrame::new()];
    for param in &f.params {
        scopes.last_mut().unwrap()
            .insert(param.name.name.clone(), param.name.span);
    }

    resolve_in_block(&f.body, cursor, &mut scopes)
}

fn resolve_in_block(block: &Block, cursor: u32, scopes: &mut Vec<ScopeFrame>) -> Option<ScopeHit> {
    scopes.push(ScopeFrame::new());

    for stmt in &block.stmts {
        // Process the statement — it may define new names or contain the cursor
        let hit = resolve_in_stmt(stmt, cursor, scopes);

        // Register any let/const bindings from this statement into the current scope
        // (so later statements can see them)
        if let StmtKind::Let(l) = &stmt.kind {
            for binding in &l.bindings {
                if let Binding::Name(ident) = binding {
                    if ident.name != "_" {
                        scopes.last_mut().unwrap()
                            .insert(ident.name.clone(), ident.span);
                    }
                }
            }
        }

        if hit.is_some() {
            scopes.pop();
            return hit;
        }
    }

    scopes.pop();
    None
}

fn resolve_in_stmt(
    stmt: &haki_ast::Stmt,
    cursor: u32,
    scopes: &mut Vec<ScopeFrame>,
) -> Option<ScopeHit> {
    match &stmt.kind {
        StmtKind::Let(l) => resolve_in_expr(&l.init, cursor, scopes),

        StmtKind::Return(r) => {
            for val in &r.values {
                if let Some(hit) = resolve_in_expr(val, cursor, scopes) {
                    return Some(hit);
                }
            }
            None
        }

        StmtKind::Yield(e) | StmtKind::Expr(e) => resolve_in_expr(e, cursor, scopes),

        StmtKind::If(i) => {
            resolve_in_expr(&i.cond, cursor, scopes)
                .or_else(|| resolve_in_block(&i.then_block, cursor, scopes))
                .or_else(|| match &i.else_branch {
                    Some(haki_ast::ElseBranch::Block(b)) => resolve_in_block(b, cursor, scopes),
                    Some(haki_ast::ElseBranch::If(inner)) => {
                        resolve_in_expr(&inner.cond, cursor, scopes)
                            .or_else(|| resolve_in_block(&inner.then_block, cursor, scopes))
                    }
                    None => None,
                })
        }

        StmtKind::While(w) => {
            resolve_in_expr(&w.cond, cursor, scopes)
                .or_else(|| resolve_in_block(&w.body, cursor, scopes))
        }

        StmtKind::For(f) => {
            // The loop variable is in scope within the body
            let mut inner: Vec<ScopeFrame> = scopes.to_vec();
            inner.push(ScopeFrame::new());
            inner.last_mut().unwrap().insert(f.var.name.clone(), f.var.span);
            if let Some(ref iv) = f.index_var {
                inner.last_mut().unwrap().insert(iv.name.clone(), iv.span);
            }
            resolve_in_expr(&f.iter, cursor, &mut inner)
                .or_else(|| resolve_in_block(&f.body, cursor, &mut inner))
        }

        StmtKind::Match(m) => {
            resolve_in_expr(&m.scrutinee, cursor, scopes)
                .or_else(|| {
                    for arm in &m.arms {
                        // Arm bindings are in scope within the arm body
                        let mut inner = scopes.to_vec();
                        inner.push(ScopeFrame::new());
                        for b in &arm.bindings {
                            inner.last_mut().unwrap().insert(b.name.clone(), b.span);
                        }
                        if let Some(hit) = resolve_in_block(&arm.body, cursor, &mut inner) {
                            return Some(hit);
                        }
                    }
                    None
                })
        }

        StmtKind::Defer(e) | StmtKind::Panic(e) => resolve_in_expr(e, cursor, scopes),

        StmtKind::Continue | StmtKind::Break => None,
    }
}

fn resolve_in_expr(
    expr: &haki_ast::Expr,
    cursor: u32,
    scopes: &mut Vec<ScopeFrame>,
) -> Option<ScopeHit> {
    // Only inspect expressions that contain the cursor
    if cursor < expr.span.lo || cursor > expr.span.hi {
        return None;
    }

    match &expr.kind {
        // ── The key case: identifier under cursor ─────────────────────────
        ExprKind::Ident(ident) if span_contains(expr.span, cursor) => {
            // Look up through scope chain (innermost first)
            for frame in scopes.iter().rev() {
                if let Some(&def_span) = frame.get(&ident.name) {
                    return Some(ScopeHit { name: ident.name.clone(), def_span });
                }
            }
            // Not a local — caller will check top-level symbols
            None
        }

        // ── Recurse into subexpressions ───────────────────────────────────
        ExprKind::Call(callee, args) => {
            resolve_in_expr(callee, cursor, scopes)
                .or_else(|| args.iter().find_map(|a| resolve_in_expr(a, cursor, scopes)))
        }

        ExprKind::NamedCall(callee, args) => {
            resolve_in_expr(callee, cursor, scopes)
                .or_else(|| args.iter().find_map(|a| resolve_in_expr(&a.value, cursor, scopes)))
        }

        ExprKind::MethodCall(recv, _, args) => {
            resolve_in_expr(recv, cursor, scopes)
                .or_else(|| args.iter().find_map(|a| resolve_in_expr(a, cursor, scopes)))
        }

        ExprKind::Field(recv, _) => resolve_in_expr(recv, cursor, scopes),

        ExprKind::Binary(_, l, r) => {
            resolve_in_expr(l, cursor, scopes)
                .or_else(|| resolve_in_expr(r, cursor, scopes))
        }

        ExprKind::Unary(_, e) => resolve_in_expr(e, cursor, scopes),

        ExprKind::Index(arr, idx) => {
            resolve_in_expr(arr, cursor, scopes)
                .or_else(|| resolve_in_expr(idx, cursor, scopes))
        }

        ExprKind::If(i) => {
            resolve_in_expr(&i.cond, cursor, scopes)
                .or_else(|| resolve_in_block(&i.then_block, cursor, scopes))
                .or_else(|| match &i.else_branch {
                    Some(haki_ast::ElseBranch::Block(b)) => resolve_in_block(b, cursor, scopes),
                    Some(haki_ast::ElseBranch::If(inner)) => {
                        resolve_in_expr(&inner.cond, cursor, scopes)
                            .or_else(|| resolve_in_block(&inner.then_block, cursor, scopes))
                    }
                    None => None,
                })
        }

        ExprKind::Block(b) => resolve_in_block(b, cursor, scopes),

        ExprKind::Match(m) => {
            resolve_in_expr(&m.scrutinee, cursor, scopes)
                .or_else(|| {
                    for arm in &m.arms {
                        let mut inner = scopes.to_vec();
                        inner.push(ScopeFrame::new());
                        for b in &arm.bindings {
                            inner.last_mut().unwrap().insert(b.name.clone(), b.span);
                        }
                        if let Some(hit) = resolve_in_block(&arm.body, cursor, &mut inner) {
                            return Some(hit);
                        }
                    }
                    None
                })
        }

        ExprKind::Assign(t, v) => {
            resolve_in_expr(t, cursor, scopes)
                .or_else(|| resolve_in_expr(v, cursor, scopes))
        }

        ExprKind::FnLiteral { body, params, .. } => {
            // Closure body: seed a new scope with the closure's own params
            let mut inner = scopes.to_vec();
            inner.push(ScopeFrame::new());
            for p in params {
                inner.last_mut().unwrap().insert(p.name.name.clone(), p.name.span);
            }
            resolve_in_block(body, cursor, &mut inner)
        }

        ExprKind::Array(elems) => {
            elems.iter().find_map(|e| resolve_in_expr(e, cursor, scopes))
        }

        ExprKind::Async(e) => resolve_in_expr(e, cursor, scopes),

        // Leaf nodes — cursor is on this token but it's not an ident we track
        ExprKind::Ident(_) | ExprKind::Int(_) | ExprKind::Float(_)
        | ExprKind::String(_) | ExprKind::Bool(_) | ExprKind::Null => None,
    }
}

/// True if `span` strictly contains `cursor` (cursor is within the token).
fn span_contains(span: Span, cursor: u32) -> bool {
    span.lo <= cursor && cursor <= span.hi
}
