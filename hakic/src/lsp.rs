/// lsp.rs — Haki Language Server Protocol implementation.
///
/// Speaks JSON-RPC over stdin/stdout. Capabilities:
///   - textDocument/didOpen, didChange, didClose
///   - textDocument/publishDiagnostics (push)
///   - textDocument/hover           — signatures + expression types
///   - textDocument/definition      — local vars, top-level, cross-file imports
///   - textDocument/completion      — scope vars, functions, module members
///   - textDocument/signatureHelp   — parameter hints while typing
///   - textDocument/references      — find all usages in file
///   - textDocument/documentSymbol  — outline panel (fns, structs, classes, enums)
///   - textDocument/rename          — rename symbol across file

use std::io::{self, BufRead, Write};
use serde_json::{json, Value};
use crate::lsp_index::{
    ProjectIndex, CompletionItem, CompletionKind, SymbolKind,
    offset_to_position, position_to_offset,
    resolve_local_definition, find_definition_in_ast, find_ident_at_offset,
    format_fn_signature, format_fn_signature_brief, format_ty_kind,
};

pub fn run_lsp() {
    let stdin  = io::stdin();
    let stdout = io::stdout();
    let mut out    = io::BufWriter::new(stdout.lock());
    let mut index  = ProjectIndex::new();
    let mut initialized = false;

    for line in stdin.lock().lines() {
        let header = match line { Ok(l) => l, Err(_) => break };
        if !header.starts_with("Content-Length:") { continue; }

        let len: usize = header["Content-Length:".len()..].trim().parse().unwrap_or(0);
        let mut blank = String::new();
        let _ = stdin.lock().read_line(&mut blank);

        let mut body = vec![0u8; len];
        use std::io::Read;
        let _ = io::stdin().lock().read_exact(&mut body);

        let msg: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = msg["method"].as_str().unwrap_or("").to_string();
        let id     = msg.get("id").cloned();
        let params = msg["params"].clone();

        match method.as_str() {
            "initialize" => {
                initialized = true;
                send_response(&mut out, id, initialize_result());
            }
            "initialized" => {}
            "shutdown" => { send_response(&mut out, id, Value::Null); }
            "exit"     => { break; }

            "textDocument/didOpen" => {
                if let Some(path_str) = uri_to_path(&params["textDocument"]["uri"]) {
                    let text = params["textDocument"]["text"]
                        .as_str().unwrap_or("").to_string();
                    let path = std::path::PathBuf::from(&path_str);
                    index.update_file(&path, text);
                    push_diagnostics(&mut out, &params["textDocument"]["uri"], &index, &path_str);
                }
            }

            "textDocument/didChange" => {
                if let Some(path_str) = uri_to_path(&params["textDocument"]["uri"]) {
                    if let Some(text) = params["contentChanges"][0]["text"].as_str() {
                        let path = std::path::PathBuf::from(&path_str);
                        index.update_file(&path, text.to_string());
                        push_diagnostics(&mut out, &params["textDocument"]["uri"], &index, &path_str);
                    }
                }
            }

            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    send_notification(&mut out, "textDocument/publishDiagnostics", json!({
                        "uri": uri, "diagnostics": []
                    }));
                }
            }

            "textDocument/hover"          => send_response(&mut out, id, handle_hover(&params, &index)),
            "textDocument/definition"     => send_response(&mut out, id, handle_definition(&params, &index)),
            "textDocument/completion"     => send_response(&mut out, id, handle_completion(&params, &index)),
            "textDocument/signatureHelp"  => send_response(&mut out, id, handle_signature_help(&params, &index)),
            "textDocument/references"     => send_response(&mut out, id, handle_references(&params, &index)),
            "textDocument/documentSymbol" => send_response(&mut out, id, handle_document_symbols(&params, &index)),
            "textDocument/rename"         => send_response(&mut out, id, handle_rename(&params, &index)),

            _ => {
                if id.is_some() { send_response(&mut out, id, Value::Null); }
            }
        }
    }
}

