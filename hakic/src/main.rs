/// hakic — The Haki compiler.
///
/// Usage:
///   hakic <source.haki>                  compile to native binary (same dir)
///   hakic <source.haki> -o <output>      specify output binary path
///   hakic run <source.haki>              compile + execute immediately
///   hakic lsp                            start the language server (LSP)
///   hakic <source.haki> --emit-ir        write .ll only, do not link
///   hakic <source.haki> --emit-runtime   write haki_runtime.c only
///   hakic <source.haki> --quiet          suppress pipeline progress output
///
/// Pipeline:
///   Source → Lex → Parse → Typeck → Mono → Codegen
///         → .ll → llc → .o + gcc runtime.c → binary

mod lsp;
mod lsp_index;

use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::fs;
use std::collections::{HashMap, HashSet};

// ── Standard library (embedded) ───────────────────────────────────────────────

/// Stdlib sources embedded at compile time via include_str!.
/// Accessed via `import "std/math"`, `import "std/strings"`, etc.
fn stdlib_source(name: &str) -> Option<&'static str> {
    match name {
        "std/math"    | "std/math.haki"    => Some(include_str!("../../stdlib/math.haki")),
        "std/strings" | "std/strings.haki" => Some(include_str!("../../stdlib/strings.haki")),
        "std/path"    | "std/path.haki"    => Some(include_str!("../../stdlib/path.haki")),
        "std/env"     | "std/env.haki"     => Some(include_str!("../../stdlib/env.haki")),
        "std/json"    | "std/json.haki"    => Some(include_str!("../../stdlib/json.haki")),
        "std/time"    | "std/time.haki"    => Some(include_str!("../../stdlib/time.haki")),
        "std/process" | "std/process.haki" => Some(include_str!("../../stdlib/process.haki")),
        "std/regex"   | "std/regex.haki"   => Some(include_str!("../../stdlib/regex.haki")),
        // std/sync — channels, task groups, select
        "std/sync" | "std/sync.haki" => Some(include_str!("../../stdlib/sync.haki")),
        // std/test — assertion framework
        "std/test" | "std/test.haki" => Some(include_str!("../../stdlib/test.haki")),
        // std/fmt — number/string formatting
        "std/fmt" | "std/fmt.haki"   => Some(include_str!("../../stdlib/fmt.haki")),
        // std/net — TCP/UDP sockets
        "std/net" | "std/net.haki"   => Some(include_str!("../../stdlib/net.haki")),
        // std/crypto — SHA-256, HMAC, Base64
        "std/crypto" | "std/crypto.haki" => Some(include_str!("../../stdlib/crypto.haki")),
        // std/db — connection pool, query builder, migrations
        "std/db" | "std/db.haki"         => Some(include_str!("../../stdlib/db.haki")),

        // haki_ui submodules
        "std/haki_ui/element" | "std/haki_ui/element.haki" => Some(include_str!("../../stdlib/haki_ui/element.haki")),
        "std/haki_ui/state"   | "std/haki_ui/state.haki"   => Some(include_str!("../../stdlib/haki_ui/state.haki")),
        "std/haki_ui/view"    | "std/haki_ui/view.haki"    => Some(include_str!("../../stdlib/haki_ui/view.haki")),
        "std/haki_ui/vnode"   | "std/haki_ui/vnode.haki"   => Some(include_str!("../../stdlib/haki_ui/vnode.haki")),
        "std/haki_ui/app"     | "std/haki_ui/app.haki"     => Some(include_str!("../../stdlib/haki_ui/app.haki")),
        _ => None,
    }
}

// ── Module resolution ─────────────────────────────────────────────────────────

/// Resolve all imports reachable from `ast`, returning:
///   - a merged SourceFile containing all imported items (renamed with alias__),
///   - a module registry mapping alias → exported symbols for the typechecker.
///
/// Cycle detection: DFS with a `visiting` set. Hard error on cycle.
fn resolve_modules(
    ast: &haki_ast::SourceFile,
    source_dir: &Path,
) -> Result<(haki_ast::SourceFile, HashMap<String, haki_typeck::ModuleSymbols>), String> {
    let mut merged  = haki_ast::SourceFile { items: vec![], span: ast.span };
    let mut registry: HashMap<String, haki_typeck::ModuleSymbols> = HashMap::new();
    let mut visited:  HashSet<PathBuf> = HashSet::new();
    let mut visiting: Vec<String>      = Vec::new(); // for cycle reporting

    // Process each import in the root file.
    for item in &ast.items {
        if let haki_ast::ItemKind::Import { path, alias, .. } = &item.kind {
            let alias = alias.clone().unwrap_or_else(|| {
                Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path)
                    .to_string()
            });

            // Check stdlib first (embedded in binary).
            if let Some(std_src) = stdlib_source(path) {
                load_module_from_src(std_src, &alias, path, &mut merged, &mut registry)?;
                continue;
            }

            let resolved = resolve_import_path(path, source_dir)?;
            load_module(
                &resolved,
                &alias,
                &mut merged,
                &mut registry,
                &mut visited,
                &mut visiting,
            )?;
        }
    }

    Ok((merged, registry))
}

/// Load a module from an already-read source string (used for embedded stdlib).
fn load_module_from_src(
    src: &str,
    alias: &str,
    display_path: &str,
    merged: &mut haki_ast::SourceFile,
    registry: &mut HashMap<String, haki_typeck::ModuleSymbols>,
) -> Result<(), String> {
    let ast = haki_parser::parse(src)
        .map_err(|e| format!("parse error in std '{}': {e}", display_path))?;
    let top_level_names = module_top_level_names(&ast);

    // Rename items first, then collect — so module symbols carry renamed types.
    let renamed_items: Vec<_> = ast.items.into_iter()
        .map(|item| rename_item(item, alias, &top_level_names))
        .collect();

    // Build a renamed SourceFile to collect symbols from.
    let renamed_ast = haki_ast::SourceFile { items: renamed_items.clone(), span: haki_ast::Span::new(0, 0) };
    let mod_syms = haki_typeck::collect_module(&renamed_ast)
        .map_err(|e| format!("type error in std '{}': {e}", display_path))?;
    registry.insert(alias.to_string(), mod_syms);

    for renamed in renamed_items {
        if !matches!(&renamed.kind, haki_ast::ItemKind::Import { .. }) {
            merged.items.push(renamed);
        }
    }
    Ok(())
}

/// Resolve `"utils/math"` → `/path/to/utils/math.haki` relative to `source_dir`.
fn resolve_import_path(path: &str, source_dir: &Path) -> Result<PathBuf, String> {
    // ── pkg/ imports → global cache ───────────────────────────────────────
    if haki_pkg::resolver::is_pkg_import(path) {
        // Walk up from source_dir to find the project root (haki.json)
        let project_dir = haki_pkg::resolver::find_project_root(source_dir)
            .ok_or_else(|| format!(
                "import error: '{}' requires haki.json — run `hakic pkg install` in your project root",
                path
            ))?;
        return haki_pkg::resolver::resolve_import(path, &project_dir)
            .map_err(|e| format!("import error: {e}"));
    }

    // Strip any leading "./" for cleanliness.
    let clean = path.trim_start_matches("./");
    let with_ext = if clean.ends_with(".haki") {
        clean.to_string()
    } else {
        format!("{clean}.haki")
    };
    let full = source_dir.join(&with_ext);
    if !full.exists() {
        return Err(format!(
            "import error: cannot find '{}' (looked for {})",
            path,
            full.display()
        ));
    }
    // Canonicalize for cycle detection.
    full.canonicalize().map_err(|e| format!("import error: {e}"))
}

/// DFS loader. Parses `path`, detects cycles, renames symbols, merges items.
fn load_module(
    path: &PathBuf,
    alias: &str,
    merged: &mut haki_ast::SourceFile,
    registry: &mut HashMap<String, haki_typeck::ModuleSymbols>,
    visited: &mut HashSet<PathBuf>,
    visiting: &mut Vec<String>,
) -> Result<(), String> {
    // Already fully loaded — skip.
    if visited.contains(path) { return Ok(()); }

    // Cycle check.
    let path_str = path.display().to_string();
    if visiting.iter().any(|v| v == &path_str) {
        let mut chain = visiting.clone();
        chain.push(path_str);
        return Err(format!(
            "circular import detected: {}",
            chain.join(" → ")
        ));
    }

    // Read and parse.
    let src = fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    let ast = haki_parser::parse(&src)
        .map_err(|e| format!("parse error in '{}': {e}", path.display()))?;

    let module_dir = path.parent().unwrap_or(Path::new("."));
    visiting.push(path_str.clone());

    // Recursively load this module's own imports first.
    for item in &ast.items {
        if let haki_ast::ItemKind::Import { path: sub_path, alias: sub_alias, .. } = &item.kind {
            let sub_alias = sub_alias.clone().unwrap_or_else(|| {
                Path::new(sub_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(sub_path)
                    .to_string()
            });
            // Stdlib intercept for transitive imports.
            if let Some(std_src) = stdlib_source(sub_path) {
                load_module_from_src(std_src, &sub_alias, sub_path, merged, registry)?;
                continue;
            }
            let resolved = resolve_import_path(sub_path, module_dir)?;
            load_module(&resolved, &sub_alias, merged, registry, visited, visiting)?;
        }
    }

    visiting.pop();
    visited.insert(path.clone());

    // Collect the set of top-level names so the rename pass only touches
    // module-level declarations, never parameters or locals.
    let top_level_names = module_top_level_names(&ast);

    // Rename all items first, then collect module symbols from the renamed AST.
    // This ensures FnInfo carries renamed type names (e.g. col__Color, not Color).
    let renamed_items: Vec<_> = ast.items.into_iter()
        .map(|item| rename_item(item, alias, &top_level_names))
        .collect();

    let renamed_ast = haki_ast::SourceFile { items: renamed_items.clone(), span: haki_ast::Span::new(0, 0) };
    let mod_syms = haki_typeck::collect_module(&renamed_ast)
        .map_err(|e| format!("type error in '{}': {e}", path.display()))?;
    registry.insert(alias.to_string(), mod_syms);

    // Append renamed items to merged AST (skip Import nodes already processed).
    for renamed in renamed_items {
        if !matches!(&renamed.kind, haki_ast::ItemKind::Import { .. }) {
            merged.items.push(renamed);
        }
    }

    Ok(())
}

/// Rename all top-level declarations in `item` by prepending `alias__`,
/// AND rewrite all call sites within function bodies so intra-module
/// calls (`hello(...)` inside greet.haki) become `alias__hello(...)`.
/// Collect the set of top-level names defined in this module's AST.
fn module_top_level_names(ast: &haki_ast::SourceFile) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in &ast.items {
        match &item.kind {
            haki_ast::ItemKind::Fn(f)       => { names.insert(f.name.name.clone()); }
            haki_ast::ItemKind::Struct(s)   => { names.insert(s.name.name.clone()); }
            haki_ast::ItemKind::Class(c)    => { names.insert(c.name.name.clone()); }
            haki_ast::ItemKind::Protocol(p) => { names.insert(p.name.name.clone()); }
            haki_ast::ItemKind::Enum(e)     => {
                names.insert(e.name.name.clone());
                // Also include variant names so enum constructors in bodies get renamed.
                for variant in &e.variants {
                    names.insert(variant.name.name.clone());
                }
            }
            _ => {}
        }
    }
    names
}

