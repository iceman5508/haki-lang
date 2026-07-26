/// lsp.rs — Haki Language Server Protocol implementation.
///
/// Transport: LSP over stdio using Content-Length framed JSON-RPC.
/// Protocol:  Subset of LSP 3.17 sufficient for v1.5 features:
///   - textDocument/didOpen
///   - textDocument/didChange
///   - textDocument/didClose
///   - textDocument/definition
///   - textDocument/hover
///   - textDocument/publishDiagnostics (server→client push)
///
/// No external LSP crates — hand-rolled transport avoids edition2024 dep chain.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::lsp_index::ProjectIndex;

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run the LSP server, reading from stdin and writing to stdout.
/// Blocks until the client sends `exit`.
pub fn run_lsp() {
    let stdin  = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut index = ProjectIndex::new();

    let mut reader = stdin.lock();

    loop {
        // ── Read Content-Length header ────────────────────────────────────
        let mut header_line = String::new();
        let mut content_length: Option<usize> = None;

        loop {
            header_line.clear();
            match reader.read_line(&mut header_line) {
                Ok(0) | Err(_) => return, // EOF or error — exit
                _ => {}
            }
            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break; // blank line ends headers
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
                content_length = rest.trim().parse().ok();
            }
        }

        let length = match content_length {
            Some(n) => n,
            None    => continue, // malformed — skip
        };

        // ── Read body ─────────────────────────────────────────────────────
        let mut body = vec![0u8; length];
        if let Err(_) = io::Read::read_exact(&mut reader, &mut body) {
            return;
        }

        let msg: Value = match serde_json::from_slice(&body) {
            Ok(v)  => v,
            Err(_) => continue,
        };

        // ── Dispatch ──────────────────────────────────────────────────────
        let method = msg["method"].as_str().unwrap_or("");
        let id     = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                let result = initialize_result();
                send_response(&mut out, id, result);
            }
            "initialized" => {
                // Client acknowledged — nothing to do
            }
            "shutdown" => {
                send_response(&mut out, id, Value::Null);
            }
            "exit" => {
                return;
            }
            "textDocument/didOpen" => {
                if let Some(path_str) = uri_to_path(&params["textDocument"]["uri"]) {
                    let text = params["textDocument"]["text"]
                        .as_str().unwrap_or("").to_string();
                    index.update_file(&PathBuf::from(&path_str), text);
                    push_diagnostics(&mut out, &params["textDocument"]["uri"], &index, &path_str);
                }
            }
            "textDocument/didChange" => {
                if let Some(path_str) = uri_to_path(&params["textDocument"]["uri"]) {
                    // Full-document sync: take the last contentChange text
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            let text = last["text"].as_str().unwrap_or("").to_string();
                            index.update_file(&PathBuf::from(&path_str), text);
                            push_diagnostics(&mut out, &params["textDocument"]["uri"], &index, &path_str);
                        }
                    }
                }
            }
            "textDocument/didClose" => {
                // Clear diagnostics on close
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    send_notification(&mut out, "textDocument/publishDiagnostics", json!({
                        "uri": uri,
                        "diagnostics": []
                    }));
                }
            }
            "textDocument/hover" => {
                let result = handle_hover(&params, &index);
                send_response(&mut out, id, result);
            }
            "textDocument/definition" => {
                let result = handle_definition(&params, &index);
                send_response(&mut out, id, result);
            }
            _ => {
                // Unknown method — send null result for requests, ignore notifications
                if id.is_some() {
                    send_response(&mut out, id, Value::Null);
                }
            }
        }
    }
}

