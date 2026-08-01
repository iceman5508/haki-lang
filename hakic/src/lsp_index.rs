/// lsp_index.rs — In-memory project index for the Haki LSP.
///
/// Maintains a per-file `FileState` with the parsed AST, typed AST, and
/// symbol table. Re-typechecks a single file on every change (~26ms).
/// Module imports are resolved against cached module state for other files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use haki_ast::{SourceFile, Span};
use haki_parser::{ParseError, ParseResult};
use haki_typeck::{TypedSourceFile, ModuleSymbols};

// ── FileState ─────────────────────────────────────────────────────────────────

/// Everything the LSP knows about a single open file.
pub struct FileState {
    pub text:         String,
    pub ast:          SourceFile,
    pub parse_errors: Vec<ParseError>,
    pub typed:        Option<TypedSourceFile>,
    pub type_error:   Option<String>,
    pub module_syms:  Option<ModuleSymbols>,
    pub sym:          Option<haki_typeck::SymbolTable>,
    /// Resolved imports: alias → absolute path
    pub import_paths: HashMap<String, PathBuf>,
}

impl FileState {
    fn new_empty(text: String) -> Self {
        Self {
            text,
            ast:          SourceFile { items: vec![], span: Span::new(0, 0) },
            parse_errors: vec![],
            typed:        None,
            type_error:   None,
            module_syms:  None,
            sym:          None,
            import_paths: HashMap::new(),
        }
    }
}

// ── ProjectIndex ──────────────────────────────────────────────────────────────