/// Rename all top-level declarations by prepending `alias__`,
/// AND rewrite intra-module call sites in function bodies.
/// Only renames identifiers that are top-level names in this module
/// (never parameters, locals, or stdlib names).
fn rename_item(mut item: haki_ast::Item, alias: &str, module_names: &HashSet<String>) -> haki_ast::Item {
    match &mut item.kind {
        haki_ast::ItemKind::Fn(f) => {
            f.name.name = format!("{alias}__{}", f.name.name);
            // Rename type annotations in params and return type.
            for p in &mut f.params { rename_ty(&mut p.ty, alias, module_names); }
            if let Some(ret) = &mut f.return_ty { rename_return_ty(ret, alias, module_names); }
            rename_block(&mut f.body, alias, module_names);
        }
        haki_ast::ItemKind::Struct(s) => {
            s.name.name = format!("{alias}__{}", s.name.name);
            for field in &mut s.fields { rename_ty(&mut field.ty, alias, module_names); }
            for m in &mut s.methods {
                m.name.name = format!("{alias}__{}", m.name.name);
                for p in &mut m.params { rename_ty(&mut p.ty, alias, module_names); }
                if let Some(ret) = &mut m.return_ty { rename_return_ty(ret, alias, module_names); }
                rename_block(&mut m.body, alias, module_names);
            }
        }
        haki_ast::ItemKind::Class(c) => {
            c.name.name = format!("{alias}__{}", c.name.name);
            for field in &mut c.fields { rename_ty(&mut field.ty, alias, module_names); }
            for m in &mut c.methods {
                m.name.name = format!("{alias}__{}", m.name.name);
                for p in &mut m.params { rename_ty(&mut p.ty, alias, module_names); }
                if let Some(ret) = &mut m.return_ty { rename_return_ty(ret, alias, module_names); }
                rename_block(&mut m.body, alias, module_names);
            }
        }
        haki_ast::ItemKind::Protocol(p) => {
            p.name.name = format!("{alias}__{}", p.name.name);
        }
        haki_ast::ItemKind::Enum(e) => {
            e.name.name = format!("{alias}__{}", e.name.name);
            // Rename variant names and their payload field types.
            for variant in &mut e.variants {
                variant.name.name = format!("{alias}__{}", variant.name.name);
                for field_ty in &mut variant.fields {
                    rename_ty(field_ty, alias, module_names);
                }
            }
        }
        haki_ast::ItemKind::Impl(i) => {
            i.target.name = format!("{alias}__{}", i.target.name);
            for m in &mut i.methods {
                m.name.name = format!("{alias}__{}", m.name.name);
                for p in &mut m.params { rename_ty(&mut p.ty, alias, module_names); }
                if let Some(ret) = &mut m.return_ty { rename_return_ty(ret, alias, module_names); }
                rename_block(&mut m.body, alias, module_names);
            }
        }
        haki_ast::ItemKind::Import { .. } => {}
        haki_ast::ItemKind::ExternFn(f) => {
            // Rename the extern fn itself and its type annotations
            f.name.name = format!("{alias}__{}", f.name.name);
            for p in &mut f.params { rename_ty(&mut p.ty, alias, module_names); }
            if let Some(ret) = &mut f.return_ty { rename_return_ty(ret, alias, module_names); }
        }
    }
    item
}

/// Rename module-local type names within a Ty node.
fn rename_ty(ty: &mut haki_ast::Ty, alias: &str, names: &HashSet<String>) {
    use haki_ast::TyKind;
    match &mut ty.kind {
        TyKind::Named(id) => {
            if names.contains(&id.name) {
                id.name = format!("{alias}__{}", id.name);
            }
        }
        TyKind::Generic(id, args) => {
            if names.contains(&id.name) {
                id.name = format!("{alias}__{}", id.name);
            }
            for a in args { rename_ty(a, alias, names); }
        }
        TyKind::Optional(inner) => rename_ty(inner, alias, names),
        TyKind::Fn(params, ret) => {
            for p in params { rename_ty(p, alias, names); }
            if let Some(r) = ret { rename_ty(r, alias, names); }
        }
        TyKind::Tuple(tys) => { for t in tys { rename_ty(t, alias, names); } }
    }
}

fn rename_return_ty(ret: &mut haki_ast::ReturnTy, alias: &str, names: &HashSet<String>) {
    match ret {
        haki_ast::ReturnTy::Single(ty) => rename_ty(ty, alias, names),
        haki_ast::ReturnTy::Tuple(tys) => { for t in tys { rename_ty(t, alias, names); } }
    }
}

fn rename_block(block: &mut haki_ast::Block, alias: &str, names: &HashSet<String>) {
    for stmt in &mut block.stmts { rename_stmt(stmt, alias, names); }
}

fn rename_stmt(stmt: &mut haki_ast::Stmt, alias: &str, names: &HashSet<String>) {
    match &mut stmt.kind {
        haki_ast::StmtKind::Let(l) => {
            if let Some(ty) = &mut l.ty { rename_ty(ty, alias, names); }
            rename_expr(&mut l.init, alias, names);
        }
        haki_ast::StmtKind::Return(r) => { for e in &mut r.values { rename_expr(e, alias, names); } }
        haki_ast::StmtKind::Yield(e)  => rename_expr(e, alias, names),
        haki_ast::StmtKind::Defer(e)  => rename_expr(e, alias, names),
        haki_ast::StmtKind::Continue | haki_ast::StmtKind::Break => {}
        haki_ast::StmtKind::Select(_) => {}
        haki_ast::StmtKind::Expr(e)   => rename_expr(e, alias, names),
        haki_ast::StmtKind::Panic(e)  => rename_expr(e, alias, names),
        haki_ast::StmtKind::If(i) => {
            rename_expr(&mut i.cond, alias, names);
            rename_block(&mut i.then_block, alias, names);
            if let Some(els) = &mut i.else_branch { match els {
                haki_ast::ElseBranch::Block(b) => rename_block(b, alias, names),
                haki_ast::ElseBranch::If(inner) => { rename_expr(&mut inner.cond, alias, names); rename_block(&mut inner.then_block, alias, names); }
            }}
        }
        haki_ast::StmtKind::For(f)   => { rename_expr(&mut f.iter, alias, names); rename_block(&mut f.body, alias, names); }
        haki_ast::StmtKind::While(w) => { rename_expr(&mut w.cond, alias, names); rename_block(&mut w.body, alias, names); }
        haki_ast::StmtKind::Match(m) => {
            rename_expr(&mut m.scrutinee, alias, names);
            for arm in &mut m.arms {
                // Only rename Ident patterns (variant/class names), not literals
                if let haki_ast::MatchPattern::Ident(ref mut ident) = arm.pattern {
                    if names.contains(&ident.name) {
                        ident.name = format!("{alias}__{}", ident.name);
                    }
                }
                rename_block(&mut arm.body, alias, names);
            }
        }
    }
}

