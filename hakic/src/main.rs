/// hakic — The Haki compiler.
///
/// Usage:
///   hakic <source.haki>                  compile to native binary (same dir)
///   hakic <source.haki> -o <output>      specify output binary path
///   hakic run <source.haki>              compile + execute immediately
///   hakic <source.haki> --emit-ir        write .ll only, do not link
///   hakic <source.haki> --emit-runtime   write haki_runtime.c only
///   hakic <source.haki> --quiet          suppress pipeline progress output
///
/// Pipeline:
///   Source → Lex → Parse → Typeck → Mono → Codegen
///         → .ll → llc → .o + gcc runtime.c → binary

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
                // Rename the variant pattern name if it's a module-level name.
                if names.contains(&arm.pattern.name) {
                    arm.pattern.name = format!("{alias}__{}", arm.pattern.name);
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
                if names.contains(&arm.pattern.name) {
                    arm.pattern.name = format!("{alias}__{}", arm.pattern.name);
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
fn format_error(e: &dyn std::fmt::Display, src: &str) -> String {
    let raw = format!("{e}");
    // Replace `Span { lo: N, hi: M }` with `line:col`
    let mut result = String::new();
    let mut rest = raw.as_str();
    while let Some(idx) = rest.find("Span { lo: ") {
        result.push_str(&rest[..idx]);
        rest = &rest[idx + "Span { lo: ".len()..];
        // Parse lo
        if let Some(comma) = rest.find(", hi: ") {
            if let Ok(lo) = rest[..comma].trim().parse::<u32>() {
                let after_hi = &rest[comma + ", hi: ".len()..];
                if let Some(close) = after_hi.find('}') {
                    if let Ok(_hi) = after_hi[..close].trim().parse::<u32>() {
                        let (line, col) = byte_to_linecol(src, lo);
                        result.push_str(&format!("{line}:{col}"));
                        rest = &after_hi[close + 1..];
                        continue;
                    }
                }
            }
        }
        // Couldn't parse — emit original
        result.push_str("Span { lo: ");
    }
    result.push_str(rest);
    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    // Handle --version and --help before anything else.
    if args[1] == "--version" || args[1] == "-V" {
        println!("hakic 0.7.0 — Haki compiler");
        println!("https://github.com/haki-lang/haki");
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
        };
        compile_and_run(run_args);
        return;
    }

    // Normal compile mode.
    let source = PathBuf::from(&args[1]);
    let mut output:       Option<PathBuf> = None;
    let mut emit_ir       = false;
    let mut emit_runtime  = false;
    let mut emit_wasm     = false;
    let mut emit_c_flag   = false;
    let mut quiet         = false;

    let mut i = 2;
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
            _ => {}
        }
        i += 1;
    }

    compile_and_run(RunArgs { source, output, emit_ir, emit_runtime, emit_wasm, emit_c: emit_c_flag, quiet, run: false, run_args: vec![] });
}