pub struct ProjectIndex {
    files: HashMap<PathBuf, FileState>,
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self { files: HashMap::new() }
    }

    pub fn update_file(&mut self, path: &Path, text: String) {
        let mut state = FileState::new_empty(text.clone());

        // ── Parse (with error recovery) ──────────────────────────────────────
        let ParseResult { ast, errors } = haki_parser::parse_recovery(&text);
        state.ast          = ast.clone();
        state.parse_errors = errors;

        // ── Resolve import paths ──────────────────────────────────────────────
        let source_dir = path.parent().unwrap_or(Path::new("."));
        for item in &ast.items {
            if let haki_ast::ItemKind::Import { path: import_path, alias, .. } = &item.kind {
                let alias_str = alias.clone().unwrap_or_else(|| {
                    Path::new(import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(import_path)
                        .to_string()
                });
                // Resolve: try relative to source file first, then stdlib
                let resolved = resolve_import_path(import_path, source_dir);
                if let Some(p) = resolved {
                    state.import_paths.insert(alias_str, p);
                }
            }
        }

        // ── Typecheck ─────────────────────────────────────────────────────────
        if !state.ast.items.is_empty() || state.parse_errors.is_empty() {
            let mut sym = haki_typeck::SymbolTable::new();
            haki_stdlib::register_builtins(&mut sym);
            self.inject_imports(&ast, &mut sym, source_dir);

            let mut sym_snapshot = sym.clone();
            let _ = sym_snapshot.collect(&ast);

            match haki_typeck::typecheck_with_sym(&ast, sym) {
                Ok(typed) => {
                    if let Ok(syms) = haki_typeck::collect_module(&ast) {
                        state.module_syms = Some(syms);
                    }
                    state.sym   = Some(sym_snapshot);
                    state.typed = Some(typed);
                }
                Err(e) => {
                    state.sym        = Some(sym_snapshot);
                    state.type_error = Some(e.to_string());
                }
            }
        }

        self.files.insert(path.to_path_buf(), state);
    }

    pub fn get(&self, path: &Path) -> Option<&FileState> {
        self.files.get(path)
    }

    /// Resolve a cross-file definition: given a module alias used in `from_path`,
    /// return the (target_path, span_in_target) for `symbol_name`.
    pub fn resolve_cross_file_definition(
        &self,
        from_path: &Path,
        alias: &str,
        symbol_name: &str,
    ) -> Option<(PathBuf, Span)> {
        let state = self.files.get(from_path)?;
        let target_path = state.import_paths.get(alias)?;

        // Try to get AST from index first; otherwise read from disk
        if let Some(target_state) = self.files.get(target_path) {
            if let Some(span) = find_definition_in_ast(&target_state.ast, symbol_name) {
                return Some((target_path.clone(), span));
            }
        } else {
            // File not open in editor — read from disk
            if let Ok(src) = fs::read_to_string(target_path) {
                let ParseResult { ast, .. } = haki_parser::parse_recovery(&src);
                if let Some(span) = find_definition_in_ast(&ast, symbol_name) {
                    return Some((target_path.clone(), span));
                }
            }
        }
        None
    }

    /// Get completions for `alias.` — returns all exported names from that module.
    pub fn module_completions(&self, from_path: &Path, alias: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let state = match self.files.get(from_path) {
            Some(s) => s,
            None    => return items,
        };

        // Get sym table for this file which has module symbols injected
        let sym = match &state.sym {
            Some(s) => s,
            None    => return items,
        };

        if let Some(mod_syms) = sym.modules.get(alias) {
            for (name, fi) in &mod_syms.functions {
                items.push(CompletionItem {
                    label:      name.clone(),
                    kind:       CompletionKind::Function,
                    detail:     Some(format_fn_signature_brief(name, fi)),
                    insert_text: name.clone(),
                });
            }
            for name in mod_syms.types.keys() {
                items.push(CompletionItem {
                    label:       name.clone(),
                    kind:        CompletionKind::Struct,
                    detail:      None,
                    insert_text: name.clone(),
                });
            }
        }
        items
    }

    /// Get completions for the current scope at `offset` in `path`.
    pub fn scope_completions(&self, path: &Path, offset: u32) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let state = match self.files.get(path) {
            Some(s) => s,
            None    => return items,
        };

        // Builtins + top-level symbols from sym table
        if let Some(sym) = &state.sym {
            for (name, fi) in &sym.functions {
                items.push(CompletionItem {
                    label:       name.clone(),
                    kind:        CompletionKind::Function,
                    detail:      Some(format_fn_signature_brief(name, fi)),
                    insert_text: name.clone(),
                });
            }
            for name in sym.types.keys() {
                items.push(CompletionItem {
                    label:       name.clone(),
                    kind:        CompletionKind::Struct,
                    detail:      None,
                    insert_text: name.clone(),
                });
            }
            // Module aliases
            for alias in sym.modules.keys() {
                items.push(CompletionItem {
                    label:       alias.clone(),
                    kind:        CompletionKind::Module,
                    detail:      Some(format!("module {alias}")),
                    insert_text: alias.clone(),
                });
            }
        }

        // Local variables from scope at cursor
        collect_locals_at_offset(&state.ast, offset, &mut items);

        items
    }

    /// Get all symbols in a document for the outline panel.
    pub fn document_symbols(&self, path: &Path) -> Vec<DocumentSymbol> {
        let mut syms = Vec::new();
        let state = match self.files.get(path) {
            Some(s) => s,
            None    => return syms,
        };

        for item in &state.ast.items {
            match &item.kind {
                haki_ast::ItemKind::Fn(f) => {
                    let (sl, sc) = offset_to_position(&state.text, f.name.span.lo);
                    let (el, ec) = offset_to_position(&state.text, f.body.span.hi);
                    syms.push(DocumentSymbol {
                        name:  f.name.name.clone(),
                        kind:  SymbolKind::Function,
                        start: (sl, sc),
                        end:   (el, ec),
                    });
                }
                haki_ast::ItemKind::Struct(s) => {
                    let (sl, sc) = offset_to_position(&state.text, s.name.span.lo);
                    let (el, ec) = offset_to_position(&state.text, item.span.hi);
                    syms.push(DocumentSymbol {
                        name:  s.name.name.clone(),
                        kind:  SymbolKind::Struct,
                        start: (sl, sc),
                        end:   (el, ec),
                    });
                }
                haki_ast::ItemKind::Class(c) => {
                    let (sl, sc) = offset_to_position(&state.text, c.name.span.lo);
                    let (el, ec) = offset_to_position(&state.text, item.span.hi);
                    syms.push(DocumentSymbol {
                        name:  c.name.name.clone(),
                        kind:  SymbolKind::Class,
                        start: (sl, sc),
                        end:   (el, ec),
                    });
                }
                haki_ast::ItemKind::Enum(e) => {
                    let (sl, sc) = offset_to_position(&state.text, e.name.span.lo);
                    let (el, ec) = offset_to_position(&state.text, item.span.hi);
                    syms.push(DocumentSymbol {
                        name:  e.name.name.clone(),
                        kind:  SymbolKind::Enum,
                        start: (sl, sc),
                        end:   (el, ec),
                    });
                }
                _ => {}
            }
        }
        syms
    }

    /// Find all references to `name` in `path`.
    pub fn find_references(&self, path: &Path, name: &str) -> Vec<(u32, u32, u32, u32)> {
        let mut refs = Vec::new();
        let state = match self.files.get(path) {
            Some(s) => s,
            None    => return refs,
        };
        find_ident_occurrences(&state.ast, &state.text, name, &mut refs);
        refs
    }

    /// Get signature help for a function call at `offset`.
    pub fn signature_help(&self, path: &Path, offset: u32) -> Option<SignatureHelp> {
        let state = self.files.get(path)?;
        let sym   = state.sym.as_ref()?;

        // Walk back from cursor to find the opening '(' and function name
        let text  = &state.text;
        let bytes = text.as_bytes();
        let mut pos   = (offset as usize).min(bytes.len());
        let mut depth = 0i32;
        let mut arg_index = 0u32;

        // Count commas at depth 0 to find active param
        while pos > 0 {
            pos -= 1;
            match bytes[pos] {
                b')' | b']' => depth += 1,
                b'(' | b'[' => {
                    if depth == 0 { break; }
                    depth -= 1;
                }
                b',' if depth == 0 => arg_index += 1,
                _ => {}
            }
        }

        // pos is now at '(' — extract function name before it
        let fn_name = find_ident_at_offset(text, pos.saturating_sub(1) as u32)?;

        // Look up in sym table
        let fi = sym.functions.get(&fn_name)?;
        let sig = format_fn_signature(&fn_name, fi);
        let params: Vec<String> = fi.params.iter()
            .map(|p| format!("{}: {}", p.name.name, format_ty_kind(&p.ty.kind)))
            .collect();

        Some(SignatureHelp {
            label:        sig,
            params,
            active_param: arg_index,
        })
    }

    fn inject_imports(
        &self,
        ast:        &SourceFile,
        sym:        &mut haki_typeck::SymbolTable,
        source_dir: &Path,
    ) {
        for item in &ast.items {
            if let haki_ast::ItemKind::Import { path: import_path, alias, .. } = &item.kind {
                let alias_str = alias.clone().unwrap_or_else(|| {
                    Path::new(import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(import_path)
                        .to_string()
                });

                // Try indexed file first
                let candidate = PathBuf::from(import_path).with_extension("haki");
                let mut found = false;
                for (indexed_path, state) in &self.files {
                    if indexed_path.ends_with(&candidate) {
                        if let Some(ref mod_syms) = state.module_syms {
                            sym.modules.insert(alias_str.clone(), mod_syms.clone());
                            found = true;
                            break;
                        }
                    }
                }

                // Fall back: resolve path from disk and parse
                if !found {
                    if let Some(resolved) = resolve_import_path(import_path, source_dir) {
                        if let Ok(src) = fs::read_to_string(&resolved) {
                            let ParseResult { ast: mod_ast, .. } = haki_parser::parse_recovery(&src);
                            if let Ok(mod_syms) = haki_typeck::collect_module(&mod_ast) {
                                sym.modules.insert(alias_str, mod_syms);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Import path resolution ────────────────────────────────────────────────────

fn resolve_import_path(import_path: &str, source_dir: &Path) -> Option<PathBuf> {
    // Stdlib (embedded in binary — no file to resolve)
    if import_path.starts_with("std/") { return None; }

    let clean = import_path.trim_start_matches("./");
    let with_ext = if clean.ends_with(".haki") {
        clean.to_string()
    } else {
        format!("{clean}.haki")
    };

    // Relative to source dir
    let rel = source_dir.join(&with_ext);
    if rel.exists() {
        return rel.canonicalize().ok();
    }

    // Absolute path
    let abs = PathBuf::from(&with_ext);
    if abs.exists() {
        return abs.canonicalize().ok();
    }

    None
}

// ── Completion types ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum CompletionKind { Function, Struct, Class, Module, Variable, Field, EnumVariant }

#[derive(Debug)]
pub struct CompletionItem {
    pub label:       String,
    pub kind:        CompletionKind,
    pub detail:      Option<String>,
    pub insert_text: String,
}

// ── Document symbol types ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SymbolKind { Function, Struct, Class, Enum }

#[derive(Debug)]
pub struct DocumentSymbol {
    pub name:  String,
    pub kind:  SymbolKind,
    pub start: (u32, u32),
    pub end:   (u32, u32),
}

// ── Signature help type ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SignatureHelp {
    pub label:        String,
    pub params:       Vec<String>,
    pub active_param: u32,
}

// ── Local variable collector for completions ──────────────────────────────────

fn collect_locals_at_offset(ast: &SourceFile, offset: u32, out: &mut Vec<CompletionItem>) {
    use haki_ast::ItemKind;
    for item in &ast.items {
        match &item.kind {
            ItemKind::Fn(f) => collect_locals_in_fn(f, offset, out),
            ItemKind::Struct(s) => {
                for m in &s.methods { collect_locals_in_fn(m, offset, out); }
            }
            ItemKind::Class(c) => {
                for m in &c.methods { collect_locals_in_fn(m, offset, out); }
            }
            ItemKind::Impl(i) => {
                for m in &i.methods { collect_locals_in_fn(m, offset, out); }
            }
            _ => {}
        }
    }
}

fn collect_locals_in_fn(f: &haki_ast::FnDef, offset: u32, out: &mut Vec<CompletionItem>) {
    if offset < f.body.span.lo || offset > f.body.span.hi { return; }
    for p in &f.params {
        out.push(CompletionItem {
            label:       p.name.name.clone(),
            kind:        CompletionKind::Variable,
            detail:      Some(format_ty_kind(&p.ty.kind)),
            insert_text: p.name.name.clone(),
        });
    }
    collect_locals_in_block(&f.body, offset, out);
}

fn collect_locals_in_block(block: &haki_ast::Block, offset: u32, out: &mut Vec<CompletionItem>) {
    for stmt in &block.stmts {
        if stmt.span.lo > offset { break; }
        if let haki_ast::StmtKind::Let(l) = &stmt.kind {
            for b in &l.bindings {
                if let haki_ast::Binding::Name(id) = b {
                    if id.name != "_" {
                        out.push(CompletionItem {
                            label:       id.name.clone(),
                            kind:        CompletionKind::Variable,
                            detail:      None,
                            insert_text: id.name.clone(),
                        });
                    }
                }
            }
        }
    }
}

// ── Find all occurrences of an identifier in AST ──────────────────────────────

fn find_ident_occurrences(
    ast:  &SourceFile,
    text: &str,
    name: &str,
    out:  &mut Vec<(u32, u32, u32, u32)>,
) {
    use haki_ast::ItemKind;
    for item in &ast.items {
        match &item.kind {
            ItemKind::Fn(f) => {
                find_ident_in_block(&f.body, name, text, out);
            }
            ItemKind::Struct(s) => {
                for m in &s.methods { find_ident_in_block(&m.body, name, text, out); }
            }
            ItemKind::Class(c) => {
                for m in &c.methods { find_ident_in_block(&m.body, name, text, out); }
            }
            _ => {}
        }
    }
}

fn find_ident_in_block(
    block: &haki_ast::Block,
    name:  &str,
    text:  &str,
    out:   &mut Vec<(u32, u32, u32, u32)>,
) {
    for stmt in &block.stmts {
        find_ident_in_stmt(stmt, name, text, out);
    }
}

fn find_ident_in_stmt(
    stmt: &haki_ast::Stmt,
    name: &str,
    text: &str,
    out:  &mut Vec<(u32, u32, u32, u32)>,
) {
    use haki_ast::StmtKind;
    match &stmt.kind {
        StmtKind::Let(l)         => find_ident_in_expr(&l.init, name, text, out),
        StmtKind::Return(r)      => { for v in &r.values { find_ident_in_expr(v, name, text, out); } }
        StmtKind::Yield(e) | StmtKind::Expr(e) | StmtKind::Defer(e) | StmtKind::Panic(e) => {
            find_ident_in_expr(e, name, text, out);
        }
        StmtKind::If(i) => {
            find_ident_in_expr(&i.cond, name, text, out);
            find_ident_in_block(&i.then_block, name, text, out);
            if let Some(haki_ast::ElseBranch::Block(b)) = &i.else_branch {
                find_ident_in_block(b, name, text, out);
            }
        }
        StmtKind::While(w) => {
            find_ident_in_expr(&w.cond, name, text, out);
            find_ident_in_block(&w.body, name, text, out);
        }
        StmtKind::For(f) => {
            find_ident_in_expr(&f.iter, name, text, out);
            find_ident_in_block(&f.body, name, text, out);
        }
        StmtKind::Match(m) => {
            find_ident_in_expr(&m.scrutinee, name, text, out);
            for arm in &m.arms {
                find_ident_in_block(&arm.body, name, text, out);
            }
        }
        StmtKind::Continue | StmtKind::Break => {}
        StmtKind::Select(_) => {}
    }
}

fn find_ident_in_expr(
    expr: &haki_ast::Expr,
    name: &str,
    text: &str,
    out:  &mut Vec<(u32, u32, u32, u32)>,
) {
    use haki_ast::ExprKind;
    match &expr.kind {
        ExprKind::Ident(id) if id.name == name => {
            let (sl, sc) = offset_to_position(text, expr.span.lo);
            let (el, ec) = offset_to_position(text, expr.span.hi);
            out.push((sl, sc, el, ec));
        }
        ExprKind::Call(callee, args) => {
            find_ident_in_expr(callee, name, text, out);
            for a in args { find_ident_in_expr(a, name, text, out); }
        }
        ExprKind::NamedCall(callee, args) => {
            find_ident_in_expr(callee, name, text, out);
            for a in args { find_ident_in_expr(&a.value, name, text, out); }
        }
        ExprKind::MethodCall(recv, _, args) => {
            find_ident_in_expr(recv, name, text, out);
            for a in args { find_ident_in_expr(a, name, text, out); }
        }
        ExprKind::Field(recv, _)     => find_ident_in_expr(recv, name, text, out),
        ExprKind::Binary(_, l, r)    => { find_ident_in_expr(l, name, text, out); find_ident_in_expr(r, name, text, out); }
        ExprKind::Unary(_, e)        => find_ident_in_expr(e, name, text, out),
        ExprKind::Assign(t, v)       => { find_ident_in_expr(t, name, text, out); find_ident_in_expr(v, name, text, out); }
        ExprKind::Index(a, i)        => { find_ident_in_expr(a, name, text, out); find_ident_in_expr(i, name, text, out); }
        ExprKind::Array(elems)       => { for e in elems { find_ident_in_expr(e, name, text, out); } }
        ExprKind::Async(e)           => find_ident_in_expr(e, name, text, out),
        ExprKind::Block(b)           => find_ident_in_block(b, name, text, out),
        ExprKind::If(i) => {
            find_ident_in_expr(&i.cond, name, text, out);
            find_ident_in_block(&i.then_block, name, text, out);
        }
        ExprKind::Match(m) => {
            find_ident_in_expr(&m.scrutinee, name, text, out);
            for arm in &m.arms { find_ident_in_block(&arm.body, name, text, out); }
        }
        ExprKind::FnLiteral { body, .. } => find_ident_in_block(body, name, text, out),
        _ => {}
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Convert a byte offset in `text` to a (line, col) pair (0-indexed).
pub fn offset_to_position(text: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let bytes  = text.as_bytes();
    let clamped = offset.min(bytes.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for i in 0..clamped {
        if bytes[i] == b'\n' { line += 1; line_start = i + 1; }
    }
    let col = (clamped - line_start) as u32;
    (line, col)
}

/// Convert a (line, col) pair (0-indexed) to a byte offset in `text`.
pub fn position_to_offset(text: &str, line: u32, col: u32) -> u32 {
    let mut current_line = 0u32;
    let mut offset = 0usize;
    for (i, ch) in text.char_indices() {
        if current_line == line { return (i + col as usize).min(text.len()) as u32; }
        if ch == '\n' { current_line += 1; }
        offset = i + ch.len_utf8();
    }
    if current_line == line { return (offset + col as usize).min(text.len()) as u32; }
    text.len() as u32
}

/// Walk the AST and find the definition span of the name at `cursor`.
pub fn resolve_local_definition(ast: &SourceFile, cursor: u32) -> Option<ScopeHit> {
    for item in &ast.items {
        match &item.kind {
            haki_ast::ItemKind::Fn(f) => {
                if let Some(hit) = resolve_in_fn(f, cursor) { return Some(hit); }
            }
            haki_ast::ItemKind::Struct(s) => {
                for m in &s.methods {
                    if let Some(hit) = resolve_in_fn(m, cursor) { return Some(hit); }
                }
            }
            haki_ast::ItemKind::Class(c) => {
                for m in &c.methods {
                    if let Some(hit) = resolve_in_fn(m, cursor) { return Some(hit); }
                }
            }
            haki_ast::ItemKind::Impl(i) => {
                for m in &i.methods {
                    if let Some(hit) = resolve_in_fn(m, cursor) { return Some(hit); }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_definition_in_ast(ast: &SourceFile, name: &str) -> Option<Span> {
    for item in &ast.items {
        match &item.kind {
            haki_ast::ItemKind::Fn(f)       if f.name.name == name => return Some(f.name.span),
            haki_ast::ItemKind::Struct(s)   if s.name.name == name => return Some(s.name.span),
            haki_ast::ItemKind::Class(c)    if c.name.name == name => return Some(c.name.span),
            haki_ast::ItemKind::Enum(e)     if e.name.name == name => return Some(e.name.span),
            haki_ast::ItemKind::Protocol(p) if p.name.name == name => return Some(p.name.span),
            _ => {}
        }
    }
    None
}

pub fn find_ident_at_offset(text: &str, offset: u32) -> Option<String> {
    let bytes  = text.as_bytes();
    let offset = offset as usize;
    if offset >= bytes.len() { return None; }
    if !is_ident_char(bytes[offset]) { return None; }
    let start = (0..=offset).rev().take_while(|&i| is_ident_char(bytes[i])).last().unwrap_or(offset);
    let end   = (offset..bytes.len()).take_while(|&i| is_ident_char(bytes[i])).last().unwrap_or(offset);
    let s = &text[start..=end];
    if s.is_empty() || matches!(s, "fn"|"let"|"const"|"if"|"else"|"while"|"for"|"return"|"match"|"struct"|"class"|"enum"|"import"|"as"|"true"|"false"|"null"|"async"|"defer"|"break"|"continue"|"yield"|"weak"|"extern"|"protocol"|"impl") {
        return None;
    }
    Some(s.to_string())
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// Re-export formatting helpers needed by lsp.rs
pub use format_fn_signature_brief as fmt_fn_brief;
pub fn format_fn_signature_brief(name: &str, fi: &haki_typeck::collector::FnInfo) -> String {
    use haki_ast::TyKind;
    let params: Vec<String> = fi.params.iter()
        .map(|p| format!("{}: {}", p.name.name, format_ty_kind(&p.ty.kind)))
        .collect();
    let ret = match &fi.return_ty {
        Some(haki_ast::ReturnTy::Single(t)) => format!(" -> {}", format_ty_kind(&t.kind)),
        Some(haki_ast::ReturnTy::Tuple(ts)) => {
            let inner: Vec<_> = ts.iter().map(|t| format_ty_kind(&t.kind)).collect();
            format!(" -> ({})", inner.join(", "))
        }
        None => String::new(),
    };
    format!("fn {}({}){}", name, params.join(", "), ret)
}

pub fn format_fn_signature(name: &str, fi: &haki_typeck::collector::FnInfo) -> String {
    format_fn_signature_brief(name, fi)
}

pub fn format_ty_kind(kind: &haki_ast::TyKind) -> String {
    match kind {
        haki_ast::TyKind::Named(id)       => id.name.clone(),
        haki_ast::TyKind::Optional(inner) => format!("{}?", format_ty_kind(&inner.kind)),
        haki_ast::TyKind::Generic(id, args) => {
            let inner: Vec<_> = args.iter().map(|a| format_ty_kind(&a.kind)).collect();
            format!("{}<{}>", id.name, inner.join(", "))
        }
        haki_ast::TyKind::Fn(params, ret) => {
            let ps: Vec<_> = params.iter().map(|p| format_ty_kind(&p.kind)).collect();
            let r = ret.as_ref().map_or("void".into(), |r| format_ty_kind(&r.kind));
            format!("fn({}) -> {}", ps.join(", "), r)
        }
        haki_ast::TyKind::Tuple(ts) => {
            let inner: Vec<_> = ts.iter().map(|t| format_ty_kind(&t.kind)).collect();
            format!("({})", inner.join(", "))
        }
    }
}

// ── Scope resolution (unchanged from v0.1) ────────────────────────────────────

use haki_ast::{StmtKind, ExprKind, Binding, Block, FnDef};
type ScopeFrame = HashMap<String, Span>;

pub struct ScopeHit {
    pub name:     String,
    pub def_span: Span,
}

fn resolve_in_fn(f: &FnDef, cursor: u32) -> Option<ScopeHit> {
    if cursor < f.body.span.lo || cursor > f.body.span.hi { return None; }
    let mut scopes: Vec<ScopeFrame> = vec![ScopeFrame::new()];
    for param in &f.params {
        scopes.last_mut().unwrap().insert(param.name.name.clone(), param.name.span);
    }
    resolve_in_block(&f.body, cursor, &mut scopes)
}

fn resolve_in_block(block: &Block, cursor: u32, scopes: &mut Vec<ScopeFrame>) -> Option<ScopeHit> {
    scopes.push(ScopeFrame::new());
    for stmt in &block.stmts {
        let hit = resolve_in_stmt(stmt, cursor, scopes);
        if let StmtKind::Let(l) = &stmt.kind {
            for binding in &l.bindings {
                if let Binding::Name(ident) = binding {
                    if ident.name != "_" {
                        scopes.last_mut().unwrap().insert(ident.name.clone(), ident.span);
                    }
                }
            }
        }
        if hit.is_some() { scopes.pop(); return hit; }
    }
    scopes.pop();
    None
}

fn resolve_in_stmt(stmt: &haki_ast::Stmt, cursor: u32, scopes: &mut Vec<ScopeFrame>) -> Option<ScopeHit> {
    match &stmt.kind {
        StmtKind::Let(l)    => resolve_in_expr(&l.init, cursor, scopes),
        StmtKind::Return(r) => r.values.iter().find_map(|v| resolve_in_expr(v, cursor, scopes)),
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
        StmtKind::Select(_) => None,
    }
}

fn resolve_in_expr(expr: &haki_ast::Expr, cursor: u32, scopes: &mut Vec<ScopeFrame>) -> Option<ScopeHit> {
    if cursor < expr.span.lo || cursor > expr.span.hi { return None; }
    match &expr.kind {
        ExprKind::Ident(ident) if span_contains(expr.span, cursor) => {
            for frame in scopes.iter().rev() {
                if let Some(&def_span) = frame.get(&ident.name) {
                    return Some(ScopeHit { name: ident.name.clone(), def_span });
                }
            }
            None
        }
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
        ExprKind::Field(recv, _)    => resolve_in_expr(recv, cursor, scopes),
        ExprKind::Binary(_, l, r)   => resolve_in_expr(l, cursor, scopes).or_else(|| resolve_in_expr(r, cursor, scopes)),
        ExprKind::Unary(_, e)       => resolve_in_expr(e, cursor, scopes),
        ExprKind::Index(arr, i)     => resolve_in_expr(arr, cursor, scopes).or_else(|| resolve_in_expr(i, cursor, scopes)),
        ExprKind::If(i)             => resolve_in_expr(&i.cond, cursor, scopes).or_else(|| resolve_in_block(&i.then_block, cursor, scopes)),
        ExprKind::Block(b)          => resolve_in_block(b, cursor, scopes),
        ExprKind::Match(m)          => {
            resolve_in_expr(&m.scrutinee, cursor, scopes).or_else(|| {
                for arm in &m.arms {
                    let mut inner = scopes.to_vec();
                    inner.push(ScopeFrame::new());
                    for b in &arm.bindings { inner.last_mut().unwrap().insert(b.name.clone(), b.span); }
                    if let Some(hit) = resolve_in_block(&arm.body, cursor, &mut inner) { return Some(hit); }
                }
                None
            })
        }
        ExprKind::Assign(t, v)      => resolve_in_expr(t, cursor, scopes).or_else(|| resolve_in_expr(v, cursor, scopes)),
        ExprKind::FnLiteral { body, params, .. } => {
            let mut inner = scopes.to_vec();
            inner.push(ScopeFrame::new());
            for p in params { inner.last_mut().unwrap().insert(p.name.name.clone(), p.name.span); }
            resolve_in_block(body, cursor, &mut inner)
        }
        ExprKind::Array(elems) => elems.iter().find_map(|e| resolve_in_expr(e, cursor, scopes)),
        ExprKind::Async(e)     => resolve_in_expr(e, cursor, scopes),
        _                      => None,
    }
}

fn span_contains(span: Span, cursor: u32) -> bool {
    span.lo <= cursor && cursor <= span.hi
}