fn rename_expr(expr: &mut haki_ast::Expr, alias: &str, names: &HashSet<String>) {
    use haki_ast::ExprKind;
    match &mut expr.kind {
        ExprKind::Call(callee, args) => { rename_expr(callee, alias, names); for a in args { rename_expr(a, alias, names); } }
        ExprKind::NamedCall(callee, args) => { rename_expr(callee, alias, names); for a in args { rename_expr(&mut a.value, alias, names); } }
        ExprKind::Ident(id) => { if names.contains(&id.name) { id.name = format!("{alias}__{}", id.name); } }
        ExprKind::Field(recv, _) => rename_expr(recv, alias, names),
        ExprKind::MethodCall(recv, _, args) => { rename_expr(recv, alias, names); for a in args { rename_expr(a, alias, names); } }
        ExprKind::Binary(_, l, r) => { rename_expr(l, alias, names); rename_expr(r, alias, names); }
        ExprKind::Unary(_, e) => rename_expr(e, alias, names),
        ExprKind::Assign(t, v) => { rename_expr(t, alias, names); rename_expr(v, alias, names); }
        ExprKind::If(i) => {
            rename_expr(&mut i.cond, alias, names);
            rename_block(&mut i.then_block, alias, names);
            if let Some(els) = &mut i.else_branch { match els {
                haki_ast::ElseBranch::Block(b) => rename_block(b, alias, names),
                haki_ast::ElseBranch::If(inner) => { rename_expr(&mut inner.cond, alias, names); rename_block(&mut inner.then_block, alias, names); }
            }}
        }
        ExprKind::Block(b) => rename_block(b, alias, names),
        ExprKind::Match(m) => {
            rename_expr(&mut m.scrutinee, alias, names);
            for arm in &mut m.arms {
                if let haki_ast::MatchPattern::Ident(ref mut ident) = arm.pattern {
                    if names.contains(&ident.name) {
                        ident.name = format!("{alias}__{}", ident.name);
                    }
                }
                rename_block(&mut arm.body, alias, names);
            }
        }
        ExprKind::Array(elems) => { for e in elems { rename_expr(e, alias, names); } }
        ExprKind::Index(arr, i) => { rename_expr(arr, alias, names); rename_expr(i, alias, names); }
        ExprKind::Async(e) => rename_expr(e, alias, names),
        ExprKind::FnLiteral { body, .. } => rename_block(body, alias, names),
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::String(_) | ExprKind::Bool(_) | ExprKind::Null => {}
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────


// ── Source location helpers ───────────────────────────────────────────────────

/// Convert a byte offset into `source` to a (line, col) pair (1-indexed).
fn byte_to_linecol(source: &str, offset: u32) -> (usize, usize) {
    let offset = offset as usize;
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.chars().filter(|&c| c == '\n').count() + 1;
    let col  = prefix.rfind('\n').map_or(offset, |n| offset - n - 1) + 1;
    (line, col)
}

/// Format an error with line:col context instead of raw byte offsets.
/// Looks for `Span { lo: N, hi: M }` patterns and replaces them.
/// Format a compiler error with Rust/Elm-style rich diagnostics:
///
///   error: expected `for`, found `.`
///    --> counter.haki:10:10
///     |
///  10 |     for.each(items)
///     |        ^ unexpected `.`
///
fn format_error(e: &dyn std::fmt::Display, src: &str) -> String {
    let raw = format!("{e}");

    // Extract the first Span { lo: N, hi: M } from the error message
    // and replace the whole message with a rich diagnostic block.
    let mut result = String::new();
    let mut rest   = raw.as_str();
    let mut first_span: Option<(u32, u32)> = None;

    // Collect all span replacements
    let mut flat = String::new();
    while let Some(idx) = rest.find("Span { lo: ") {
        flat.push_str(&rest[..idx]);
        rest = &rest[idx + "Span { lo: ".len()..];
        if let Some(comma) = rest.find(", hi: ") {
            if let Ok(lo) = rest[..comma].trim().parse::<u32>() {
                let after_hi = &rest[comma + ", hi: ".len()..];
                if let Some(close) = after_hi.find('}') {
                    if let Ok(hi) = after_hi[..close].trim().parse::<u32>() {
                        let (line, col) = byte_to_linecol(src, lo);
                        if first_span.is_none() { first_span = Some((lo, hi)); }
                        flat.push_str(&format!("{line}:{col}"));
                        rest = &after_hi[close + 1..];
                        continue;
                    }
                }
            }
        }
        flat.push_str("Span { lo: ");
    }
    flat.push_str(rest);

    // If we found a span, emit rich diagnostic
    if let Some((lo, hi)) = first_span {
        let (line_num, col_num) = byte_to_linecol(src, lo);
        let span_len = ((hi - lo) as usize).max(1);

        // Extract the source line
        let source_line = src.lines().nth(line_num - 1).unwrap_or("");

        // Gutter width based on line number digits
        let gutter = line_num.to_string().len();

        // Strip the raw "hakic: " prefix from the message for cleaner output
        let msg = flat.trim_start_matches("hakic: ").trim_start_matches("hakic:");

        result.push_str(&format!("error: {msg}
"));
        result.push_str(&format!(" {:gutter$}--> {}:{line_num}:{col_num}
",
                                  "", "source"));
        result.push_str(&format!(" {:gutter$} |
", ""));
        result.push_str(&format!(" {line_num} | {source_line}
"));
        // Caret line: col_num - 1 spaces, then ^ repeated for span length
        // Clamp caret to line length
        let caret_start = (col_num - 1).min(source_line.len());
        let caret_len   = span_len.min(source_line.len().saturating_sub(caret_start)).max(1);
        result.push_str(&format!(" {:gutter$} | {}{}
",
                                  "",
                                  " ".repeat(caret_start),
                                  "^".repeat(caret_len)));
    } else {
        // No span found — emit plain message
        result.push_str(&flat);
    }

    result
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();

    // ── Binary alias dispatch ─────────────────────────────────────────────────
    // When invoked as haki-gtk, haki-dom, or haki-web, inject the appropriate
    // --target flag and run-mode before handing off to the normal argument parser.
    //
    //   haki-gtk app.haki    →  haki --target gtk app.haki    (compile + run GTK)
    //   haki-dom app.haki    →  haki --target dom app.haki    (compile to .wasm)
    //   haki-web app.haki    →  haki --target so  app.haki    (compile to .so)
    //
    // Any extra flags the user passes (e.g. -o, --quiet) are preserved.
    let binary_name = raw_args[0]
        .split(['/', '\\'])   // handle full paths on both Unix and Windows
        .last()
        .unwrap_or("hakic")
        .to_string();
    // Strip .exe on Windows
    let binary_stem = binary_name.trim_end_matches(".exe");

    let args: Vec<String> = match binary_stem {
        "haki-gtk" => {
            // Compile + run the GTK app: inject --target gtk
            let mut v = vec![raw_args[0].clone(), "--target".into(), "gtk".into()];
            v.extend_from_slice(&raw_args[1..]);
            v
        }
        "haki-dom" => {
            // Compile to Wasm for browser: inject --emit-wasm
            let mut v = vec![raw_args[0].clone(), "--emit-wasm".into()];
            v.extend_from_slice(&raw_args[1..]);
            v
        }
        "haki-web" => {
            // Compile to .so for mod_haki/FastCGI: inject --target so
            let mut v = vec![raw_args[0].clone(), "--target".into(), "so".into()];
            v.extend_from_slice(&raw_args[1..]);
            v
        }
        _ => raw_args,
    };

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    // Handle --version and --help before anything else.
    if args[1] == "--version" || args[1] == "-V" {
        println!("haki 2.2.0 — Haki compiler");
        println!("  haki          run any .haki file");
        println!("  haki-gtk      compile + run as GTK desktop app");
        println!("  haki-dom      compile to WebAssembly for the browser");
        println!("  haki-web      compile to .so for Apache/nginx (mod_haki)");
        println!("  hakic         compiler driver (tooling/CI alias)");
        println!("https://github.com/iceman5508/haki-lang");
        return;
    }
    if args[1] == "--help" || args[1] == "-h" {
        print_usage();
        return;
    }

    // `hakic doc <file>` — generate documentation markdown.
    if args[1] == "doc" {
        if args.len() < 3 {
            eprintln!("usage: hakic doc <source.haki>");
            process::exit(1);
        }
        let source = PathBuf::from(&args[2]);
        doc_file(&source);
        return;
    }

    // `hakic fmt <file>` — format source in place.
    if args[1] == "fmt" {
        if args.len() < 3 {
            eprintln!("usage: hakic fmt <source.haki>");
            process::exit(1);
        }
        let source   = PathBuf::from(&args[2]);
        let check_only = args.iter().any(|a| a == "--check");
        fmt_file(&source, check_only);
        return;
    }

    // `hakic test <file>` — run test_* functions.
    if args[1] == "test" {
        if args.len() < 3 {
            eprintln!("usage: hakic test <source.haki>");
            process::exit(1);
        }
        let source = PathBuf::from(&args[2]);
        let quiet  = args.iter().any(|a| a == "--quiet");
        run_tests(&source, quiet);
        return;
    }

    // `hakic check <file>` — typecheck only, no codegen.
    if args[1] == "check" {
        if args.len() < 3 {
            eprintln!("usage: hakic check <source.haki>");
            process::exit(1);
        }
        let source = PathBuf::from(&args[2]);
        let quiet  = args.iter().any(|a| a == "--quiet");
        check_only(&source, quiet);
        return;
    }

    // `hakic lsp` — start the Language Server Protocol daemon.
    if args[1] == "lsp" {
        lsp::run_lsp();
        return;
    }

    // ── hakic pkg <subcommand> ─────────────────────────────────────────────
    if args[1] == "pkg" {
        let sub = args.get(2).map(|s| s.as_str()).unwrap_or("help");
        let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let result = match sub {
            "init" => {
                let name = args.get(3).map(|s| s.as_str());
                haki_pkg::commands::cmd_init(name, &project_dir)
            }
            "add" => {
                if args.len() < 4 {
                    eprintln!("usage: hakic pkg add <url> [as <alias>]");
                    process::exit(1);
                }
                let url = &args[3];
                // Support: hakic pkg add <url> as <alias>
                let alias = if args.get(4).map(|s| s.as_str()) == Some("as") {
                    args.get(5).map(|s| s.as_str())
                } else {
                    None
                };
                haki_pkg::commands::cmd_add(url, alias, &project_dir)
            }
            "install" => haki_pkg::commands::cmd_install(&project_dir),
            "update" => {
                let alias = args.get(3).map(|s| s.as_str());
                haki_pkg::commands::cmd_update(alias, &project_dir)
            }
            "remove" | "rm" => {
                if args.len() < 4 {
                    eprintln!("usage: hakic pkg remove <alias>");
                    process::exit(1);
                }
                haki_pkg::commands::cmd_remove(&args[3], &project_dir)
            }
            "list" | "ls" => haki_pkg::commands::cmd_list(&project_dir),
            "help" | "--help" | "-h" | _ => {
                println!("hakic pkg — Haki package manager\n");
                println!("Usage:");
                println!("  hakic pkg init [name]           Create haki.json");
                println!("  hakic pkg add <url> [as <name>] Add a dependency");
                println!("  hakic pkg install               Install all dependencies");
                println!("  hakic pkg update [name]         Update one or all deps");
                println!("  hakic pkg remove <name>         Remove a dependency");
                println!("  hakic pkg list                  List dependencies");
                println!("\nURL format:");
                println!("  https://github.com/user/repo          latest default branch");
                println!("  https://github.com/user/repo#v1.2.0   specific tag");
                println!("  https://github.com/user/repo#main     specific branch");
                println!("\nImport syntax:");
                println!("  import \"pkg/utils/strings\" as strings");
                return;
            }
        };

        if let Err(e) = result {
            eprintln!("hakic pkg {sub}: {e}");
            process::exit(1);
        }
        return;
    }

    // Detect `hakic run <file>` subcommand.
    if args[1] == "run" {
        if args.len() < 3 {
            eprintln!("usage: hakic run <source.haki>");
            process::exit(1);
        }
        let run_args = RunArgs {
            source:      PathBuf::from(&args[2]),
            output:      None,
            emit_ir:     false,
            emit_runtime:false,
            emit_wasm:   false,
            emit_c:      false,
            quiet:       args.iter().any(|a| a == "--quiet"),
            run:         true,
            run_args:    args[3..].iter()
                            .filter(|a| !a.starts_with("--"))
                            .cloned().collect(),
            target_so:   false,
            target_gtk:  false,
        };
        compile_and_run(run_args);
        return;
    }

    // `hakic watch <file>` — recompile and restart on file change.
    if args[1] == "watch" {
        if args.len() < 3 {
            eprintln!("usage: hakic watch <source.haki> [-- args...]");
            process::exit(1);
        }
        let source    = PathBuf::from(&args[2]);
        let run_args: Vec<String> = args[4..].to_vec();
        watch_mode(&source, run_args);
        return;
    }

    // `hakic repl` — interactive read-eval-print loop.
    if args[1] == "repl" {
        run_repl();
        return;
    }

    // Normal compile mode.
    // Scan all arguments: find the source file (first non-flag arg after index 1),
    // then parse all flags regardless of order.
    // This allows: `hakic --target so handler.haki -o handler.so`
    //          and: `hakic handler.haki --target so -o handler.so`
    let mut source_opt:   Option<PathBuf> = None;
    let mut output:       Option<PathBuf> = None;
    let mut emit_ir       = false;
    let mut emit_runtime  = false;
    let mut emit_wasm     = false;
    let mut emit_c_flag   = false;
    let mut quiet         = false;
    let mut target_so     = false;
    let mut target_gtk    = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i < args.len() { output = Some(PathBuf::from(&args[i])); }
            }
            "--emit-ir"      => emit_ir      = true,
            "--emit-runtime" => emit_runtime = true,
            "--emit-wasm"    => emit_wasm    = true,
            "--emit-c"       => emit_c_flag  = true,
            "--quiet"        => quiet        = true,
            "--target" => {
                i += 1;
                if i < args.len() {
                    match args[i].as_str() {
                        "so"  => { target_so   = true; emit_c_flag = true; }
                        "gtk" => { target_gtk  = true; emit_c_flag = true; }
                        "dom" => { emit_wasm   = true; }
                        other => { eprintln!("hakic: unknown target '{other}' (known: so, gtk, dom)"); process::exit(1); }
                    }
                }
            }
            arg if !arg.starts_with('-') => {
                // First positional argument is the source file
                if source_opt.is_none() {
                    source_opt = Some(PathBuf::from(arg));
                }
            }
            _ => {}
        }
        i += 1;
    }

    let source = match source_opt {
        Some(s) => s,
        None => {
            eprintln!("hakic: no source file specified");
            print_usage();
            process::exit(1);
        }
    };

    // If the source is a .haki file and no output/emit flags specified,
    // treat as `hakic run` — compile and execute immediately.
    let is_run_mode = output.is_none()
        && !emit_ir && !emit_runtime && !emit_wasm && !emit_c_flag
        && source.extension().and_then(|e| e.to_str()) == Some("haki");

    if is_run_mode {
        let run_args = RunArgs {
            source:       source.clone(),
            output:       None,
            emit_ir:      false,
            emit_runtime: false,
            emit_wasm:    false,
            emit_c:       true,  // bare-path always uses portable C backend
            quiet,
            run:          true,
            run_args:     args[2..].iter()
                            .filter(|a| !a.starts_with("--"))
                            .cloned().collect(),
            target_so:    false,
            target_gtk:   false,
        };
        compile_and_run(run_args);
        return;
    }

    compile_and_run(RunArgs { source, output, emit_ir, emit_runtime, emit_wasm, emit_c: emit_c_flag, quiet, run: false, run_args: vec![], target_so, target_gtk });
}

fn print_usage() {
    println!("Haki compiler v1.7.0");
    println!();
    println!("Usage:");
    println!("  hakic <source.haki>              compile to native binary");
    println!("  hakic <source.haki> -o <output>  specify output path");
    println!("  hakic run <source.haki>           compile and run immediately");
    println!("  hakic check <source.haki>         typecheck only, no codegen");
    println!("  hakic test <source.haki>          run test_* functions");
    println!("  hakic fmt <source.haki>           format source in place");
    println!("  hakic fmt <source.haki> --check   check formatting without writing");
    println!("  hakic watch <source.haki>         recompile + restart on save (100ms polling)");
    println!("  hakic repl                        interactive REPL");
    println!("  hakic doc <source.haki>           generate documentation markdown");
    println!();
    println!("Flags:");
    println!("  --emit-ir       write LLVM IR (.ll) only, do not link");
    println!("  --emit-wasm     write WebAssembly (.wasm) binary");
    println!("  --emit-runtime  write haki_runtime.c only");
    println!("  --quiet         suppress pipeline progress output");
    println!("  --version       print version and exit");
    println!("  --help          print this help and exit");
}

// ── Compilation arguments ─────────────────────────────────────────────────────

struct RunArgs {
    source:       PathBuf,
    output:       Option<PathBuf>,
    emit_ir:      bool,
    emit_runtime: bool,
    emit_wasm:    bool,
    emit_c:       bool,
    quiet:        bool,
    run:          bool,
    run_args:     Vec<String>,
    /// Compile to a shared library (.so) for use with mod_haki.
    /// Suppresses main(), exports haki_handle_request.
    target_so:    bool,
    /// Compile + run as a GTK desktop application.
    /// Links haki_ui_gtk.c + libgtk-3, runs the resulting binary.
    target_gtk:   bool,
}

// ── Doc generator (hakic doc) ─────────────────────────────────────────────────

/// Extract `///` doc comments from source and emit a Markdown document.
/// Scans raw source for doc-comment blocks immediately preceding declarations.
fn doc_file(source: &Path) {
    let src = match fs::read_to_string(source) {
        Ok(s) => s,
        Err(e) => { eprintln!("hakic: cannot read '{}': {e}", source.display()); process::exit(1); }
    };

    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    println!("# {}", stem);
    println!();

    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Collect consecutive `///` doc comment lines.
        if trimmed.starts_with("///") {
            let mut doc_lines: Vec<&str> = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with("///") {
                let content = lines[i].trim().trim_start_matches("///").trim_start_matches(' ');
                doc_lines.push(content);
                i += 1;
            }

            // The next non-blank line is the declaration this doc comment belongs to.
            while i < lines.len() && lines[i].trim().is_empty() { i += 1; }
            if i >= lines.len() { break; }

            let decl = lines[i].trim();
            if let Some(sig) = extract_signature(decl) {
                println!("## `{sig}`");
                println!();
                for dl in &doc_lines {
                    println!("{dl}");
                }
                println!();
            }
        } else {
            i += 1;
        }
    }
}

/// Extract a short signature from a declaration line for display.
fn extract_signature(line: &str) -> Option<String> {
    let trimmed = line.trim();
    // fn, struct, class, protocol
    for kw in &["fn ", "struct ", "class ", "protocol "] {
        if trimmed.starts_with(kw) {
            // Extract up to the opening brace or end
            let _sig = trimmed.split('{').next()?.trim().trim_end_matches(')');
            // Trim trailing whitespace + include closing paren if fn
            let full = if trimmed.starts_with("fn ") {
                // Keep up to and including the ')' + optional return type
                let s = trimmed.split('{').next()?.trim();
                s.to_string()
            } else {
                trimmed.split('{').next()?.trim().to_string()
            };
            return Some(full);
        }
    }
    None
}

// ── Formatter (hakic fmt) ─────────────────────────────────────────────────────

fn fmt_file(source: &Path, check_only: bool) {
    let src = match fs::read_to_string(source) {
        Ok(s) => s,
        Err(e) => { eprintln!("hakic: cannot read '{}': {e}", source.display()); process::exit(1); }
    };
    let ast = match haki_parser::parse(&src) {
        Ok(a) => a,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };

    let formatted = fmt_source_file(&ast, &src);

    if check_only {
        if formatted == src {
            println!("✓  {} — already formatted", source.display());
        } else {
            eprintln!("✗  {} — needs formatting", source.display());
            process::exit(1);
        }
        return;
    }

    if formatted != src {
        if let Err(e) = fs::write(source, &formatted) {
            eprintln!("hakic: cannot write '{}': {e}", source.display()); process::exit(1);
        }
        println!("formatted {}", source.display());
    }
}

/// Walk the untyped AST and emit canonically formatted Haki source.
fn fmt_source_file(ast: &haki_ast::SourceFile, original: &str) -> String {
    let mut out = String::new();
    for (i, item) in ast.items.iter().enumerate() {
        if i > 0 { out.push('\n'); }
        fmt_item(&mut out, item, original);
        out.push('\n');
    }
    out
}

fn fmt_item(out: &mut String, item: &haki_ast::Item, src: &str) {
    use haki_ast::ItemKind;
    match &item.kind {
        ItemKind::Import { path, alias, .. } => {
            out.push_str("import \"");
            out.push_str(path);
            out.push('"');
            if let Some(a) = alias {
                out.push_str(" as ");
                out.push_str(a);
            }
        }
        ItemKind::Fn(f)       => fmt_fn_def(out, f, src, 0),
        ItemKind::Struct(s)   => fmt_struct(out, s, src),
        ItemKind::Class(c)    => fmt_class(out, c, src),
        ItemKind::Enum(e)     => fmt_enum(out, e),
        ItemKind::Protocol(p) => fmt_protocol(out, p, src),
        ItemKind::Impl(i)     => fmt_impl(out, i, src),
        ItemKind::ExternFn(f) => {
            out.push_str("extern \"");
            out.push_str(&f.abi);
            out.push_str("\" fn ");
            out.push_str(&f.name.name);
            out.push('(');
            for (i, p) in f.params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(&p.name.name);
                out.push_str(": ");
                fmt_ty(out, &p.ty);
            }
            out.push(')');
            if let Some(ret) = &f.return_ty {
                out.push_str(" -> ");
                fmt_return_ty(out, ret);
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth { out.push_str("    "); }
}

fn fmt_fn_def(out: &mut String, f: &haki_ast::FnDef, src: &str, depth: usize) {
    indent(out, depth);
    out.push_str("fn ");
    out.push_str(&f.name.name);
    if !f.type_params.is_empty() {
        out.push('<');
        for (i, tp) in f.type_params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&tp.name.name);
            if !tp.bounds.is_empty() {
                out.push_str(": ");
                out.push_str(&tp.bounds.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(" + "));
            }
        }
        out.push('>');
    }
    out.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push_str(&p.name.name);
        out.push_str(": ");
        fmt_ty(out, &p.ty);
    }
    out.push(')');
    if let Some(ret) = &f.return_ty {
        out.push_str(" -> ");
        fmt_return_ty(out, ret);
    }
    out.push_str(" {\n");
    fmt_block_stmts(out, &f.body, src, depth + 1);
    indent(out, depth);
    out.push('}');
}

fn fmt_struct(out: &mut String, s: &haki_ast::StructDef, src: &str) {
    out.push_str("struct ");
    out.push_str(&s.name.name);
    if !s.type_params.is_empty() {
        out.push('<');
        out.push_str(&s.type_params.iter().map(|tp| tp.name.name.as_str()).collect::<Vec<_>>().join(", "));
        out.push('>');
    }
    out.push_str(" {\n");
    for field in &s.fields {
        out.push_str("    ");
        out.push_str(if field.mutability == haki_ast::Mut::Const { "const " } else { "let " });
        if field.is_weak { out.push_str("weak "); }
        out.push_str(&field.name.name);
        out.push_str(": ");
        fmt_ty(out, &field.ty);
        out.push('\n');
    }
    for m in &s.methods {
        out.push('\n');
        fmt_fn_def(out, m, src, 1);
        out.push('\n');
    }
    out.push('}');
}

fn fmt_enum(out: &mut String, e: &haki_ast::EnumDef) {
    out.push_str("enum ");
    out.push_str(&e.name.name);
    if !e.type_params.is_empty() {
        out.push('<');
        out.push_str(&e.type_params.iter().map(|tp| tp.name.name.as_str()).collect::<Vec<_>>().join(", "));
        out.push('>');
    }
    out.push_str(" {\n");
    for v in &e.variants {
        out.push_str("    ");
        out.push_str(&v.name.name);
        if !v.fields.is_empty() {
            out.push('(');
            for (i, f) in v.fields.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_ty(out, f);
            }
            out.push(')');
        }
        out.push('\n');
    }
    out.push('}');
}

fn fmt_class(out: &mut String, c: &haki_ast::ClassDef, src: &str) {
    out.push_str("class ");
    out.push_str(&c.name.name);
    if let Some(sup) = &c.superclass {
        out.push_str(" extends ");
        out.push_str(&sup.name);
    }
    out.push_str(" {\n");
    for field in &c.fields {
        out.push_str("    ");
        out.push_str(if field.mutability == haki_ast::Mut::Const { "const " } else { "let " });
        if field.is_weak { out.push_str("weak "); }
        out.push_str(&field.name.name);
        out.push_str(": ");
        fmt_ty(out, &field.ty);
        out.push('\n');
    }
    for m in &c.methods {
        out.push('\n');
        fmt_fn_def(out, m, src, 1);
        out.push('\n');
    }
    out.push('}');
}

fn fmt_protocol(out: &mut String, p: &haki_ast::ProtocolDef, src: &str) {
    out.push_str("protocol ");
    out.push_str(&p.name.name);
    out.push_str(" {\n");
    for sig in &p.methods {
        out.push_str("    fn ");
        out.push_str(&sig.name.name);
        out.push('(');
        for (i, param) in sig.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&param.name.name);
            out.push_str(": ");
            fmt_ty(out, &param.ty);
        }
        out.push(')');
        if let Some(ret) = &sig.return_ty {
            out.push_str(" -> ");
            fmt_return_ty(out, ret);
        }
        out.push('\n');
    }
    for d in &p.default_methods {
        out.push('\n');
        fmt_fn_def(out, d, src, 1);
        out.push('\n');
    }
    out.push('}');
}