fn print_usage() {
    println!("Haki compiler v0.7.0");
    println!();
    println!("Usage:");
    println!("  hakic <source.haki>              compile to native binary");
    println!("  hakic <source.haki> -o <output>  specify output path");
    println!("  hakic run <source.haki>           compile and run immediately");
    println!("  hakic check <source.haki>         typecheck only, no codegen");
    println!("  hakic test <source.haki>          run test_* functions");
    println!("  hakic fmt <source.haki>           format source in place");
    println!("  hakic fmt <source.haki> --check   check formatting without writing");
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
            let sig = trimmed.split('{').next()?.trim().trim_end_matches(')');
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
                out.push_str(&arm.pattern.name);
                if !arm.bindings.is_empty() {
                    out.push('(');
                    for (i, b) in arm.bindings.iter().enumerate() {
                        if i > 0 { out.push_str(", "); }
                        out.push_str(&b.name);
                    }
                    out.push(')');
                } else if arm.pattern.name != "_" {
                    // Check if there's a legacy single binding (class match)
                }
                out.push_str(" {\n");
                fmt_block_stmts(out, &arm.body, src, depth + 2);
                indent(out, depth + 1);
                out.push_str("}\n");
            }
            indent(out, depth);
            out.push_str("}\n");
        }
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
                out.push_str(&arm.pattern.name);
                if !arm.bindings.is_empty() {
                    out.push('(');
                    for (i, b) in arm.bindings.iter().enumerate() {
                        if i > 0 { out.push_str(", "); }
                        out.push_str(&b.name);
                    }
                    out.push(')');
                } else if arm.pattern.name != "_" {
                    // Check if there's a legacy single binding (class match)
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
        let compile_args = RunArgs {
            source:       harness_path.clone(),
            output:       Some(binary_path.clone()),
            emit_ir:      false,
            emit_runtime: false,
            emit_wasm:    false,
            emit_c:       false,
            quiet:        true,
            run:          false,
            run_args:     vec![],
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

    // ── Codegen ───────────────────────────────────────────────────────────
    let ir = match haki_codegen::emit_ir(&mono, &stem) {
        Ok(ir) => ir,
        Err(e) => { eprintln!("hakic: {e}"); process::exit(1); }
    };
    log!("[codegen] {} bytes of IR", ir.len());

    // ── Write IR ──────────────────────────────────────────────────────────
    if let Err(e) = fs::write(&ir_path, &ir) {
        eprintln!("hakic: cannot write IR: {e}"); process::exit(1);
    }

    if args.emit_ir {
        // Copy .ll to source dir if we're in a temp dir
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

    // ── Optional: emit C source ───────────────────────────────────────────
    if args.emit_c {
        match haki_cemit::emit_c(&mono) {
            Ok(c_src) => {
                let c_path = work_dir.join(format!("{stem}.c"));
                if let Err(e) = fs::write(&c_path, &c_src) {
                    eprintln!("hakic: cannot write C source: {e}"); process::exit(1);
                }
                // Compile C to native binary with gcc
                let out_path = args.output.clone().unwrap_or_else(|| {
                    args.source.parent().unwrap_or(Path::new(".")).join(&stem)
                });
                if !quiet { eprintln!("[c-emit]  {} ({} bytes)", c_path.display(), c_src.len()); }
                // Try gcc first (Linux, macOS, MinGW), then clang, then cl.exe (MSVC)
                let gcc_result = std::process::Command::new("gcc")
                    .args(["-std=gnu11", "-O2", "-lpthread", "-lm",
                           c_path.to_str().unwrap(),
                           "-o", out_path.to_str().unwrap()])
                    .status()
                    .or_else(|_| {
                        // Fallback to clang
                        std::process::Command::new("clang")
                            .args(["-std=gnu11", "-O2",
                                   c_path.to_str().unwrap(),
                                   "-o", out_path.to_str().unwrap()])
                            .status()
                    })
                    .or_else(|_| {
                        // Fallback to MSVC cl.exe (Windows, no GNU extensions)
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

    // ── Optional: emit Wasm binary (.wasm) ────────────────────────────────
    if args.emit_wasm {
        let wasm_path = work_dir.join(format!("{stem}.wasm"));
        match haki_wasm::emit_wasm(&mono, &stem) {
            Ok(bytes) => {
                if let Err(e) = fs::write(&wasm_path, &bytes) {
                    eprintln!("hakic: cannot write wasm: {e}"); process::exit(1);
                }
                // If running from temp dir, copy to source dir.
                let dest = if args.run {
                    let d = args.source.parent().unwrap_or(Path::new("."))
                        .join(format!("{stem}.wasm"));
                    let _ = fs::copy(&wasm_path, &d);
                    d
                } else { wasm_path };
                eprintln!("[wasm]    {} ({} bytes)", dest.display(), bytes.len());
            }
            Err(e) => { eprintln!("hakic: wasm error: {e}"); process::exit(1); }
        }
        return;
    }

    // ── Write runtime ─────────────────────────────────────────────────────
    if let Err(e) = fs::write(&runtime_c, haki_stdlib::RUNTIME_C_SOURCE) {
        eprintln!("hakic: cannot write runtime: {e}"); process::exit(1);
    }

    if args.emit_runtime {
        let dest = args.source.parent().unwrap_or(Path::new("."))
            .join("haki_runtime.c");
        if args.run { let _ = fs::copy(&runtime_c, &dest); eprintln!("{}", dest.display()); }
        else { eprintln!("{}", runtime_c.display()); }
        return;
    }

    // ── .ll → .o via llc ─────────────────────────────────────────────────
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
        // Replace current process with the compiled binary (exec-style).
        // Falls back to Command::status on platforms without exec.
        exec_binary(&binary_path, &args.run_args);
    } else {
        log!("[done]    {}", binary_path.display());
    }
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