// ── Capability negotiation ────────────────────────────────────────────────────

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            // Full document sync — client sends entire file on every change
            "textDocumentSync": 1,
            // Hover support
            "hoverProvider": true,
            // Go-to-definition support
            "definitionProvider": true,
        },
        "serverInfo": {
            "name": "hakic-lsp",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

fn push_diagnostics(
    out: &mut impl Write,
    uri: &Value,
    index: &ProjectIndex,
    path_str: &str,
) {
    let path = PathBuf::from(path_str);
    let state = match index.get(&path) {
        Some(s) => s,
        None    => return,
    };

    let mut diagnostics: Vec<Value> = Vec::new();

    // Parse errors
    for err in &state.parse_errors {
        let span = err.span();
        let (sl, sc) = crate::lsp_index::offset_to_position(&state.text, span.lo);
        let (el, ec) = crate::lsp_index::offset_to_position(&state.text, span.hi);
        diagnostics.push(json!({
            "range": lsp_range(sl, sc, el, ec),
            "severity": 1,  // Error
            "source": "hakic",
            "message": err.to_string(),
        }));
    }

    // Type error (single error from typeck)
    if let Some(ref msg) = state.type_error {
        // Parse "line:col: message" format from the typeck error string
        let (range, message) = parse_type_error_range(msg, &state.text);
        diagnostics.push(json!({
            "range": range,
            "severity": 1,
            "source": "hakic",
            "message": message,
        }));
    }

    send_notification(out, "textDocument/publishDiagnostics", json!({
        "uri": uri,
        "diagnostics": diagnostics,
    }));
}

/// Try to extract line:col from a typeck error message.
/// Our error format is "Span { lo: N, hi: M }: message" — extract lo/hi as byte offsets.
fn parse_type_error_range(msg: &str, text: &str) -> (Value, String) {
    // Try "Span { lo: N, hi: M }: message" format
    if msg.starts_with("Span { lo: ") {
        if let Some(rest) = msg.strip_prefix("Span { lo: ") {
            if let Some((lo_str, rest2)) = rest.split_once(", hi: ") {
                if let Some((hi_str, message)) = rest2.split_once(" }: ") {
                    if let (Ok(lo), Ok(hi)) = (lo_str.parse::<u32>(), hi_str.parse::<u32>()) {
                        let (sl, sc) = crate::lsp_index::offset_to_position(text, lo);
                        let (el, ec) = crate::lsp_index::offset_to_position(text, hi);
                        return (lsp_range(sl, sc, el, ec), message.to_string());
                    }
                }
            }
        }
    }
    // Try plain "lo:hi: message" format
    if let Some((prefix, rest)) = msg.split_once(": ") {
        if let Some((lo_str, hi_str)) = prefix.split_once(':') {
            if let (Ok(lo), Ok(hi)) = (lo_str.parse::<u32>(), hi_str.parse::<u32>()) {
                let (sl, sc) = crate::lsp_index::offset_to_position(text, lo);
                let (el, ec) = crate::lsp_index::offset_to_position(text, hi);
                return (lsp_range(sl, sc, el, ec), rest.to_string());
            }
        }
    }
    (lsp_range(0, 0, 0, 0), msg.to_string())
}

// ── Hover ─────────────────────────────────────────────────────────────────────

fn handle_hover(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p,
        None    => return Value::Null,
    };
    let state = match index.get(&PathBuf::from(&path_str)) {
        Some(s) => s,
        None    => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = crate::lsp_index::position_to_offset(&state.text, line, col);

    // Priority 1: cursor on a function/method name → show full signature
    if let Some(ident) = find_ident_at_offset(&state.text, offset) {
        if let Some(sig) = hover_signature_for_ident(&ident, state) {
            return json!({
                "contents": { "kind": "markdown", "value": sig }
            });
        }
    }

    // Priority 2: expression type from typed AST
    if let Some(typed) = &state.typed {
        if let Some(ty_str) = find_type_at_offset(typed, offset) {
            return json!({
                "contents": { "kind": "markdown", "value": format!("`{}`", ty_str) }
            });
        }
    }

    Value::Null
}

/// Look up a full signature for `name` in the symbol table.
/// Returns a Markdown-formatted string like:
///   `fn add(a: int, b: int) -> int`
///   `struct Point { x: float, y: float }`
fn hover_signature_for_ident(name: &str, state: &crate::lsp_index::FileState) -> Option<String> {
    let sym = state.sym.as_ref()?;

    // Top-level function
    if let Some(fi) = sym.functions.get(name) {
        return Some(format_fn_signature(name, fi));
    }

    // Type (struct/class)
    if let Some(td) = sym.types.get(name) {
        return Some(format_type_signature(td));
    }

    // Enum
    if let Some(ed) = sym.enum_defs.get(name) {
        let variants: Vec<String> = ed.variants.iter()
            .map(|v| {
                if v.fields.is_empty() {
                    v.name.name.clone()
                } else {
                    let fields: Vec<String> = v.fields.iter()
                        .map(|f| format_ty_kind(&f.kind))
                        .collect();
                    format!("{}({})", v.name.name, fields.join(", "))
                }
            })
            .collect();
        return Some(format!("```haki\nenum {} {{\n    {}\n}}\n```",
            ed.name.name.as_str(), variants.join("\n    ")));
    }

    // Protocol
    if let Some(pi) = sym.protocols.get(name) {
        let methods: Vec<String> = pi.methods.iter()
            .map(|m| format!("    {}", format_fn_signature_brief(&m.name, m)))
            .collect();
        return Some(format!("```haki\nprotocol {} {{\n{}\n}}\n```",
            name, methods.join("\n")));
    }

    // Module-qualified: search all loaded modules for this function
    for (alias, mod_syms) in &sym.modules {
        // Check raw name and alias-prefixed name
        let prefixed = format!("{}__{}", alias, name);
        let fi_opt = mod_syms.functions.get(name)
            .or_else(|| mod_syms.functions.get(&prefixed));
        if let Some(fi) = fi_opt {
            return Some(format_fn_signature(&format!("{}.{}", alias, name), fi));
        }
    }

    None
}

/// Format a function signature as a fenced Haki code block.
/// Example: `fn add(a: int, b: int) -> int`
fn format_fn_signature(name: &str, fi: &haki_typeck::collector::FnInfo) -> String {
    let params: Vec<String> = fi.params.iter()
        .map(|p| format!("{}: {}", p.name.name, format_ty_kind(&p.ty.kind)))
        .collect();

    let ret = format_return_ty(&fi.return_ty);
    if ret == "void" {
        format!("```haki\nfn {}({})\n```", name, params.join(", "))
    } else {
        format!("```haki\nfn {}({}) -> {}\n```", name, params.join(", "), ret)
    }
}

/// Same as format_fn_signature but for FnInfo from ProtocolInfo methods.
fn format_fn_signature_brief(name: &str, fi: &haki_typeck::collector::FnInfo) -> String {
    let params: Vec<String> = fi.params.iter()
        .map(|p| format!("{}: {}", p.name.name, format_ty_kind(&p.ty.kind)))
        .collect();
    let ret = format_return_ty(&fi.return_ty);
    if ret == "void" {
        format!("fn {}({})", name, params.join(", "))
    } else {
        format!("fn {}({}) -> {}", name, params.join(", "), ret)
    }
}

/// Format a type definition (struct/class) as a fenced code block.
fn format_type_signature(td: &haki_typeck::collector::TypeDef) -> String {
    use haki_typeck::collector::TypeKind;
    let kw = match td.kind {
        TypeKind::Struct => "struct",
        TypeKind::Class  => "class",
    };

    if td.fields.is_empty() && td.methods.is_empty() {
        return format!("```haki\n{} {}\n```", kw, td.name);
    }

    let mut lines = vec![format!("{} {} {{", kw, td.name)];
    for f in &td.fields {
        let mutability = match f.mutability {
            haki_ast::Mut::Const => "const",
            haki_ast::Mut::Let   => "let",
        };
        let weak = if f.is_weak { "weak " } else { "" };
        lines.push(format!("    {}{} {}: {}", weak, mutability, f.name, format_ty_kind(&f.ty.kind)));
    }
    for m in &td.methods {
        lines.push(format!("    {}", format_fn_signature_brief(&m.name, m)));
    }
    lines.push("}".into());

    format!("```haki\n{}\n```", lines.join("\n"))
}

/// Format a `ReturnTy` to a display string.
fn format_return_ty(ret: &Option<haki_ast::ReturnTy>) -> String {
    use haki_ast::ReturnTy;
    match ret {
        None                   => "void".into(),
        Some(ReturnTy::Single(ty)) => format_ty_kind(&ty.kind),
        Some(ReturnTy::Tuple(tys)) => {
            let parts: Vec<String> = tys.iter()
                .map(|t| format_ty_kind(&t.kind))
                .collect();
            format!("({})", parts.join(", "))
        }
    }
}

/// Format an AST `TyKind` to a display string.
fn format_ty_kind(kind: &haki_ast::TyKind) -> String {
    match kind {
        haki_ast::TyKind::Named(id) => id.name.clone(),
        haki_ast::TyKind::Generic(id, args) => {
            let formatted: Vec<String> = args.iter()
                .map(|a| format_ty_kind(&a.kind))
                .collect();
            format!("{}<{}>", id.name, formatted.join(", "))
        }
        haki_ast::TyKind::Optional(inner) => {
            format!("{}?", format_ty_kind(&inner.kind))
        }
        haki_ast::TyKind::Tuple(tys) => {
            let parts: Vec<String> = tys.iter()
                .map(|t| format_ty_kind(&t.kind))
                .collect();
            format!("({})", parts.join(", "))
        }
        haki_ast::TyKind::Fn(params, ret) => {
            let ps: Vec<String> = params.iter()
                .map(|p| format_ty_kind(&p.kind))
                .collect();
            match ret {
                Some(r) => format!("fn({}) -> {}", ps.join(", "), format_ty_kind(&r.kind)),
                None    => format!("fn({})", ps.join(", ")),
            }
        }
    }
}

// ── Go-to-definition ──────────────────────────────────────────────────────────

fn handle_definition(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p,
        None    => return Value::Null,
    };
    let state = match index.get(&PathBuf::from(&path_str)) {
        Some(s) => s,
        None    => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = crate::lsp_index::position_to_offset(&state.text, line, col);
    let uri    = params["textDocument"]["uri"].clone();

    // Priority 1: local variable / parameter (scope-aware)
    if let Some(hit) = crate::lsp_index::resolve_local_definition(&state.ast, offset) {
        let (dl, dc) = crate::lsp_index::offset_to_position(&state.text, hit.def_span.lo);
        let (el, ec) = crate::lsp_index::offset_to_position(&state.text, hit.def_span.hi);
        return json!({ "uri": uri, "range": lsp_range(dl, dc, el, ec) });
    }

    // Priority 2: top-level definition (fn / struct / class / enum)
    let ident_name = match find_ident_at_offset(&state.text, offset) {
        Some(n) => n,
        None    => return Value::Null,
    };

    if let Some(def_span) = find_definition_in_ast(&state.ast, &ident_name) {
        let (dl, dc) = crate::lsp_index::offset_to_position(&state.text, def_span.lo);
        let (el, ec) = crate::lsp_index::offset_to_position(&state.text, def_span.hi);
        return json!({ "uri": uri, "range": lsp_range(dl, dc, el, ec) });
    }

    Value::Null
}

// ── AST traversal helpers ─────────────────────────────────────────────────────

use haki_typeck::typed_ast::*;
use haki_typeck::typed_ast::SemTy;

/// Walk the typed AST and return the display string for the type of the
/// expression whose span contains `offset`.
fn find_type_at_offset(typed: &TypedSourceFile, offset: u32) -> Option<String> {
    for item in &typed.items {
        if let TypedItemKind::Fn(f) = &item.kind {
            if let Some(ty) = find_type_in_block(&f.body, offset) {
                return Some(ty);
            }
        }
    }
    None
}

fn find_type_in_block(block: &TypedBlock, offset: u32) -> Option<String> {
    for stmt in &block.stmts {
        if let Some(ty) = find_type_in_stmt(stmt, offset) {
            return Some(ty);
        }
    }
    None
}

fn find_type_in_stmt(stmt: &TypedStmt, offset: u32) -> Option<String> {
    match &stmt.kind {
        TypedStmtKind::Let(l)    => find_type_in_expr(&l.init, offset),
        TypedStmtKind::Return(r) => {
            for v in &r.values {
                if let Some(ty) = find_type_in_expr(v, offset) {
                    return Some(ty);
                }
            }
            None
        }
        TypedStmtKind::Expr(e)   => find_type_in_expr(e, offset),
        TypedStmtKind::Yield(e)  => find_type_in_expr(e, offset),
        TypedStmtKind::If(i)     => {
            find_type_in_expr(&i.cond, offset)
                .or_else(|| find_type_in_block(&i.then_block, offset))
                .or_else(|| match &i.else_branch {
                    Some(TypedElseBranch::Block(b)) => find_type_in_block(b, offset),
                    Some(TypedElseBranch::If(inner)) => find_type_in_block(&inner.then_block, offset),
                    None => None,
                })
        }
        TypedStmtKind::While(w)  => {
            find_type_in_expr(&w.cond, offset)
                .or_else(|| find_type_in_block(&w.body, offset))
        }
        _ => None,
    }
}

fn find_type_in_expr(expr: &TypedExpr, offset: u32) -> Option<String> {
    // Check if offset falls within this expression's span
    if expr.span.lo <= offset && offset <= expr.span.hi {
        // Check children first (most specific match wins)
        let child = match &expr.kind {
            TypedExprKind::Call(callee, args) => {
                find_type_in_expr(callee, offset)
                    .or_else(|| args.iter().find_map(|a| find_type_in_expr(a, offset)))
            }
            TypedExprKind::MethodCall(recv, _, args) => {
                find_type_in_expr(recv, offset)
                    .or_else(|| args.iter().find_map(|a| find_type_in_expr(a, offset)))
            }
            TypedExprKind::Field(recv, _) => find_type_in_expr(recv, offset),
            TypedExprKind::Binary(_, l, r) => {
                find_type_in_expr(l, offset).or_else(|| find_type_in_expr(r, offset))
            }
            TypedExprKind::Unary(_, e) => find_type_in_expr(e, offset),
            TypedExprKind::If(i) => {
                find_type_in_expr(&i.cond, offset)
                    .or_else(|| find_type_in_block(&i.then_block, offset))
                    .or_else(|| match &i.else_branch {
                        Some(TypedElseBranch::Block(b)) => find_type_in_block(b, offset),
                        Some(TypedElseBranch::If(inner)) => find_type_in_block(&inner.then_block, offset),
                        None => None,
                    })
            }
            _ => None,
        };
        child.or_else(|| Some(format_sem_ty(&expr.ty)))
    } else {
        None
    }
}

fn format_sem_ty(ty: &SemTy) -> String {
    ty.display()
}

/// Find the identifier token that spans `offset` in the raw source text.
fn find_ident_at_offset(text: &str, offset: u32) -> Option<String> {
    let offset = offset as usize;
    if offset >= text.len() { return None; }
    let bytes = text.as_bytes();

    // Walk back to find start of identifier
    let mut start = offset;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    // Walk forward to find end
    let mut end = offset;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    if start == end { return None; }
    Some(text[start..end].to_string())
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Search the untyped AST for a top-level definition named `name`.
/// Returns the definition's span if found.
fn find_definition_in_ast(ast: &haki_ast::SourceFile, name: &str) -> Option<haki_ast::Span> {
    for item in &ast.items {
        match &item.kind {
            haki_ast::ItemKind::Fn(f) if f.name.name == name => {
                return Some(f.name.span);
            }
            haki_ast::ItemKind::Struct(s) if s.name.name == name => {
                return Some(s.name.span);
            }
            haki_ast::ItemKind::Class(c) if c.name.name == name => {
                return Some(c.name.span);
            }
            haki_ast::ItemKind::Enum(e) if e.name.name == name => {
                return Some(e.name.span);
            }
            _ => {}
        }
    }
    None
}

// ── JSON-RPC transport ────────────────────────────────────────────────────────

fn send_response(out: &mut impl Write, id: Option<Value>, result: Value) {
    let msg = if let Some(id) = id {
        json!({ "jsonrpc": "2.0", "id": id, "result": result })
    } else {
        json!({ "jsonrpc": "2.0", "id": Value::Null, "result": result })
    };
    write_message(out, &msg);
}

fn send_notification(out: &mut impl Write, method: &str, params: Value) {
    let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    write_message(out, &msg);
}

fn write_message(out: &mut impl Write, msg: &Value) {
    let body = msg.to_string();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = out.flush();
}

// ── URI helpers ───────────────────────────────────────────────────────────────

fn uri_to_path(uri: &Value) -> Option<String> {
    let s = uri.as_str()?;
    // Strip "file://" prefix
    if let Some(rest) = s.strip_prefix("file://") {
        // URL decode %XX sequences minimally
        Some(url_decode(rest))
    } else {
        Some(s.to_string())
    }
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next().unwrap_or(b'0');
            let h2 = bytes.next().unwrap_or(b'0');
            if let Ok(decoded) = u8::from_str_radix(
                std::str::from_utf8(&[h1, h2]).unwrap_or("00"), 16
            ) {
                result.push(decoded as char);
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

// ── LSP range helper ─────────────────────────────────────────────────────────

fn lsp_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Value {
    json!({
        "start": { "line": start_line, "character": start_char },
        "end":   { "line": end_line,   "character": end_char   },
    })
}