fn fmt_impl(out: &mut String, i: &haki_ast::ImplBlock, src: &str) {
    out.push_str("impl ");
    out.push_str(&i.protocol.name);
    out.push_str(" for ");
    out.push_str(&i.target.name);
    out.push_str(" {\n");
    for m in &i.methods {
        fmt_fn_def(out, m, src, 1);
        out.push('\n');
    }
    out.push('}');
}

fn fmt_ty(out: &mut String, ty: &haki_ast::Ty) {
    use haki_ast::TyKind;
    match &ty.kind {
        TyKind::Named(id)      => out.push_str(&id.name),
        TyKind::Optional(inner) => { fmt_ty(out, inner); out.push('?'); }
        TyKind::Generic(id, args) => {
            out.push_str(&id.name);
            out.push('<');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_ty(out, a);
            }
            out.push('>');
        }
        TyKind::Fn(params, ret) => {
            out.push_str("fn(");
            for (i, p) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_ty(out, p);
            }
            out.push(')');
            if let Some(r) = ret {
                out.push_str(" -> ");
                fmt_ty(out, r);
            }
        }
        TyKind::Tuple(tys) => {
            out.push('(');
            for (i, t) in tys.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_ty(out, t);
            }
            out.push(')');
        }
    }
}

fn fmt_return_ty(out: &mut String, ret: &haki_ast::ReturnTy) {
    match ret {
        haki_ast::ReturnTy::Single(ty) => fmt_ty(out, ty),
        haki_ast::ReturnTy::Tuple(tys) => {
            out.push('(');
            for (i, t) in tys.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_ty(out, t);
            }
            out.push(')');
        }
    }
}

fn fmt_block_stmts(out: &mut String, block: &haki_ast::Block, src: &str, depth: usize) {
    for stmt in &block.stmts {
        fmt_stmt(out, stmt, src, depth);
    }
}