// ── Capabilities ──────────────────────────────────────────────────────────────

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "definitionProvider": true,
            "referencesProvider": true,
            "documentSymbolProvider": true,
            "renameProvider": true,
            "completionProvider": {
                "triggerCharacters": [".", "("],
                "resolveProvider": false
            },
            "signatureHelpProvider": {
                "triggerCharacters": ["(", ","]
            }
        },
        "serverInfo": {
            "name":    "hakic-lsp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

fn push_diagnostics(
    out:      &mut impl Write,
    uri:      &Value,
    index:    &ProjectIndex,
    path_str: &str,
) {
    let path  = std::path::PathBuf::from(path_str);
    let state = match index.get(&path) {
        Some(s) => s,
        None    => return,
    };

    let mut diagnostics: Vec<Value> = Vec::new();

    for err in &state.parse_errors {
        let span = err.span();
        let (sl, sc) = offset_to_position(&state.text, span.lo);
        let (el, ec) = offset_to_position(&state.text, span.hi);
        diagnostics.push(json!({
            "range":    lsp_range(sl, sc, el, ec),
            "severity": 1,
            "source":   "hakic",
            "message":  err.to_string(),
        }));
    }

    if let Some(ref msg) = state.type_error {
        let (range, message) = parse_type_error_range(msg, &state.text);
        diagnostics.push(json!({
            "range":    range,
            "severity": 1,
            "source":   "hakic",
            "message":  message,
        }));
    }

    send_notification(out, "textDocument/publishDiagnostics", json!({
        "uri": uri, "diagnostics": diagnostics
    }));
}

fn parse_type_error_range(msg: &str, text: &str) -> (Value, String) {
    // Format: "file.haki: line:col: message"  or  "line:col: message"
    let after_file = msg.find(": ").map(|i| &msg[i+2..]).unwrap_or(msg);
    if let Some(colon) = after_file.find(':') {
        if let Ok(line) = after_file[..colon].trim().parse::<u32>() {
            let rest = &after_file[colon+1..];
            if let Some(c2) = rest.find(':') {
                if let Ok(col) = rest[..c2].trim().parse::<u32>() {
                    let message = rest[c2+1..].trim().to_string();
                    let l = line.saturating_sub(1);
                    let c = col.saturating_sub(1);
                    return (lsp_range(l, c, l, c + 20), message);
                }
            }
        }
    }
    (lsp_range(0, 0, 0, 1), msg.to_string())
}

// ── Hover ─────────────────────────────────────────────────────────────────────

fn handle_hover(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path  = std::path::PathBuf::from(&path_str);
    let state = match index.get(&path) {
        Some(s) => s, None => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = position_to_offset(&state.text, line, col);

    // Priority 1: identifier → show full signature from sym table
    if let Some(ident) = find_ident_at_offset(&state.text, offset) {
        if let Some(sig) = hover_signature_for_ident(&ident, state) {
            return json!({ "contents": { "kind": "markdown", "value": sig } });
        }
    }

    // Priority 2: expression type from typed AST
    if let Some(typed) = &state.typed {
        if let Some(ty_str) = find_type_at_offset(typed, offset) {
            return json!({ "contents": { "kind": "markdown", "value": format!("`{}`", ty_str) } });
        }
    }

    Value::Null
}

fn hover_signature_for_ident(name: &str, state: &crate::lsp_index::FileState) -> Option<String> {
    let sym = state.sym.as_ref()?;

    if let Some(fi) = sym.functions.get(name) {
        let sig = format_fn_signature(name, fi);
        // Add doc comment if present
        return Some(format!("```haki\n{sig}\n```"));
    }
    if let Some(td) = sym.types.get(name) {
        return Some(format!("```haki\n{}\n```", format_type_signature(td)));
    }
    if let Some(ed) = sym.enum_defs.get(name) {
        let variants: Vec<String> = ed.variants.iter().map(|v| {
            if v.fields.is_empty() { v.name.name.clone() }
            else {
                let fs: Vec<_> = v.fields.iter().map(|f| format_ty_kind(&f.kind)).collect();
                format!("{}({})", v.name.name, fs.join(", "))
            }
        }).collect();
        return Some(format!("```haki\nenum {} {{\n    {}\n}}\n```",
            name, variants.join("\n    ")));
    }
    None
}

fn format_type_signature(td: &haki_typeck::collector::TypeDef) -> String {
    let kw = match td.kind { haki_typeck::collector::TypeKind::Class => "class", _ => "struct" };
    if td.fields.is_empty() {
        format!("{} {} {{}}", kw, td.name)
    } else {
        let fs: Vec<_> = td.fields.iter()
            .map(|f| format!("    {}: {}", f.name, format_ty_kind(&f.ty.kind)))
            .collect();
        format!("{} {} {{\n{}\n}}", kw, td.name, fs.join("\n"))
    }
}

// ── Go-to-definition ──────────────────────────────────────────────────────────

fn handle_definition(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path  = std::path::PathBuf::from(&path_str);
    let state = match index.get(&path) {
        Some(s) => s, None => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = position_to_offset(&state.text, line, col);
    let uri    = params["textDocument"]["uri"].clone();

    // Priority 1: local variable / parameter
    if let Some(hit) = resolve_local_definition(&state.ast, offset) {
        let (dl, dc) = offset_to_position(&state.text, hit.def_span.lo);
        let (el, ec) = offset_to_position(&state.text, hit.def_span.hi);
        return json!([{ "uri": uri, "range": lsp_range(dl, dc, el, ec) }]);
    }

    // Priority 2: qualified access  alias.Name  →  cross-file definition
    let raw_text = &state.text;
    let bytes    = raw_text.as_bytes();
    let pos      = offset as usize;

    // Look left for  alias.  pattern
    if pos > 1 {
        let mut dot_pos = None;
        // Find a '.' immediately before the current ident
        let ident_start = {
            let mut s = pos;
            while s > 0 && (bytes[s-1].is_ascii_alphanumeric() || bytes[s-1] == b'_') { s -= 1; }
            s
        };
        if ident_start > 0 && bytes[ident_start - 1] == b'.' {
            dot_pos = Some(ident_start - 1);
        }

        if let Some(dp) = dot_pos {
            // Extract alias name before the dot
            let alias_end = dp;
            let mut alias_start = alias_end;
            while alias_start > 0 && (bytes[alias_start-1].is_ascii_alphanumeric() || bytes[alias_start-1] == b'_') {
                alias_start -= 1;
            }
            let alias      = &raw_text[alias_start..alias_end];
            let symbol     = find_ident_at_offset(raw_text, offset);

            if !alias.is_empty() {
                if let Some(sym_name) = symbol {
                    // Try cross-file resolution
                    if let Some((target_path, span)) =
                        index.resolve_cross_file_definition(&path, alias, &sym_name)
                    {
                        // Read the target file to map spans
                        let target_text = std::fs::read_to_string(&target_path)
                            .unwrap_or_default();
                        let (dl, dc) = offset_to_position(&target_text, span.lo);
                        let (el, ec) = offset_to_position(&target_text, span.hi);
                        let target_uri = path_to_uri(&target_path);
                        return json!([{ "uri": target_uri, "range": lsp_range(dl, dc, el, ec) }]);
                    }
                }
            }
        }
    }

    // Priority 3: top-level definition in same file
    let ident_name = match find_ident_at_offset(raw_text, offset) {
        Some(n) => n, None => return Value::Null,
    };

    if let Some(def_span) = find_definition_in_ast(&state.ast, &ident_name) {
        let (dl, dc) = offset_to_position(&state.text, def_span.lo);
        let (el, ec) = offset_to_position(&state.text, def_span.hi);
        return json!([{ "uri": uri, "range": lsp_range(dl, dc, el, ec) }]);
    }

    Value::Null
}

// ── Completion ────────────────────────────────────────────────────────────────

fn handle_completion(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path  = std::path::PathBuf::from(&path_str);
    let state = match index.get(&path) {
        Some(s) => s, None => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = position_to_offset(&state.text, line, col);
    let text   = &state.text;
    let bytes  = text.as_bytes();
    let pos    = (offset as usize).min(bytes.len());

    // Check if we're after a '.' — module or field completion
    let before_dot = pos > 0 && bytes[pos - 1] == b'.';
    let dot_pos    = if before_dot { pos - 1 } else {
        // Check if there's identifier.| pattern (cursor right after '.')
        let mut p = pos;
        // skip back over current partial ident
        while p > 0 && (bytes[p-1].is_ascii_alphanumeric() || bytes[p-1] == b'_') { p -= 1; }
        if p > 0 && bytes[p-1] == b'.' { p - 1 } else { usize::MAX }
    };

    if dot_pos != usize::MAX {
        // Extract the identifier before the dot
        let mut alias_end   = dot_pos;
        let mut alias_start = alias_end;
        while alias_start > 0 && (bytes[alias_start-1].is_ascii_alphanumeric() || bytes[alias_start-1] == b'_') {
            alias_start -= 1;
        }
        let alias = &text[alias_start..alias_end];
        if !alias.is_empty() {
            let items = index.module_completions(&path, alias);
            if !items.is_empty() {
                return completion_list(items, false);
            }
        }
    }

    // Scope + top-level completion
    let items = index.scope_completions(&path, offset);
    completion_list(items, true)
}

fn completion_list(items: Vec<CompletionItem>, is_incomplete: bool) -> Value {
    let list: Vec<Value> = items.into_iter().map(|item| {
        let kind_num = match item.kind {
            CompletionKind::Function    => 3,  // Function
            CompletionKind::Struct      => 22, // Struct
            CompletionKind::Class       => 7,  // Class
            CompletionKind::Module      => 9,  // Module
            CompletionKind::Variable    => 6,  // Variable
            CompletionKind::Field       => 5,  // Field
            CompletionKind::EnumVariant => 20, // EnumMember
        };
        let mut v = json!({
            "label":      item.label,
            "kind":       kind_num,
            "insertText": item.insert_text,
        });
        if let Some(detail) = item.detail {
            v["detail"] = json!(detail);
        }
        v
    }).collect();

    json!({ "isIncomplete": is_incomplete, "items": list })
}

// ── Signature help ────────────────────────────────────────────────────────────

fn handle_signature_help(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path  = std::path::PathBuf::from(&path_str);
    let line  = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col   = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let state = match index.get(&path) {
        Some(s) => s, None => return Value::Null,
    };
    let offset = position_to_offset(&state.text, line, col);

    let help = match index.signature_help(&path, offset) {
        Some(h) => h, None => return Value::Null,
    };

    let params_json: Vec<Value> = help.params.iter()
        .map(|p| json!({ "label": p }))
        .collect();

    json!({
        "signatures": [{
            "label":          help.label,
            "parameters":     params_json,
            "activeParameter": help.active_param,
        }],
        "activeSignature":  0,
        "activeParameter":  help.active_param,
    })
}

// ── Find references ───────────────────────────────────────────────────────────

fn handle_references(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path  = std::path::PathBuf::from(&path_str);
    let state = match index.get(&path) {
        Some(s) => s, None => return Value::Null,
    };

    let line   = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col    = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let offset = position_to_offset(&state.text, line, col);
    let uri    = params["textDocument"]["uri"].clone();

    let name = match find_ident_at_offset(&state.text, offset) {
        Some(n) => n, None => return json!([]),
    };

    let refs = index.find_references(&path, &name);
    let locations: Vec<Value> = refs.iter().map(|(sl, sc, el, ec)| {
        json!({ "uri": uri, "range": lsp_range(*sl, *sc, *el, *ec) })
    }).collect();

    json!(locations)
}

// ── Document symbols ──────────────────────────────────────────────────────────

fn handle_document_symbols(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path = std::path::PathBuf::from(&path_str);
    let syms = index.document_symbols(&path);

    let items: Vec<Value> = syms.iter().map(|s| {
        let kind_num = match s.kind {
            SymbolKind::Function => 12,
            SymbolKind::Struct   => 23,
            SymbolKind::Class    => 5,
            SymbolKind::Enum     => 10,
        };
        let (sl, sc) = s.start;
        let (el, ec) = s.end;
        json!({
            "name":           s.name,
            "kind":           kind_num,
            "range":          lsp_range(sl, sc, el, ec),
            "selectionRange": lsp_range(sl, sc, sl, sc + s.name.len() as u32),
        })
    }).collect();

    json!(items)
}

// ── Rename ────────────────────────────────────────────────────────────────────

fn handle_rename(params: &Value, index: &ProjectIndex) -> Value {
    let path_str = match uri_to_path(&params["textDocument"]["uri"]) {
        Some(p) => p, None => return Value::Null,
    };
    let path      = std::path::PathBuf::from(&path_str);
    let state     = match index.get(&path) { Some(s) => s, None => return Value::Null };
    let line      = params["position"]["line"].as_u64().unwrap_or(0) as u32;
    let col       = params["position"]["character"].as_u64().unwrap_or(0) as u32;
    let new_name  = params["newName"].as_str().unwrap_or("").to_string();
    let offset    = position_to_offset(&state.text, line, col);
    let uri       = params["textDocument"]["uri"].clone();

    let name = match find_ident_at_offset(&state.text, offset) {
        Some(n) => n, None => return Value::Null,
    };

    let refs = index.find_references(&path, &name);
    let edits: Vec<Value> = refs.iter().map(|(sl, sc, el, ec)| {
        json!({ "range": lsp_range(*sl, *sc, *el, *ec), "newText": new_name })
    }).collect();

    if edits.is_empty() { return Value::Null; }

    json!({
        "changes": {
            uri.as_str().unwrap_or(""): edits
        }
    })
}

// ── Typed AST traversal for expression types ─────────────────────────────────

use haki_typeck::typed_ast::*;

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
        TypedStmtKind::Return(r) => r.values.iter().find_map(|v| find_type_in_expr(v, offset)),
        TypedStmtKind::Yield(e) | TypedStmtKind::Expr(e) => find_type_in_expr(e, offset),
        TypedStmtKind::If(i) => {
            find_type_in_expr(&i.cond, offset)
                .or_else(|| find_type_in_block(&i.then_block, offset))
                .or_else(|| match &i.else_branch {
                    Some(haki_typeck::typed_ast::TypedElseBranch::Block(b)) => find_type_in_block(b, offset),
                    _ => None,
                })
        }
        TypedStmtKind::While(w)  => find_type_in_expr(&w.cond, offset)
            .or_else(|| find_type_in_block(&w.body, offset)),
        TypedStmtKind::For(f)    => find_type_in_block(&f.body, offset),
        TypedStmtKind::Match(m)  => {
            find_type_in_expr(&m.scrutinee, offset)
                .or_else(|| m.arms.iter().find_map(|a| find_type_in_block(&a.body, offset)))
        }
        _ => None,
    }
}

fn find_type_in_expr(expr: &TypedExpr, offset: u32) -> Option<String> {
    if expr.span.lo > offset || expr.span.hi < offset { return None; }

    // Check child expressions for a tighter match first
    let child_match = match &expr.kind {
        TypedExprKind::Call(callee, args) => {
            find_type_in_expr(callee, offset)
                .or_else(|| args.iter().find_map(|a| find_type_in_expr(a, offset)))
        }
        TypedExprKind::MethodCall(recv, _, args) => {
            find_type_in_expr(recv, offset)
                .or_else(|| args.iter().find_map(|a| find_type_in_expr(a, offset)))
        }
        TypedExprKind::Field(recv, _) => find_type_in_expr(recv, offset),
        TypedExprKind::Binary(_, left, right) => {
            find_type_in_expr(left, offset).or_else(|| find_type_in_expr(right, offset))
        }
        TypedExprKind::If(i) => {
            find_type_in_expr(&i.cond, offset)
                .or_else(|| find_type_in_block(&i.then_block, offset))
                .or_else(|| match &i.else_branch {
                    Some(haki_typeck::typed_ast::TypedElseBranch::Block(b)) => find_type_in_block(b, offset),
                    Some(haki_typeck::typed_ast::TypedElseBranch::If(inner)) => find_type_in_block(&inner.then_block, offset),
                    None => None,
                })
        }
        TypedExprKind::Block(b) => find_type_in_block(b, offset),
        _ => None,
    };

    child_match.or_else(|| Some(format_sem_ty(&expr.ty)))
}

fn format_sem_ty(ty: &SemTy) -> String {
    match ty {
        SemTy::Int            => "int".into(),
        SemTy::Float          => "float".into(),
        SemTy::Bool           => "bool".into(),
        SemTy::String         => "string".into(),
        SemTy::Void           => "void".into(),
        
        SemTy::Named(n)       => n.clone(),
        SemTy::Optional(i)    => format!("{}?", format_sem_ty(i)),
        
        
        SemTy::Fn(ps, r)      => {
            let params: Vec<_> = ps.iter().map(format_sem_ty).collect();
            format!("fn({}) -> {}", params.join(", "), format_sem_ty(r))
        }
        SemTy::Tuple(ts)      => {
            let inner: Vec<_> = ts.iter().map(format_sem_ty).collect();
            format!("({})", inner.join(", "))
        }
        
        
        SemTy::Generic(n, _)  => n.clone(),
        SemTy::Var(_) | SemTy::Never => "?".into(),
        SemTy::Closure(ps, r) => { let params: Vec<_> = ps.iter().map(format_sem_ty).collect(); format!("fn({}) -> {}", params.join(", "), format_sem_ty(r)) }
    }
}

// ── Transport helpers ─────────────────────────────────────────────────────────

fn send_response(out: &mut impl Write, id: Option<Value>, result: Value) {
    let msg = if let Some(id) = id {
        json!({ "jsonrpc": "2.0", "id": id, "result": result })
    } else {
        json!({ "jsonrpc": "2.0", "result": result })
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

fn uri_to_path(uri: &Value) -> Option<String> {
    let s = uri.as_str()?;
    let s = s.strip_prefix("file://")?;
    Some(url_decode(s))
}

fn path_to_uri(path: &std::path::Path) -> String {
    format!("file://{}", path.display())
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i+1..i+3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn lsp_range(sl: u32, sc: u32, el: u32, ec: u32) -> Value {
    json!({
        "start": { "line": sl, "character": sc },
        "end":   { "line": el, "character": ec }
    })
}