fn fmt_stmt(out: &mut String, stmt: &haki_ast::Stmt, src: &str, depth: usize) {
    use haki_ast::StmtKind;
    indent(out, depth);
    match &stmt.kind {
        StmtKind::Let(l) => {
            let kw = if l.mutability == haki_ast::Mut::Const { "const " } else { "let " };
            out.push_str(kw);
            for (i, b) in l.bindings.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                match b {
                    haki_ast::Binding::Name(id)    => out.push_str(&id.name),
                    haki_ast::Binding::Discard(_)  => out.push('_'),
                }
            }
            if let Some(ann) = &l.ty {
                out.push_str(": ");
                fmt_ty(out, ann);
            }
            out.push_str(" = ");
            fmt_expr(out, &l.init, src, depth);
            out.push('\n');
        }
        StmtKind::Return(r) => {
            out.push_str("return");
            if !r.values.is_empty() {
                out.push(' ');
                for (i, v) in r.values.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    fmt_expr(out, v, src, depth);
                }
            }
            out.push('\n');
        }
        StmtKind::Yield(e) => {
            out.push_str("yield ");
            fmt_expr(out, e, src, depth);
            out.push('\n');
        }
        StmtKind::Defer(e) => {
            out.push_str("defer ");
            fmt_expr(out, e, src, depth);
            out.push('\n');
        }
        StmtKind::Continue => { out.push_str("continue\n"); }
        StmtKind::Break    => { out.push_str("break\n"); }
        StmtKind::Panic(e) => {
            out.push_str("panic(");
            fmt_expr(out, e, src, depth);
            out.push_str(")\n");
        }
        StmtKind::Expr(e) => {
            fmt_expr(out, e, src, depth);
            out.push('\n');
        }
        StmtKind::If(i) => {
            fmt_if_expr_stmt(out, i, src, depth);
        }
        StmtKind::While(w) => {
            out.push_str("while ");
            fmt_expr(out, &w.cond, src, depth);
            out.push_str(" {\n");
            fmt_block_stmts(out, &w.body, src, depth + 1);
            indent(out, depth);
            out.push_str("}\n");
        }
        StmtKind::For(f) => {
            out.push_str("for ");
            if let Some(idx) = &f.index_var {
                out.push_str(&idx.name);
                out.push_str(", ");
            }
            out.push_str(&f.var.name);
            out.push_str(" in ");
            fmt_expr(out, &f.iter, src, depth);
            out.push_str(" {\n");
            fmt_block_stmts(out, &f.body, src, depth + 1);
            indent(out, depth);
            out.push_str("}\n");
        }
        StmtKind::Match(m) => {
            out.push_str("match ");
            fmt_expr(out, &m.scrutinee, src, depth);
            out.push_str(" {\n");
            for arm in &m.arms {
                indent(out, depth + 1);
                match &arm.pattern {
                    haki_ast::MatchPattern::Ident(id) => out.push_str(&id.name),
                    haki_ast::MatchPattern::Int(n)    => out.push_str(&n.to_string()),
                    haki_ast::MatchPattern::String(s) => { out.push('"'); out.push_str(s); out.push('"'); }
                }
                if !arm.bindings.is_empty() {
                    out.push('(');
                    for (i, b) in arm.bindings.iter().enumerate() {
                        if i > 0 { out.push_str(", "); }
                        out.push_str(&b.name);
                    }
                    out.push(')');
                }
                out.push_str(" {\n");
                fmt_block_stmts(out, &arm.body, src, depth + 2);
                indent(out, depth + 1);
                out.push_str("}\n");
            }
            indent(out, depth);
            out.push_str("}\n");
        }
        StmtKind::Select(_) => { out.push_str("select { /* ... */ }\n"); }
    }
}

fn fmt_if_expr_stmt(out: &mut String, i: &haki_ast::IfExpr, src: &str, depth: usize) {
    out.push_str("if ");
    fmt_expr(out, &i.cond, src, depth);
    out.push_str(" {\n");
    fmt_block_stmts(out, &i.then_block, src, depth + 1);
    indent(out, depth);
    out.push('}');
    if let Some(els) = &i.else_branch {
        match els {
            haki_ast::ElseBranch::Block(b) => {
                out.push_str(" else {\n");
                fmt_block_stmts(out, b, src, depth + 1);
                indent(out, depth);
                out.push('}');
            }
            haki_ast::ElseBranch::If(inner) => {
                out.push_str(" else ");
                fmt_if_expr_stmt(out, inner, src, depth);
            }
        }
    }
    out.push('\n');
}

fn fmt_expr(out: &mut String, expr: &haki_ast::Expr, src: &str, depth: usize) {
    use haki_ast::ExprKind;
    match &expr.kind {
        ExprKind::Int(n)    => out.push_str(&n.to_string()),
        ExprKind::Float(f)  => out.push_str(&f.to_string()),
        ExprKind::Bool(b)   => out.push_str(if *b { "true" } else { "false" }),
        ExprKind::Null      => out.push_str("null"),
        ExprKind::String(s) => { out.push('"'); out.push_str(s); out.push('"'); }
        ExprKind::Ident(id) => out.push_str(&id.name),
        ExprKind::Unary(op, e) => {
            out.push_str(match op { haki_ast::UnaryOp::Neg => "-", haki_ast::UnaryOp::Not => "!" });
            fmt_expr(out, e, src, depth);
        }
        ExprKind::Binary(op, l, r) => {
            fmt_expr(out, l, src, depth);
            out.push(' ');
            out.push_str(fmt_binop(op));
            out.push(' ');
            fmt_expr(out, r, src, depth);
        }
        ExprKind::Field(recv, field) => {
            fmt_expr(out, recv, src, depth);
            out.push('.');
            out.push_str(&field.name);
        }
        ExprKind::MethodCall(recv, method, args) => {
            fmt_expr(out, recv, src, depth);
            out.push('.');
            out.push_str(&method.name);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_expr(out, a, src, depth);
            }
            out.push(')');
        }
        ExprKind::Call(callee, args) => {
            fmt_expr(out, callee, src, depth);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_expr(out, a, src, depth);
            }
            out.push(')');
        }
        ExprKind::NamedCall(callee, args) => {
            fmt_expr(out, callee, src, depth);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(&a.name.name);
                out.push_str(": ");
                fmt_expr(out, &a.value, src, depth);
            }
            out.push(')');
        }
        ExprKind::Index(arr, idx) => {
            fmt_expr(out, arr, src, depth);
            out.push('[');
            fmt_expr(out, idx, src, depth);
            out.push(']');
        }
        ExprKind::Array(elems) => {
            out.push('[');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                fmt_expr(out, e, src, depth);
            }
            out.push(']');
        }
        ExprKind::Assign(target, val) => {
            fmt_expr(out, target, src, depth);
            out.push_str(" = ");
            fmt_expr(out, val, src, depth);
        }
        ExprKind::Async(e) => {
            out.push_str("async ");
            fmt_expr(out, e, src, depth);
        }
        ExprKind::If(i) => {
            // If used as an expression (with yield)
            out.push_str("if ");
            fmt_expr(out, &i.cond, src, depth);
            out.push_str(" {\n");
            fmt_block_stmts(out, &i.then_block, src, depth + 1);
            indent(out, depth);
            out.push('}');
            if let Some(els) = &i.else_branch {
                match els {
                    haki_ast::ElseBranch::Block(b) => {
                        out.push_str(" else {\n");
                        fmt_block_stmts(out, b, src, depth + 1);
                        indent(out, depth);
                        out.push('}');
                    }
                    haki_ast::ElseBranch::If(inner) => {
                        out.push_str(" else if ");
                        fmt_expr(out, &inner.cond, src, depth);
                        out.push_str(" {\n");
                        fmt_block_stmts(out, &inner.then_block, src, depth + 1);
                        indent(out, depth);
                        out.push('}');
                    }
                }
            }
        }
        ExprKind::Block(b) => {
            out.push_str("{\n");
            fmt_block_stmts(out, b, src, depth + 1);
            indent(out, depth);
            out.push('}');
        }
        ExprKind::Match(m) => {
            out.push_str("match ");
            fmt_expr(out, &m.scrutinee, src, depth);
            out.push_str(" {\n");
            for arm in &m.arms {
                indent(out, depth + 1);
                match &arm.pattern {
                    haki_ast::MatchPattern::Ident(id) => out.push_str(&id.name),
                    haki_ast::MatchPattern::Int(n)    => out.push_str(&n.to_string()),
                    haki_ast::MatchPattern::String(s) => { out.push('"'); out.push_str(s); out.push('"'); }
                }
                if !arm.bindings.is_empty() {
                    out.push('(');
                    for (i, b) in arm.bindings.iter().enumerate() {
                        if i > 0 { out.push_str(", "); }
                        out.push_str(&b.name);
                    }
                    out.push(')');
                }
                out.push_str(" {\n");
                fmt_block_stmts(out, &arm.body, src, depth + 2);
                indent(out, depth + 1);
                out.push_str("}\n");
            }
            indent(out, depth);
            out.push('}');
        }
        ExprKind::FnLiteral { captures, params, return_ty, body } => {
            out.push_str("fn");
            if !captures.is_empty() {
                out.push('[');
                for (i, c) in captures.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    if c.weak { out.push_str("weak "); }
                    out.push_str(&c.name.name);
                }
                out.push(']');
            }
            out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(&p.name.name);
                out.push_str(": ");
                fmt_ty(out, &p.ty);
            }
            out.push(')');
            if let Some(ret) = return_ty {
                out.push_str(" -> ");
                fmt_return_ty(out, ret);
            }
            out.push_str(" {\n");
            fmt_block_stmts(out, body, src, depth + 1);
            indent(out, depth);
            out.push('}');
        }
    }
}

fn fmt_binop(op: &haki_ast::BinaryOp) -> &'static str {
    use haki_ast::BinaryOp;
    match op {
        BinaryOp::Add => "+",  BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",  BinaryOp::Div => "/",  BinaryOp::Mod => "%",
        BinaryOp::Eq  => "==", BinaryOp::Ne  => "!=",
        BinaryOp::Lt  => "<",  BinaryOp::Le  => "<=",
        BinaryOp::Gt  => ">",  BinaryOp::Ge  => ">=",
        BinaryOp::And => "&&", BinaryOp::Or  => "||",
    }
}

// ── Test runner (hakic test) ──────────────────────────────────────────────────

/// Discover test_* functions, generate a harness, compile and run it.
fn run_tests(source: &Path, quiet: bool) {
    let src = match fs::read_to_string(source) {
        Ok(s) => s,
        Err(e) => { eprintln!("hakic: cannot read '{}': {e}", source.display()); process::exit(1); }
    };

    // Parse to find test function names.
    let ast = match haki_parser::parse(&src) {
        Ok(a) => a,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };

    let test_fns: Vec<String> = ast.items.iter().filter_map(|item| {
        if let haki_ast::ItemKind::Fn(f) = &item.kind {
            if f.name.name.starts_with("test_") && f.params.is_empty() {
                return Some(f.name.name.clone());
            }
        }
        None
    }).collect();

    if test_fns.is_empty() {
        println!("no test functions found (expected fn test_*() with no parameters)");
        return;
    }

    if !quiet { eprintln!("found {} test(s)", test_fns.len()); }

    // Build a harness source that calls each test and tracks pass/fail.
    // The harness calls each test_* function wrapped in a print of its name.
    // Tests signal failure by calling panic(); pass is implicit (no panic).
    // We run them one at a time by compiling per-test; simpler and gives
    // clean isolation. For v0.8, sequential is fine.
    let mut passed = 0usize;
    let mut failed = 0usize;

    for test_name in &test_fns {
        // Build a harness: include original source WITHOUT the main() function,
        // then append a synthetic main() that calls only the test function.
        // We strip the original main() by filtering it from the AST item list
        // using source spans — simpler: just emit all non-main top-level items.
        let src_no_main = {
            let mut out = String::new();
            for item in &ast.items {
                if let haki_ast::ItemKind::Fn(f) = &item.kind {
                    if f.name.name == "main" { continue; }
                }
                // Re-emit by slicing the source at the item span.
                // Fallback: just include the whole source and accept duplicate main.
                // A cleaner approach: reparse and pretty-print — but for now we use
                // a simple line-based exclusion: strip fn main() { ... } blocks.
                let _ = item;
            }
            // Simpler approach: use the formatter on a filtered AST.
            // For now: include all source and rely on the fact that the test
            // harness's main() definition will shadow the original at link time.
            // The REAL fix: filter via spans.
            let lines: Vec<&str> = src.lines().collect();
            let mut in_main = false;
            let mut brace_depth = 0i32;
            for line in &lines {
                let trimmed = line.trim();
                if !in_main && (trimmed.starts_with("fn main()") || trimmed.starts_with("fn main(") && trimmed.contains("main()")) {
                    in_main = true;
                    brace_depth = 0;
                }
                if in_main {
                    for ch in line.chars() {
                        if ch == '{' { brace_depth += 1; }
                        if ch == '}' { brace_depth -= 1; }
                    }
                    if brace_depth <= 0 && line.contains('}') {
                        in_main = false;
                    }
                    continue;
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        };

        let harness = format!(
            "{src_no_main}\n\nfn main() {{\n    {test_name}()\n}}\n"
        );

        // Write harness to temp file.
        let tmp_dir  = std::env::temp_dir().join(format!("hakic_test_{test_name}"));
        let _ = fs::create_dir_all(&tmp_dir);
        let harness_path = tmp_dir.join("harness.haki");
        let binary_path  = tmp_dir.join("test_bin");
        if fs::write(&harness_path, &harness).is_err() { continue; }

        // Compile harness using the same pipeline as compile_and_run.
        let _compile_args = RunArgs {
            source:       harness_path.clone(),
            output:       Some(binary_path.clone()),
            emit_ir:      false,
            emit_runtime: false,
            emit_wasm:    false,
            emit_c:       false,
            quiet:        true,
            run:          false,
            run_args:     vec![],
            target_so:    false,
            target_gtk:   false,
        };

        // Capture compile errors.
        let compile_result = std::panic::catch_unwind(|| {
            // We can't easily capture process::exit — compile in a subprocess instead.
        });
        let _ = compile_result;

        // Compile as subprocess so a failing test doesn't kill the runner.
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hakic"));
        let compile_out = std::process::Command::new(&exe)
            .arg(harness_path.to_str().unwrap())
            .arg("-o").arg(binary_path.to_str().unwrap())
            .arg("--quiet")
            .output();

        match compile_out {
            Err(e) => {
                eprintln!("  FAIL  {test_name} — compile error: {e}");
                failed += 1;
                continue;
            }
            Ok(out) if !out.status.success() => {
                let err = String::from_utf8_lossy(&out.stderr);
                eprintln!("  FAIL  {test_name} — {}", err.trim());
                failed += 1;
                continue;
            }
            _ => {}
        }

        // Run the test binary.
        let run_out = std::process::Command::new(&binary_path).output();
        match run_out {
            Ok(out) if out.status.success() => {
                println!("  pass  {test_name}");
                passed += 1;
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                let detail = if !stderr.is_empty() { stderr.trim() } else { stdout.trim() };
                eprintln!("  FAIL  {test_name} — {detail}");
                failed += 1;
            }
            Err(e) => {
                eprintln!("  FAIL  {test_name} — run error: {e}");
                failed += 1;
            }
        }
    }

    println!();
    println!("{} passed, {} failed", passed, failed);
    if failed > 0 { process::exit(1); }
}

// ── Check-only (hakic check) ──────────────────────────────────────────────────

/// Run lex → parse → module resolution → typeck, then stop.
/// Prints a clean success or error message and exits.
fn check_only(source: &Path, quiet: bool) {
    let src = match fs::read_to_string(source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hakic: cannot read '{}': {e}", source.display());
            process::exit(1);
        }
    };

    // Lex
    let tokens = match haki_lexer::lex(&src) {
        Ok(t) => t,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };
    if !quiet { eprintln!("[lex]     {} tokens", tokens.len()); }

    // Parse
    let ast = match haki_parser::parse(&src) {
        Ok(a) => a,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };
    if !quiet { eprintln!("[parse]   {} items", ast.items.len()); }

    // Module resolution
    let source_dir = source.parent().unwrap_or(Path::new("."));
    let (mut merged_ast, module_registry) = match resolve_modules(&ast, source_dir) {
        Ok(r) => r,
        Err(e) => { eprintln!("hakic: {e}"); process::exit(1); }
    };
    for item in ast.items.iter() {
        if !matches!(&item.kind, haki_ast::ItemKind::Import { .. }) {
            merged_ast.items.push(item.clone());
        }
    }

    // Typecheck
    let mut sym = haki_typeck::SymbolTable::new();
    haki_stdlib::register_builtins(&mut sym);
    sym.modules = module_registry.clone();
    for mod_syms in module_registry.values() {
        for (name, ed) in &mod_syms.enum_defs {
            sym.enum_defs.insert(name.clone(), ed.clone());
        }
    }
    match haki_typeck::typecheck_with_sym(&merged_ast, sym) {
        Ok(typed) => {
            if !quiet {
                eprintln!("[typeck]  {} items", typed.items.len());
            }
            println!("✓  {} — ok", source.display());
            // Exit 0 — success
        }
        Err(e) => {
            eprintln!("hakic: {}", format_error(&e, &src));
            process::exit(1);
        }
    }
}

// ── Main compile + optional run ───────────────────────────────────────────────

// ── Watch mode ──────────────────────────────────────────────────────────────

fn watch_mode(source: &Path, run_args: Vec<String>) {
    use std::time::{Duration, SystemTime};
    use std::collections::HashMap;

    let source = source.canonicalize().unwrap_or_else(|_| source.to_path_buf());
    let watch_dir = source.parent().unwrap_or(Path::new(".")).to_path_buf();
    eprintln!("[watch] {} (100ms polling — Ctrl+C to stop)", source.display());

    let collect_mtimes = |dir: &Path| -> HashMap<PathBuf, SystemTime> {
        let mut map = HashMap::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("haki") {
                    if let Ok(m) = fs::metadata(&p) {
                        if let Ok(t) = m.modified() { map.insert(p, t); }
                    }
                }
            }
        }
        map
    };

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hakic"));

    let spawn = |exe: &Path, src: &Path, args: &[String]| -> Option<std::process::Child> {
        let ok = std::process::Command::new(exe)
            .arg(src).arg("--quiet")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok { eprintln!("[watch] compile failed — fix and save"); return None; }
        let bin = src.with_extension("");
        match std::process::Command::new(&bin).args(args).spawn() {
            Ok(c) => { eprintln!("[watch] running pid {}", c.id()); Some(c) }
            Err(e) => { eprintln!("[watch] run error: {e}"); None }
        }
    };

    let mut mtimes = collect_mtimes(&watch_dir);
    let mut child: Option<std::process::Child> = spawn(&exe, &source, &run_args);

    loop {
        std::thread::sleep(Duration::from_millis(100));
        let cur = collect_mtimes(&watch_dir);
        let changed = cur.iter().any(|(p,t)| mtimes.get(p) != Some(t))
            || mtimes.keys().any(|p| !cur.contains_key(p));

        if changed {
            mtimes = cur;
            eprintln!("[watch] change detected — rebuilding...");
            if let Some(mut c) = child.take() { let _ = c.kill(); let _ = c.wait(); }
            child = spawn(&exe, &source, &run_args);
        }

        if let Some(ref mut c) = child {
            if let Ok(Some(s)) = c.try_wait() {
                eprintln!("[watch] process exited ({s}) — waiting for changes");
                child = None;
            }
        }
    }
}

// ── REPL ────────────────────────────────────────────────────────────────────

const REPL_SENTINEL: &str = "__HAKI_REPL_7f3a9b__";

fn run_repl() {
    use std::io::{self, BufRead, Write};

    eprintln!("Haki REPL  —  :help for commands, :quit to exit");
    let exe  = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hakic"));
    let tmp  = std::env::temp_dir().join("hakic_repl");
    let _    = fs::create_dir_all(&tmp);
    let src_path = tmp.join("session.haki");
    let bin_path = tmp.join("session_bin");

    let mut decls: Vec<String> = vec![];
    let mut stmts: Vec<String> = vec![];
    let mut n = 0usize;

    loop {
        {
            let mut out = io::stdout();
            if n == 0 { let _ = write!(out, "haki> "); }
            else       { let _ = write!(out, "  {}> ", n + 1); }
            let _ = out.flush();
        }

        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 { break; }
        let line = line.trim_end().to_string();
        if line.is_empty() { continue; }

        match line.trim() {
            ":quit"|":q" => { eprintln!("bye."); break; }
            ":clear"|":c" => { decls.clear(); stmts.clear(); n = 0; eprintln!("[repl] cleared"); continue; }
            ":show"|":s" => {
                for d in &decls { eprintln!("{d}"); }
                if !stmts.is_empty() {
                    eprintln!("fn main() {{");
                    for s in &stmts { eprintln!("    {s}"); }
                    eprintln!("}}");
                }
                continue;
            }
            ":help"|":h" => {
                eprintln!(":quit  :clear  :show  :help");
                eprintln!("fn/class/import/struct/enum — top-level (persist)");
                eprintln!("everything else — statement (runs in main)");
                continue;
            }
            _ => {}
        }

        // Classify: top-level declaration or executable statement?
        let t = line.trim();
        let is_decl = t.starts_with("fn ")
            || t.starts_with("class ")  || t.starts_with("struct ")
            || t.starts_with("enum ")   || t.starts_with("import ")
            || t.starts_with("protocol ");

        // Build session source
        let mut src = String::new();
        let mut nd = decls.clone();
        let mut ns = stmts.clone();
        if is_decl { nd.push(line.clone()); } else { ns.push(line.clone()); }

        for d in &nd { src.push_str(d); src.push('\n'); }
        src.push_str("\nfn main() {\n");
        // Replay prior stmts to rebuild state
        for s in &stmts { src.push_str("    "); src.push_str(s); src.push('\n'); }
        // Sentinel marks start of new output
        src.push_str(&format!("    print(\"{}\")\n", REPL_SENTINEL));
        if !is_decl { src.push_str("    "); src.push_str(&line); src.push('\n'); }
        src.push_str("}\n");

        if fs::write(&src_path, &src).is_err() { continue; }

        // Compile
        let cc = std::process::Command::new(&exe)
            .arg(&src_path).arg("-o").arg(&bin_path).arg("--quiet")
            .output();
        match cc {
            Ok(o) if !o.status.success() => {
                let e = String::from_utf8_lossy(&o.stderr);
                let clean = e.lines()
                    .map(|l| l.replace(src_path.to_str().unwrap_or(""), "<input>"))
                    .collect::<Vec<_>>().join("\n");
                eprintln!("{clean}");
                continue;
            }
            Err(e) => { eprintln!("[repl] {e}"); continue; }
            Ok(_)  => {}
        }

        // Run, capture output, print delta after sentinel
        match std::process::Command::new(&bin_path).output() {
            Err(e) => { eprintln!("[repl] {e}"); continue; }
            Ok(o)  => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if let Some(delta) = stdout.split(REPL_SENTINEL).nth(1) {
                    let out = delta.trim_start_matches('\n');
                    if !out.is_empty() {
                        print!("{out}");
                        if !out.ends_with('\n') { println!(); }
                    }
                }
                if !o.status.success() {
                    let e = String::from_utf8_lossy(&o.stderr);
                    if !e.is_empty() { eprintln!("{e}"); }
                    continue;
                }
            }
        }

        // Commit to state
        if is_decl { decls = nd; } else { stmts = ns; }
        n += 1;
    }
}


/// Post-process emitted C to replace `/* haki span:N */` comments with
/// real `#line L "file"` directives. This gives debuggers (lldb/gdb)
/// source-level mapping back to the original .haki file.
///
/// The mapping: we have the raw source string, so we convert byte offsets
/// back to line numbers here in the driver where both are available.
fn inject_line_directives(c_src: &str, haki_src: &str, haki_path: &str) -> String {
    // Build a byte-offset → line-number lookup for the Haki source
    let mut line_map: Vec<usize> = vec![1]; // byte 0 is line 1
    let mut line = 1usize;
    for (i, b) in haki_src.bytes().enumerate() {
        if b == b'\n' { line += 1; }
        line_map.push(line);
    }
    let offset_to_line = |off: usize| -> usize {
        *line_map.get(off.min(line_map.len() - 1)).unwrap_or(&1)
    };

    // Escape path for C string literal
    let escaped_path = haki_path.replace('\\', "\\\\");

    let mut out = String::with_capacity(c_src.len() + 1024);
    for raw_line in c_src.lines() {
        // Match: `/* haki span:N */`
        if let Some(rest) = raw_line.trim().strip_prefix("/* haki span:") {
            if let Some(end) = rest.find(" */") {
                if let Ok(offset) = rest[..end].parse::<u32>() {
                    let haki_line = offset_to_line(offset as usize);
                    out.push_str(&format!(
                        "#line {haki_line} \"{escaped_path}\"\n"
                    ));
                    continue;
                }
            }
        }
        out.push_str(raw_line);
        out.push('\n');
    }
    out
}


fn compile_and_run(args: RunArgs) {
    let stem   = args.source.file_stem().unwrap_or_default().to_string_lossy().to_string();

    // For `hakic run`, compile into a temp directory so we don't litter
    // the source directory with .ll / .o files.
    let work_dir: PathBuf = if args.run {
        let tmp = std::env::temp_dir().join(format!("hakic_{stem}_{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap_or_else(|e| {
            eprintln!("hakic: cannot create temp dir: {e}"); process::exit(1);
        });
        tmp
    } else {
        args.source.parent().unwrap_or(Path::new(".")).to_path_buf()
    };

    let ir_path      = work_dir.join(format!("{stem}.ll"));
    let obj_path     = work_dir.join(format!("{stem}.o"));
    let runtime_c    = work_dir.join("haki_runtime.c");
    let runtime_obj  = work_dir.join("haki_runtime.o");
    let binary_path  = args.output.clone().unwrap_or_else(|| work_dir.join(&stem));

    let quiet = args.quiet;
    macro_rules! log {
        ($($t:tt)*) => { if !quiet { eprintln!($($t)*); } }
    }

    // ── Windows: always use --emit-c (no LLVM backend compiled in) ───────────
    #[cfg(target_os = "windows")]
    {
        if !args.emit_wasm {
            let c_args = RunArgs {
                source:       args.source.clone(),
                output:       args.output.clone(),
                emit_ir:      false,
                emit_runtime: args.emit_runtime,
                emit_wasm:    false,
                emit_c:       true,
                quiet:        args.quiet,
                run:          args.run,
                run_args:     args.run_args.clone(),
                target_so:    false,
                target_gtk:   false,
            };
            compile_and_run(c_args);
            return;
        }
    }

    // ── Read source ───────────────────────────────────────────────────────
    let src = match fs::read_to_string(&args.source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hakic: cannot read '{}': {e}", args.source.display());
            process::exit(1);
        }
    };

    // ── Lex ───────────────────────────────────────────────────────────────
    let tokens = match haki_lexer::lex(&src) {
        Ok(t) => t,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };
    log!("[lex]     {} tokens", tokens.len());

    // ── Parse ─────────────────────────────────────────────────────────────
    let ast = match haki_parser::parse(&src) {
        Ok(a) => a,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };
    log!("[parse]   {} items", ast.items.len());

    // ── Module resolution ────────────────────────────────────────────────
    // Collect imports, resolve paths, topo-sort with cycle detection,
    // rename all exported symbols with alias__ prefix, then merge into
    // the main AST before typechecking.
    let source_dir = args.source.parent().unwrap_or(Path::new("."));
    let (mut merged_ast, module_registry) = match resolve_modules(&ast, source_dir) {
        Ok(r) => r,
        Err(e) => { eprintln!("hakic: {e}"); process::exit(1); }
    };
    // Append main file's items after all imported items so forward refs work.
    for item in ast.items.iter() {
        // Skip Import nodes — they're already processed.
        if !matches!(&item.kind, haki_ast::ItemKind::Import { .. }) {
            merged_ast.items.push(item.clone());
        }
    }

    // ── Typeck ────────────────────────────────────────────────────────────
    let mut sym = haki_typeck::SymbolTable::new();
    haki_stdlib::register_builtins(&mut sym);
    // Register the module symbol tables so the typechecker can resolve
    // qualified access like `math.add(1, 2)`.
    sym.modules = module_registry.clone();
    // Also merge module enum defs into sym.enum_defs so variant lookup works.
    for mod_syms in module_registry.values() {
        for (name, ed) in &mod_syms.enum_defs {
            sym.enum_defs.insert(name.clone(), ed.clone());
        }
    }
    let typed = match haki_typeck::typecheck_with_sym(&merged_ast, sym) {
        Ok(t) => t,
        Err(e) => { eprintln!("hakic: {}", format_error(&e, &src)); process::exit(1); }
    };
    log!("[typeck]  {} items", typed.items.len());

    // ── Mono ──────────────────────────────────────────────────────────────
    let mono = match haki_mono::monomorphize(&typed) {
        Ok(m) => m,
        Err(e) => { eprintln!("hakic: {e}"); process::exit(1); }
    };
    log!("[mono]    {} fns, {} structs, {} classes",
        mono.fns.len(), mono.structs.len(), mono.classes.len());

    // ── Wasm: short-circuit before LLVM codegen ───────────────────────────
    // Wasm operates directly on MonoProgram — no LLVM IR needed.
    // extern "js" fns become Wasm imports; LLVM never needs to see them.
    if args.emit_wasm {
        let wasm_path = args.output.clone().unwrap_or_else(||
            args.source.parent().unwrap_or(Path::new("."))
                .join(format!("{stem}.wasm"))
        );
        match haki_wasm::emit_wasm(&mono, &stem) {
            Ok(bytes) => {
                if let Err(e) = fs::write(&wasm_path, &bytes) {
                    eprintln!("hakic: cannot write wasm: {e}"); process::exit(1);
                }
                if !quiet { eprintln!("[wasm]    {} ({} bytes)", wasm_path.display(), bytes.len()); }
            }
            Err(e) => { eprintln!("hakic: wasm error: {e}"); process::exit(1); }
        }
        return;
    }

    // ── Codegen (LLVM — not compiled on Windows) ─────────────────────────────
    // Skip LLVM entirely when --emit-c is set — the C emitter handles output directly.
    #[cfg(not(target_os = "windows"))]
    let ir = if args.emit_c {
        String::new() // placeholder — not used when emit_c is set
    } else {
        match haki_codegen::emit_ir(&mono, &stem) {
            Ok(ir) => ir,
            Err(e) => { eprintln!("hakic: {e}"); process::exit(1); }
        }
    };
    #[cfg(not(target_os = "windows"))]
    if !args.emit_c { log!("[codegen] {} bytes of IR", ir.len()); }

    // ── Write IR (non-Windows only) ───────────────────────────────────────────
    #[cfg(not(target_os = "windows"))]
    {
        if let Err(e) = fs::write(&ir_path, &ir) {
            eprintln!("hakic: cannot write IR: {e}"); process::exit(1);
        }

        if args.emit_ir {
            if args.run {
                let dest = args.source.parent().unwrap_or(Path::new("."))
                    .join(format!("{stem}.ll"));
                let _ = fs::copy(&ir_path, &dest);
                eprintln!("{}", dest.display());
            } else {
                eprintln!("{}", ir_path.display());
            }
            return;
        }
    }

    // ── Optional: emit C source ───────────────────────────────────────────
    if args.emit_c {
        let c_result = if args.target_so {
            haki_cemit::emit_c_so(&mono)
        } else {
            haki_cemit::emit_c(&mono, Some(args.source.to_str().unwrap_or("")))
        };
        match c_result {
            Ok(c_src) => {
                let c_path = work_dir.join(format!("{stem}.c"));
                if let Err(e) = fs::write(&c_path, &c_src) {
                    eprintln!("hakic: cannot write C source: {e}"); process::exit(1);
                }
                let out_path = if args.target_so {
                    args.output.clone().unwrap_or_else(|| {
                        args.source.parent().unwrap_or(Path::new(".")).join(format!("{stem}.so"))
                    })
                } else {
                    args.output.clone().unwrap_or_else(|| {
                        if args.run { work_dir.join(&stem) }
                        else { args.source.parent().unwrap_or(Path::new(".")).join(&stem) }
                    })
                };
                // Inject `#line` directives so gcc embeds DWARF pointing to .haki source.
                // Replaces `/* haki span:N */` comments the cemit left as markers.
                let c_src = inject_line_directives(
                    &c_src, &src, args.source.to_str().unwrap_or("")
                );
                if !quiet { eprintln!("[c-emit]  {} ({} bytes)", c_path.display(), c_src.len()); }
                let is_so = args.target_so;

                // Collect @link("libname") attributes from extern "c" declarations.
                // These become -llib flags passed automatically to the linker.
                let mut link_libs: Vec<String> = mono.extern_fns.iter()
                    .filter(|ef| ef.abi == "c")
                    .flat_map(|ef| ef.attributes.iter())
                    .filter(|attr| attr.name == "link")
                    .flat_map(|attr| attr.args.iter())
                    .map(|lib| format!("-l{lib}"))
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                link_libs.sort(); // deterministic order
                if !quiet && !link_libs.is_empty() {
                    eprintln!("[link]    libs: {}", link_libs.join(" "));
                }

                // Pass -g (DWARF debug info) when --debug flag is set.
                // Combined with #line directives, gives source-level debugging in lldb/gdb.
                let debug_flag = args.emit_c; // reuse emit_c; in v2.8 add proper --debug flag
                let base_flags: Vec<&str> = if is_so {
                    vec!["-std=gnu11", "-O2", "-shared", "-fPIC", "-lpthread", "-lm"]
                } else if debug_flag {
                    vec!["-std=gnu11", "-g", "-O0", "-lpthread", "-lm"]
                } else {
                    vec!["-std=gnu11", "-O2", "-lpthread", "-lm"]
                };

                // Write the GTK UI runtime C file alongside the user C if targeting GTK
                // When targeting GTK, prepend forward declarations for all GTK platform
                // functions so the user C compiles before the GTK runtime C is linked.
                if args.target_gtk {
                    let decls = concat!(
                        "#include <stdint.h>
",
                        "void haki_app_run(const char* json, const char* title, long long width, long long height);
",
                        "long long haki_gtk_create_window(const char* title, long long width, long long height);
",
                        "long long haki_gtk_create_label(long long parent_id, const char* text);
",
                        "long long haki_gtk_create_button(long long parent_id, const char* label, long long node_id);
",
                        "long long haki_gtk_create_box(long long parent_id, long long horizontal);
",
                        "void haki_gtk_set_text(long long node_id, const char* text);
",
                        "void haki_gtk_set_visible(long long node_id, long long visible);
",
                        "void haki_gtk_insert_child(long long parent_id, long long index, long long child_id);
",
                        "void haki_gtk_remove_child(long long node_id);
",
                        "void haki_platform_run(void);
",
                        "void haki_set_callback_dispatcher(void* fn);
",
                        "long long haki_gtk_alloc_node_id(void);
",
                        "void haki_register_callback(long long node_id, void* fn);
",
                        "void haki_fire_callback(long long node_id);
",
                        "void haki_set_rerender_callback(long long label_id, void* closure);
",
                        "long long haki_gtk_peek_next_id(void);
",
                        "void haki_gtk_mark_label(long long node_id);
",
                        "long long haki_gtk_get_label_id(void);
",
                    );
                    let mut patched = decls.to_string();
                    patched.push_str(&c_src);
                    let _ = fs::write(&c_path, &patched);
                }
                let gtk_runtime_path = if args.target_gtk {
                    let p = work_dir.join("haki_ui_gtk_runtime.c");
                    let _ = fs::write(&p, haki_stdlib::UI_RUNTIME_C_SOURCE);
                    Some(p)
                } else { None };

                // macOS GTK include paths (from Homebrew)
                // Use pkg-config to find GTK includes on macOS (handles Homebrew arm64/x86_64)
                // Resolve GTK include paths using brew --prefix for each package
                // pkg-config is not always installed; brew --prefix is always available
                let brew_gtk    = std::process::Command::new("brew").args(["--prefix","gtk+3"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/gtk+3".to_string());
                let brew_glib   = std::process::Command::new("brew").args(["--prefix","glib"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/glib".to_string());
                let brew_pango  = std::process::Command::new("brew").args(["--prefix","pango"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/pango".to_string());
                let brew_cairo  = std::process::Command::new("brew").args(["--prefix","cairo"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/cairo".to_string());
                let brew_gdk_pb = std::process::Command::new("brew").args(["--prefix","gdk-pixbuf"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/gdk-pixbuf".to_string());
                let brew_atk    = std::process::Command::new("brew").args(["--prefix","atk"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/atk".to_string());
                let brew_hb     = std::process::Command::new("brew").args(["--prefix","harfbuzz"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/harfbuzz".to_string());

                let gtk_includes_macos_owned: Vec<String> = vec![
                    format!("-I{brew_gtk}/include/gtk-3.0"),
                    format!("-I{brew_glib}/include/glib-2.0"),
                    format!("-I{brew_glib}/lib/glib-2.0/include"),
                    format!("-I{brew_pango}/include/pango-1.0"),
                    format!("-I{brew_hb}/include/harfbuzz"),
                    format!("-I{brew_cairo}/include/cairo"),
                    format!("-I{brew_gdk_pb}/include/gdk-pixbuf-2.0"),
                    format!("-I{brew_atk}/include/atk-1.0"),
                    // gdk headers live inside gtk+3
                    format!("-I{brew_gtk}/include/gdk-3.0"),
                ];
                let gtk_includes_macos: Vec<&str> = gtk_includes_macos_owned.iter().map(|s| s.as_str()).collect();
                // Linux GTK include paths
                let gtk_includes_linux = vec![
                    "-I/usr/include/gtk-3.0",
                    "-I/usr/include/glib-2.0",
                    "-I/usr/lib/x86_64-linux-gnu/glib-2.0/include",
                    "-I/usr/include/pango-1.0",
                    "-I/usr/include/harfbuzz",
                    "-I/usr/include/cairo",
                    "-I/usr/include/gdk-pixbuf-2.0",
                    "-I/usr/include/atk-1.0",
                ];
                let brew_gdk_pb2 = std::process::Command::new("brew").args(["--prefix","gdk-pixbuf"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/gdk-pixbuf".to_string());
                let brew_atk2 = std::process::Command::new("brew").args(["--prefix","at-spi2-core"]).output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "/opt/homebrew/opt/at-spi2-core".to_string());
                let gtk_libs_owned: Vec<String> = vec![
                    format!("-L{brew_gtk}/lib"),
                    format!("-L{brew_glib}/lib"),
                    format!("-L{brew_cairo}/lib"),
                    format!("-L{brew_pango}/lib"),
                    format!("-L{brew_gdk_pb2}/lib"),
                    format!("-L{brew_atk2}/lib"),
                    "-lgtk-3".into(), "-lgdk-3".into(),
                    "-lpangocairo-1.0".into(), "-lpango-1.0".into(),
                    "-lglib-2.0".into(), "-lgobject-2.0".into(),
                    "-lcairo".into(), "-lgdk_pixbuf-2.0".into(),
                    "-latk-1.0".into(), "-lgio-2.0".into(),
                ];
                let gtk_libs: Vec<&str> = gtk_libs_owned.iter().map(|s| s.as_str()).collect();

                let gcc_result = {
                    let mut cmd = std::process::Command::new("gcc");
                    cmd.args(&base_flags)
                       .arg(c_path.to_str().unwrap());
                    if let Some(ref gtk_c) = gtk_runtime_path {
                        // Include GTK paths for the runtime compile
                        if !quiet {
                            eprintln!("[gtk]     includes: {:?}", &gtk_includes_macos_owned);
                        }
                        cmd.args(gtk_includes_macos_owned.iter().map(|s| s.as_str()));
                        cmd.arg(gtk_c.to_str().unwrap());
                        cmd.args(gtk_libs_owned.iter().map(|s| s.as_str()));
                    }
                    cmd.arg("-o").arg(out_path.to_str().unwrap())
                       .args(&link_libs);
                    let status = cmd.status();
                    if status.is_err() {
                        // Fallback to clang
                        let mut cmd2 = std::process::Command::new("clang");
                        cmd2.args(&base_flags)
                            .arg(c_path.to_str().unwrap())
                            .arg("-o").arg(out_path.to_str().unwrap())
                            .args(&link_libs);
                        cmd2.status().or(status)
                    } else {
                        status
                    }
                };
                // MSVC fallback (Windows, no GNU extensions, no -l flags)
                let gcc_result = gcc_result.or_else(|_| {
                    std::process::Command::new("cl.exe")
                        .args(["/O2", "/TC",
                               c_path.to_str().unwrap(),
                               &format!("/Fe:{}", out_path.to_str().unwrap())])
                        .status()
                });
                match gcc_result {
                    Ok(s) if s.success() => {
                        if !quiet { eprintln!("[link]    {}", out_path.display()); }
                    }
                    Ok(s) => {
                        eprintln!("hakic: gcc failed (exit {:?})", s.code());
                        process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("hakic: gcc not found: {e}");
                        process::exit(1);
                    }
                }
                if args.run {
                    let status = std::process::Command::new(&out_path)
                        .args(&args.run_args)
                        .status()
                        .unwrap_or_else(|e| { eprintln!("hakic: exec failed: {e}"); process::exit(1); });
                    process::exit(status.code().unwrap_or(0));
                }
            }
            Err(e) => { eprintln!("hakic: C emit error: {e}"); process::exit(1); }
        }
        return;
    }

    // ── Write runtime ─────────────────────────────────────────────────────
    if let Err(e) = fs::write(&runtime_c, haki_stdlib::RUNTIME_C_SOURCE) {
        eprintln!("hakic: cannot write runtime: {e}"); process::exit(1);
    }

    // ── Everything below requires LLVM — non-Windows only ────────────────────
    #[cfg(not(target_os = "windows"))]
    {

    if args.emit_runtime {
        let dest = args.source.parent().unwrap_or(Path::new("."))
            .join("haki_runtime.c");
        if args.run { let _ = fs::copy(&runtime_c, &dest); eprintln!("{}", dest.display()); }
        else { eprintln!("{}", runtime_c.display()); }
        return;
    }

    // ── .ll → .o via llc (or fall back to --emit-c if llc not found) ────────
    if !tool_exists(&["llc", "llc-17", "llc-18", "llc-16"]) {
        // llc not available — transparently fall back to the C backend.
        // This is the common case for end users who installed hakic via
        // Homebrew or the install script without a full LLVM toolchain.
        if !args.quiet {
            eprintln!("[info]    llc not found, using --emit-c backend");
        }
        let c_args = RunArgs {
            source:       args.source.clone(),
            output:       args.output.clone(),
            emit_ir:      false,
            emit_runtime: false,
            emit_wasm:    false,
            emit_c:       true,
            quiet:        args.quiet,
            run:          args.run,
            run_args:     args.run_args.clone(),
            target_so:    args.target_so,
            target_gtk:   args.target_gtk,
        };
        compile_and_run(c_args);
        return;
    }

    let llc = find_tool(&["llc", "llc-17", "llc-18", "llc-16"]);
    run_step(Command::new(&llc).args([
        "-filetype=obj",
        "--relocation-model=pic",
        ir_path.to_str().unwrap(),
        "-o", obj_path.to_str().unwrap(),
    ]), "llc");
    log!("[llc]     {}", obj_path.display());

    // ── Detect UI usage ───────────────────────────────────────────────────
    // If the IR references haki_app_run, the program uses haki_ui widgets.
    // We detect this from the emitted IR so we don't need a separate flag.
    let uses_ui = ir.contains("haki_app_run") || ir.contains("haki_text_new");

    // ── Compile runtime ───────────────────────────────────────────────────
    let cc = find_tool(&["gcc", "cc", "clang", "clang-17"]);
    run_step(Command::new(&cc).args([
        "-c", runtime_c.to_str().unwrap(),
        "-o", runtime_obj.to_str().unwrap(),
    ]), "compile runtime");

    // ── Compile UI runtime (if needed) ───────────────────────────────────
    let ui_runtime_obj = work_dir.join("haki_ui_runtime.o");
    if uses_ui {
        let ui_runtime_c = work_dir.join("haki_ui_runtime.c");
        if let Err(e) = fs::write(&ui_runtime_c, haki_stdlib::UI_RUNTIME_C_SOURCE) {
            eprintln!("hakic: cannot write UI runtime: {e}"); process::exit(1);
        }
        // GTK 3 include paths — same set needed on all Linux distros.
        let gtk_includes = [
            "-I/usr/include/gtk-3.0",
            "-I/usr/include/glib-2.0",
            "-I/usr/lib/x86_64-linux-gnu/glib-2.0/include",
            "-I/usr/include/pango-1.0",
            "-I/usr/include/harfbuzz",
            "-I/usr/include/cairo",
            "-I/usr/include/gdk-pixbuf-2.0",
            "-I/usr/include/atk-1.0",
        ];
        let mut cmd = Command::new(&cc);
        cmd.arg("-c").arg(ui_runtime_c.to_str().unwrap());
        for inc in &gtk_includes { cmd.arg(inc); }
        cmd.arg("-o").arg(ui_runtime_obj.to_str().unwrap());
        run_step(&mut cmd, "compile UI runtime");
        log!("[cc]      compiled runtime + UI runtime (GTK)");
    } else {
        log!("[cc]      compiled runtime");
    }

    // ── Link ──────────────────────────────────────────────────────────────
    let mut link_cmd = Command::new(&cc);
    link_cmd.arg(obj_path.to_str().unwrap());
    link_cmd.arg(runtime_obj.to_str().unwrap());
    if uses_ui {
        link_cmd.arg(ui_runtime_obj.to_str().unwrap());
        // GTK 3 link libraries
        for lib in &["-lgtk-3", "-lgdk-3", "-lpangocairo-1.0", "-lpango-1.0",
                     "-lglib-2.0", "-lgobject-2.0", "-lcairo",
                     "-lgdk_pixbuf-2.0", "-latk-1.0"] {
            link_cmd.arg(lib);
        }
    }
    link_cmd.args(["-lpthread", "-lmicrohttpd"]);
    link_cmd.args(["-o", binary_path.to_str().unwrap()]);
    run_step(&mut link_cmd, "link");

    if args.run {
        log!("[run]     {}", binary_path.display());
        exec_binary(&binary_path, &args.run_args);
    } else {
        log!("[done]    {}", binary_path.display());
    }

    } // end #[cfg(not(target_os = "windows"))]
}

// ── exec or spawn ─────────────────────────────────────────────────────────────

fn exec_binary(path: &Path, extra_args: &[String]) {
    // On Unix, replace this process entirely so signals and exit codes
    // flow through correctly — `hakic run foo.haki` behaves exactly
    // like running `./foo` directly.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new(path).args(extra_args).exec();
        // exec() only returns on error.
        eprintln!("hakic: exec failed: {err}");
        process::exit(1);
    }
    #[cfg(not(unix))]
    {
        let status = Command::new(path).args(extra_args).status()
            .unwrap_or_else(|e| { eprintln!("hakic: run failed: {e}"); process::exit(1); });
        process::exit(status.code().unwrap_or(1));
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_tool(candidates: &[&str]) -> String {
    for c in candidates {
        if Command::new("which").arg(c).output()
            .map_or(false, |o| o.status.success())
        {
            return c.to_string();
        }
    }
    candidates[0].to_string()
}

fn tool_exists(candidates: &[&str]) -> bool {
    candidates.iter().any(|c| {
        Command::new("which").arg(c).output()
            .map_or(false, |o| o.status.success())
    })
}

fn run_step(cmd: &mut Command, step: &str) {
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("hakic: {step} failed (exit {:?})", s.code());
            process::exit(1);
        }
        Err(e) => {
            eprintln!("hakic: cannot run {step}: {e}");
            process::exit(1);
        }
    }
}
