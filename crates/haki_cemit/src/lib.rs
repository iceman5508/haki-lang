/// haki_cemit — Emit a Haki MonoProgram as a single portable C file.
///
/// The emitted C file:
///   1. Embeds haki_runtime.c (ARC, arrays, maps, strings, IO, threads)
///   2. Forward-declares all struct and function names
///   3. Defines concrete structs
///   4. Defines all functions in C syntax
///
/// The resulting file compiles with:
///   gcc -O2 -lpthread -o program program.c
///   clang -O2 -lpthread -o program program.c
///
/// This backend targets v1.0 distribution: users install a pre-built hakic binary
/// that emits C, then compile with their system gcc/clang. No LLVM or Rust required.

use haki_ast::{BinaryOp, Binding, Mut, UnaryOp};
use haki_mono::mono_ast::*;
use haki_typeck::typed_ast::SemTy;
use haki_stdlib::RUNTIME_C_SOURCE;

// ── mod_haki ABI footer ───────────────────────────────────────────────────────
//
// Appended when compiling with --target so. Bridges between the stable C ABI
// (haki_abi.h) and the Haki handler's fn handle(req: HttpRequest) -> HttpResponse.
//
// The user's Haki file must define:
//   fn handle(req: HttpRequest) -> HttpResponse
//
// This footer exports the three symbols mod_haki requires:
//   haki_handle_request  — called per request
//   haki_response_free   — called by mod_haki to free the response
//   haki_abi_version     — checked by mod_haki at load time

/// HTTP type definitions injected before function prototypes in --target so mode.
/// Defines HttpRequest and HttpResponse so compiler-generated prototypes compile.
pub const SO_HTTP_TYPES: &str = concat!(
    "/* ── HTTP types (must match runtime HttpRequest/HttpResponse/HttpServer) ── */\n",
    "#define HAKI_HTTP_TYPES_DEFINED\n",
    "#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\n\n",
    "typedef struct {\n",
    "    const char* method;\n",
    "    const char* path;\n",
    "    const char* body;\n",
    "    size_t      body_len;\n",
    "    void*       connection;\n",
    "} HttpRequest;\n\n",
    "typedef struct {\n",
    "    int         status;\n",
    "    const char* body;\n",
    "    const char* contentType;\n",
    "} HttpResponse;\n\n",
    "typedef HttpResponse* (*HttpHandler)(HttpRequest*);\n\n",
    "typedef struct {\n",
    "    int64_t     port;\n",
    "    HttpHandler handler;\n",
    "    void*       daemon;\n",
    "} HttpServer;\n\n",
    "HttpServer* haki_http_server_new(int64_t port, HttpHandler handler);\n",
    "void        haki_http_server_listen(HttpServer* s);\n",
    "const char* haki_request_param(HttpRequest* req, const char* key);\n",
    "const char* haki_json_decode_get(const char* json, const char* key);\n\n",
    "/* Router */\n",
    "typedef HttpResponse* (*HakiRouteHandler)(HttpRequest*);\n",
    "typedef struct { void* routes[256]; int count; } HakiRouter;\n",
    "typedef HakiRouter Router;\n",
    "Router*     haki_router_new(void);\n",
    "void        haki_router_add(Router* r, const char* method, const char* pattern, HakiRouteHandler h);\n",
    "HttpResponse* haki_router_dispatch(Router* r, HttpRequest* req);\n\n",
    "/* Helper: construct HttpResponse on heap */\n",
    "static inline HttpResponse* haki_make_http_response(int status, const char* body, const char* ct) {\n",
    "    HttpResponse* r = (HttpResponse*)malloc(sizeof(HttpResponse));\n",
    "    r->status = status; r->body = body; r->contentType = ct; return r; }\n\n"
);
/// ABI bridge footer appended at end of --target so output.
/// Exports haki_handle_request, haki_response_free, haki_abi_version.
const SO_ABI_FOOTER: &str = concat!(
    "/* ── mod_haki ABI exports ── */\n",
    "#define HAKI_ABI_VERSION 1\n",
    "#define HAKI_MAX_HEADERS 64\n\n",
    "typedef struct { const char* name; const char* value; } HakiHeader_abi;\n\n",
    "typedef struct {\n",
    "    uint32_t abi_version;\n    const char* method;\n    const char* path;\n",
    "    const char* query;\n    const char* body;\n    size_t body_len;\n",
    "    HakiHeader_abi headers[HAKI_MAX_HEADERS];\n    uint32_t header_count;\n",
    "    const char* remote_addr;\n} HakiRequest;\n\n",
    "typedef struct {\n",
    "    int32_t status;\n    char* body;\n    size_t body_len;\n",
    "    char* content_type;\n    HakiHeader_abi headers[HAKI_MAX_HEADERS];\n",
    "    uint32_t header_count;\n} HakiResponse;\n\n",
    "HakiResponse* haki_handle_request(const HakiRequest* abi_req) {\n",
    "    HttpRequest haki_req_val;\n",
    "    haki_req_val.method = abi_req->method ? abi_req->method : \"GET\";\n",
    "    haki_req_val.path = abi_req->path ? abi_req->path : \"/\";\n",
    "    haki_req_val.body = abi_req->body ? abi_req->body : \"\";\n",
    "    haki_req_val.body_len = abi_req->body_len;\n",
    "    haki_req_val.connection = NULL;\n",
    "    HttpResponse* haki_resp = handle(&haki_req_val);\n",
    "    HakiResponse* resp = (HakiResponse*)malloc(sizeof(HakiResponse));\n",
    "    if (!resp) return NULL;\n",
    "    memset(resp, 0, sizeof(HakiResponse));\n",
    "    resp->status = haki_resp ? (int32_t)haki_resp->status : 500;\n",
    "    const char* body_src = haki_resp ? haki_resp->body : NULL;\n",
    "    if (body_src && body_src[0]) {\n",
    "        size_t len = strlen(body_src);\n",
    "        resp->body = (char*)malloc(len + 1);\n",
    "        if (resp->body) { memcpy(resp->body, body_src, len + 1); resp->body_len = len; }\n",
    "    }\n",
    "    const char* ct = (haki_resp && haki_resp->content_type) ? haki_resp->content_type : \"text/plain; charset=utf-8\";\n",
    "    resp->content_type = strdup(ct);\n",
    "    return resp;\n}\n\n",
    "void haki_response_free(HakiResponse* resp) {\n",
    "    if (!resp) return; free(resp->body); free(resp->content_type); free(resp);\n}\n\n",
    "uint32_t haki_abi_version(void) { return HAKI_ABI_VERSION; }\n"
);

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum CEmitError {
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub type CResult<T> = Result<T, CEmitError>;

// ── Public entry point ────────────────────────────────────────────────────────

/// Emit a complete, self-contained C file from a monomorphized program.
/// Pass `source_path` to enable `#line` directives for debugger source mapping.
pub fn emit_c(prog: &MonoProgram, source_path: Option<&str>) -> CResult<String> {
    emit_c_impl(prog, false, false, false, source_path.unwrap_or(""))
}

/// Emit C source for a program that uses HttpServer/HttpRequest.
pub fn emit_c_http(prog: &MonoProgram, source_path: Option<&str>) -> CResult<String> {
    emit_c_impl(prog, false, true, false, source_path.unwrap_or(""))
}

/// Emit C source targeting a shared library (.so) for mod_haki.
pub fn emit_c_so(prog: &MonoProgram) -> CResult<String> {
    emit_c_impl(prog, true, true, false, "")
}

/// Emit C with the full RUNTIME_C_SOURCE as preamble (for HTTP unity builds).
/// This avoids struct conflicts when compiling a single .c file that includes everything.
pub fn emit_c_http_unity(prog: &MonoProgram, source_path: Option<&str>) -> CResult<String> {
    emit_c_impl(prog, false, true, true, source_path.unwrap_or(""))
}

fn emit_c_impl(prog: &MonoProgram, target_so: bool, uses_http: bool, full_runtime: bool, source_path: &str) -> CResult<String> {
    let mut out = String::with_capacity(64 * 1024);
    let cx = Cx { prog, self_fields: std::cell::RefCell::new(std::collections::HashSet::new()), fn_locals: std::cell::RefCell::new(std::collections::HashSet::new()), mutable_captures: std::cell::RefCell::new(std::collections::HashSet::new()), async_thunks: std::cell::RefCell::new(Vec::new()), current_fn_error_msg: std::cell::RefCell::new(None), source_path: source_path.to_string() };

    // Header
    if target_so {
        out.push_str("/* Generated by hakic --target so. Build: gcc -shared -fPIC -O2 -lpthread -lm -o handler.so this.c */\n");
    } else {
        out.push_str("/* Generated by hakic --emit-c. Compile: gcc -O2 -lpthread -lm -o out this.c */\n");
    }
    if full_runtime {
        // Unity build: CORE (has http_client under HAKI_HAS_CURL) + HTTP server section
        // CORE_RUNTIME_C_SOURCE is the inline preamble with everything up to HTTP Server
        out.push_str(haki_stdlib::CORE_RUNTIME_C_SOURCE);
        // Append the HTTP-specific section from RUNTIME_C_SOURCE (router, server, etc.)
        let http_section = RUNTIME_C_SOURCE
            .split("/* ── HTTP Server")
            .nth(1)
            .map(|s| format!("/* ── HTTP Server{}", s))
            .unwrap_or_default();
        out.push_str(&http_section);
    } else {
        // Normal build: CORE_RUNTIME_C_SOURCE (everything before HTTP Server section)
        // Use RUNTIME_C_SOURCE up to HTTP Server section, which has map helpers etc.
        let core_runtime = RUNTIME_C_SOURCE
            .split("/* ── HTTP Server")
            .next()
            .unwrap_or(RUNTIME_C_SOURCE);
        out.push_str(core_runtime);
    }
    out.push('\n');

    // Add haki_array_append_val helper used for typed appends
    out.push_str("static void haki_array_append_val(void* arr, void* val) { haki_array_append(arr, val); }\n\n");
    out.push_str("/* ── Tuple structs for multi-return functions ── */\n");
    let mut tuple_arities = std::collections::BTreeSet::new();
    for f in &prog.fns {
        // Check return type — if it's Tuple or a function returning multiple values
        if let SemTy::Tuple(tys) = &f.return_ty {
            tuple_arities.insert(tys.len());
        }
        // Also check by walking return statements in body
        collect_tuple_arities_block(&f.body, &mut tuple_arities);
    }
    // Also collect from enum variant multi-payloads
    for f in &prog.fns {
        collect_tuple_arities_block(&f.body, &mut tuple_arities);
    }
    for n in &tuple_arities {
        out.push_str(&format!("typedef struct {{ "));
        for i in 0..*n {
            out.push_str(&format!("void* f{i}; "));
        }
        out.push_str(&format!("}} __Tuple{n};\n"));
    }
    // Always define Tuple2 (used by multi-return) and Tuple3
    for n in [2usize, 3, 4] {
        if !tuple_arities.contains(&n) {
            out.push_str(&format!("typedef struct {{ "));
            for i in 0..n {
                out.push_str(&format!("void* f{i}; "));
            }
            out.push_str(&format!("}} __Tuple{n};\n"));
        }
    }
    out.push('\n');
    out.push_str("\n/* ── Forward declarations ── */\n");
    for s in &prog.structs {
        out.push_str(&format!("typedef struct {0} {0};\n", c_name(&s.name)));
    }
    for c in &prog.classes {
        out.push_str(&format!("typedef struct {0} {0};\n", c_name(&c.name)));
    }

    // Enum types — represented as { int64_t tag; void* payload; }
    let mut emitted_enums = std::collections::HashSet::new();
    for (ename, _) in &prog.enum_defs {
        let cn = c_name(ename);
        if emitted_enums.insert(cn.clone()) {
            out.push_str(&format!(
                "typedef struct {{ int64_t tag; void* payload; }} {};\n",
                cn
            ));
        }
    }
    out.push('\n');

    // Struct definitions
    out.push_str("/* ── Struct definitions ── */\n");
for s in &prog.structs {
        cx.emit_struct_def(&mut out, s)?;
    }
    for c in &prog.classes {
        cx.emit_class_def(&mut out, c)?;
    }

    // Inject HTTP type definitions BEFORE function prototypes
    // Required for both --target so and regular HttpServer programs
    // Only inject SO_HTTP_TYPES for non-unity builds.
    // Unity builds (full_runtime=true) already have all HTTP types from RUNTIME_C_SOURCE.
    if (target_so || uses_http) && !full_runtime {
        out.push_str(SO_HTTP_TYPES);
    }

    // Forward-declare all functions (so call order doesn't matter)
    // Emit top-level const declarations as C static constants
    if !prog.global_consts.is_empty() {
        out.push_str("/* ── Global constants ── */\n");
        for (name, ty, val_expr) in &prog.global_consts {
            let cx_tmp = Cx {
                prog,
                self_fields: std::cell::RefCell::new(std::collections::HashSet::new()),
                fn_locals: std::cell::RefCell::new(std::collections::HashSet::new()),
                mutable_captures: std::cell::RefCell::new(std::collections::HashSet::new()),
                async_thunks: std::cell::RefCell::new(Vec::new()),
                current_fn_error_msg: std::cell::RefCell::new(None),
                source_path: String::new(),
            };
            let c_type = cx_tmp.c_ty(ty);
            let val_str = cx_tmp.emit_expr(val_expr).unwrap_or_else(|_| "0".into());
            out.push_str(&format!("static const {c_type} {cn} = {val_str};\n",
                cn = c_name(name)));
        }
        out.push('\n');
    }

    out.push_str("/* ── Function prototypes ── */\n");

    // extern "c" functions: emit as C forward declarations so call sites compile.
    // The actual symbol is provided by the linked C library at link time.
    for ef in &prog.extern_fns {
        if ef.abi == "c" {
            out.push_str(&cx.extern_c_prototype(ef)?);
            out.push_str(";\n");
        }
        // extern "js" fns: no declaration needed in C output — they're Wasm-only.
    }

    for f in &prog.fns {
        out.push_str(&cx.fn_prototype(f)?);
        out.push_str(";\n");
    }
    for s in &prog.structs {
        for m in &s.methods {
            out.push_str(&cx.fn_prototype(m)?);
            out.push_str(";\n");
        }
    }
    for c in &prog.classes {
        for m in &c.methods {
            out.push_str(&cx.fn_prototype(m)?);
            out.push_str(";\n");
        }
    }
    for i in &prog.impls {
        for m in &i.methods {
            out.push_str(&cx.fn_prototype(m)?);
            out.push_str(";\n");
        }
    }
    out.push('\n');

    // Function definitions
    out.push_str("/* ── Functions ── */\n");
    for f in &prog.fns {
        // In --target so mode, skip main() — we replace it with haki_handle_request
        if target_so && f.name == "main" { continue; }
        // Emit async thunk definitions before this function:
        cx.async_thunks.borrow_mut().clear();
        let mut fn_buf = String::new();
        cx.emit_fn(&mut fn_buf, f)?;
        for thunk in cx.async_thunks.borrow().iter() {
            out.push_str(thunk);
        }
        cx.async_thunks.borrow_mut().clear();
        out.push_str(&fn_buf);
    }
    for s in &prog.structs {
        for m in &s.methods {
            cx.emit_fn(&mut out, m)?;
        }
    }
    for c in &prog.classes {
        // Set self_fields so emit_var uses self->field for class field accesses
        {
            let mut sf = cx.self_fields.borrow_mut();
            sf.clear();
            for f in &c.fields { sf.insert(f.name.clone()); }
        }
        for m in &c.methods {
            cx.emit_fn(&mut out, m)?;
        }
        cx.self_fields.borrow_mut().clear();
    }
    // Emit impl block methods (protocol implementations)
    for i in &prog.impls {
        // Find the target class to get its field names for self-> injection
        let target_fields: std::collections::HashSet<String> =
            prog.classes.iter()
                .find(|c| c.name == i.target)
                .map(|c| c.fields.iter().map(|f| f.name.clone()).collect())
                .unwrap_or_default();
        {
            let mut sf = cx.self_fields.borrow_mut();
            sf.clear();
            sf.extend(target_fields);
        }
        for m in &i.methods {
            cx.emit_fn(&mut out, m)?;
        }
        cx.self_fields.borrow_mut().clear();
    }

    // For --target so: emit the ABI entry points that mod_haki calls
    if target_so {
        out.push_str(SO_ABI_FOOTER);
    }

    Ok(out)
}

// ── Emitter context ───────────────────────────────────────────────────────────

struct Cx<'a> {
    prog: &'a MonoProgram,
    /// Field names of the class whose method we're currently emitting.
    self_fields: std::cell::RefCell<std::collections::HashSet<String>>,
    /// Parameter names of fn-type in the current function (closure params).
    /// Used to detect fat-pointer calls vs regular calls.
    fn_locals: std::cell::RefCell<std::collections::HashSet<String>>,
    /// Names of mutable captures in the current closure (let bindings captured by ref).
    /// Reads emit (*varname) and writes emit (*varname) = rhs.
    mutable_captures: std::cell::RefCell<std::collections::HashSet<String>>,
    /// Async thunk definitions to emit before the current function body.
    async_thunks: std::cell::RefCell<Vec<String>>,
    /// @error message for the current function being emitted (None if no @error attr).
    /// When set, multi-value returns check the error field and panic rather than return it.
    current_fn_error_msg: std::cell::RefCell<Option<String>>,
    /// Original Haki source path — used for #line directives (empty = disabled).
    source_path: String,
}

impl<'a> Cx<'a> {
    // ── Type mapping ─────────────────────────────────────────────────────────

    fn c_ty(&self, ty: &SemTy) -> String {
        match ty {
            SemTy::Int               => "int64_t".into(),
            SemTy::Float             => "double".into(),
            SemTy::Bool              => "int8_t".into(),
            SemTy::String            => "const char*".into(),
            SemTy::Void              => "void".into(),
            SemTy::Never             => "void".into(),
            SemTy::Named(n)          => {
                match n.as_str() {
                    "Error"     => "void*".into(),  // HakiError* opaque
                    "__env_ptr" => "void*".into(),  // closure env — opaque pointer
                    _           => format!("{}*", c_name(n)),
                }
            }
            SemTy::Generic(n, args) => {
                // Map Haki generic types to their C runtime names
                match n.as_str() {
                    "Array"  => "void*".into(),  // HakiArray* opaque as void*
                    "Map"    => "void*".into(),  // HakiMap* opaque as void*
                    "Chan"   => "void*".into(),  // HakiChan* opaque as void*
                    "Task"      => "void*".into(),
                    "TaskGroup" => "void*".into(),
                    "Mutex"  => "void*".into(),
                    // User-defined generic class: mangle to concrete name
                    // e.g. state__State<int> → state__State__int*
                    _ => {
                        let suffix = args.iter().map(|a| self.c_ty_suffix(a)).collect::<Vec<_>>().join("__");
                        if suffix.is_empty() {
                            format!("{}*", c_name(n))
                        } else {
                            format!("{}__{suffix}*", c_name(n))
                        }
                    }
                }
            }
            SemTy::Optional(inner)   => self.c_ty(inner), // nullable pointer
            SemTy::Tuple(_)          => "void*".into(),   // heap-allocated tuple
            SemTy::Fn(_, _)          => "void*".into(),   // fat pointer
            SemTy::Closure(_, _)     => "void*".into(),   // fat pointer
            SemTy::Var(_)            => "void*".into(),
        }
    }

    /// Returns just the type-arg suffix for mangling generic class names.
    /// e.g. SemTy::Int → "int", SemTy::Named("User") → "User"
    fn c_ty_suffix(&self, ty: &SemTy) -> String {
        match ty {
            SemTy::Int       => "int".into(),
            SemTy::Float     => "f64".into(),
            SemTy::Bool      => "bool".into(),
            SemTy::String    => "string".into(),
            SemTy::Named(n)  => c_name(n).to_string(),
            SemTy::Generic(n, args) => {
                let inner = args.iter().map(|a| self.c_ty_suffix(a)).collect::<Vec<_>>().join("__");
                if inner.is_empty() { c_name(n).to_string() } else { format!("{}__{inner}", c_name(n)) }
            }
            _ => "void".into(),
        }
    }

    fn c_ret_ty(&self, ty: &SemTy) -> String {
        match ty {
            SemTy::Void | SemTy::Never => "void".into(),
            SemTy::Tuple(_)            => "void*".into(), // __TupleN* returned as void*
            other => self.c_ty(other),
        }
    }

    // ── Struct/class definitions ──────────────────────────────────────────────

    fn emit_struct_def(&self, out: &mut String, s: &MonoStruct) -> CResult<()> {
        let cn = c_name(&s.name);
        out.push_str(&format!("struct {} {{\n", cn));
        for f in &s.fields {
            out.push_str(&format!("    {} {};\n", self.c_ty(&f.ty), c_name(&f.name)));
        }
        if s.fields.is_empty() { out.push_str("    int _dummy;\n"); }
        out.push_str("};\n\n");
        Ok(())
    }

    fn emit_class_def(&self, out: &mut String, c: &MonoClass) -> CResult<()> {
        let cn = c_name(&c.name);
        out.push_str(&format!("struct {} {{\n", cn));
        // ARC refcount first
        out.push_str("    int64_t __arc_count;\n");
        // Flatten inherited fields directly (no __super embedding — field access uses ptr->field)
        if let Some(ref sup_name) = c.superclass {
            if let Some(sup) = self.prog.classes.iter().find(|s| s.name == *sup_name) {
                for f in &sup.fields {
                    out.push_str(&format!("    {} {};\n", self.c_ty(&f.ty), c_name(&f.name)));
                }
            }
        }
        for f in &c.fields {
            out.push_str(&format!("    {} {};\n", self.c_ty(&f.ty), c_name(&f.name)));
        }
        if c.fields.is_empty() && c.superclass.is_none() {
            out.push_str("    int _dummy;\n");
        }
        out.push_str("};\n\n");
        Ok(())
    }

    // ── Function prototype ────────────────────────────────────────────────────

    fn fn_prototype(&self, f: &MonoFn) -> CResult<String> {
        let ret = self.c_ret_ty(&f.return_ty);
        let name = if f.name == "main" {
            "int main".to_string()
        } else {
            format!("{} {}", ret, c_name(&f.name))
        };
        let params: Vec<String> = if f.name == "main" {
            vec!["int argc".into(), "char** argv".into()]
        } else {
            f.params.iter()
                .map(|p| format!("{} {}", self.c_ty(&p.ty), c_name(&p.name)))
                .collect()
        };
        let params_str = if params.is_empty() { "void".into() } else { params.join(", ") };
        Ok(format!("{name}({params_str})"))
    }

    /// Emit a C forward declaration for an `extern "c"` function.
    /// e.g. `const char* PQerrorMessage(void* conn)` → the C header equivalent.
    fn extern_c_prototype(&self, f: &haki_ast::ExternFnDef) -> CResult<String> {
        // Map Haki AST types to C type strings
        let ret = match &f.return_ty {
            None => "void".to_string(),
            Some(haki_ast::ReturnTy::Single(ty)) => ast_ty_to_c(&ty.kind),
            Some(haki_ast::ReturnTy::Tuple(_))   => "void*".to_string(),
        };
        let params: Vec<String> = f.params.iter()
            .map(|p| format!("{} {}", ast_ty_to_c(&p.ty.kind), c_name(&p.name.name)))
            .collect();
        let params_str = if params.is_empty() { "void".into() } else { params.join(", ") };
        Ok(format!("extern {} {}({})", ret, f.name.name, params_str))
    }

    // ── Function body ─────────────────────────────────────────────────────────

    /// Emit a `#line N "file"` directive if source_path is set.
    /// This lets gcc/clang embed DWARF debug info pointing to the original .haki file.
    fn emit_line_directive(&self, out: &mut String, span: haki_ast::Span, src: &str) {
        if self.source_path.is_empty() { return; }
        // Convert byte offset to line number
        let lo = span.lo as usize;
        let prefix = &src[..lo.min(src.len())];
        let line_num = prefix.chars().filter(|&c| c == '\n').count() + 1;
        // Escape backslashes in path for C string literal (Windows paths)
        let path = self.source_path.replace('\\', "\\\\");
        out.push_str(&format!("#line {line_num} \"{path}\"\n"));
    }

    fn emit_fn(&self, out: &mut String, f: &MonoFn) -> CResult<()> {
        // Collect fn-type parameter names so emit_call can detect fat-pointer calls.
        // Also collect mutable capture names for pointer-indirection emission.
        {
            let mut locals = self.fn_locals.borrow_mut();
            locals.clear();
            for p in &f.params {
                if matches!(p.ty, SemTy::Fn(_, _) | SemTy::Closure(_, _)) {
                    locals.insert(p.name.clone());
                }
            }
            // Closure variables stored in local scope are also fat-pointer callables
            // (e.g. `const inc = fn[count]() -> void { ... }` makes `inc` a closure var)
            // These appear as local vars with Closure type — we detect them in emit_local_let.
        }
        // Populate mutable_captures for this function
        {
            let mut mut_caps = self.mutable_captures.borrow_mut();
            mut_caps.clear();
            for (cap_name, _, _, is_mut) in &f.captures {
                if *is_mut {
                    mut_caps.insert(cap_name.clone());
                }
            }
        }

        // Emit #line directive before each function so debuggers map to .haki source
        if !self.source_path.is_empty() {
            let lo = f.span.lo as usize;
            // We don't have the raw source here — use span.lo as line approximation
            // The exact line is computed in the driver which has the source string.
            // For now emit a comment; full DWARF mapping is done via emit_line_directive.
            out.push_str(&format!("/* haki span:{} */\n", f.span.lo));
        }
        // Track @error msg for this function (used in return emitter to panic on non-null errors)
        {
            let mut emsg = self.current_fn_error_msg.borrow_mut();
            *emsg = f.attributes.iter()
                .find(|a| a.name == "error")
                .and_then(|a| a.args.first())
                .map(|s| s.clone());
        }
        // Emit annotations as C attributes/pragmas before the function
        for attr in &f.attributes {
            match attr.name.as_str() {
                "inline" => {
                    out.push_str("__attribute__((always_inline)) ");
                }
                "deprecated" => {
                    let msg = attr.args.first().map(|s| s.as_str()).unwrap_or("deprecated");
                    out.push_str(&format!("__attribute__((deprecated(\"{msg}\"))) "));
                }
                _ => {}
            }
        }
        out.push_str(&self.fn_prototype(f)?);
        out.push_str(" {
");
        // main: call haki_runtime_init with argc/argv
        if f.name == "main" {
            out.push_str("    haki_runtime_init(argc, argv);\n");
        }
        // @requires(condition) — emit entry guard immediately after opening brace
        for attr in &f.attributes {
            if attr.name == "requires" {
                if let Some(cond) = attr.args.first() {
                    out.push_str(&format!(
                        "    if (!({cond})) {{ haki_panic(\"@requires({cond}) failed\"); }}\n"
                    ));
                }
            }
        }
        // Closure capture unpacking:
        // For each captured variable, unpack from __env and also expand
        // any class fields of the capture into bare names so the body can
        // access them directly (e.g. `count` from `self.count`).
        if !f.captures.is_empty() {
            for (cap_name, cap_ty, _is_weak, is_mutable) in &f.captures {
                let c_type = self.c_ty(cap_ty);
                let cn = c_name(cap_name);
                if *is_mutable {
                    // Mutable capture: __env is &outer_var (pointer to the outer let binding).
                    // Declare as T* so every read is (*varname) and every write is (*varname)=rhs.
                    // No copy — direct pointer indirection eliminates early-return write-back issues.
                    out.push_str(&format!("    {c_type}* {cn} = ({c_type}*)__env;\n"));
                } else {
                    // Immutable capture: __env IS the value cast to void* (by-value fast path).
                    out.push_str(&format!("    {c_type} {cn} = ({c_type})(intptr_t)__env;\n"));
                }
                // If the capture is a class/struct, also inject its fields as bare names
                let type_name = match cap_ty {
                    SemTy::Named(n) => Some(n.clone()),
                    _ => None,
                };
                if let Some(ref tname) = type_name {
                    if let Some(cls) = self.prog.classes.iter().find(|c| c.name == *tname) {
                        for field in &cls.fields {
                            let fty = self.c_ty(&field.ty);
                            out.push_str(&format!(
                                "    {fty} {fname} = {cn}->{fname};\n",
                                fname = c_name(&field.name),
                                cn = cn.as_str(),
                            ));
                        }
                    }
                }
            }
        }
        // Emit deferred expressions tracked during block emission
        let mut deferred: Vec<String> = Vec::new();
        self.emit_block(out, &f.body, 1, &mut deferred)?;
        // If the function returns void and doesn't have an explicit return,
        // emit the deferred expressions at the end.
        if matches!(f.return_ty, SemTy::Void | SemTy::Never) {
            for d in &deferred {
                out.push_str(&format!("    {};\n", d));
            }
            if f.name == "main" {
                out.push_str("    return 0;\n");
            }
        }
        out.push_str("}\n\n");
        Ok(())
    }

    // ── Block / statement emission ────────────────────────────────────────────

    fn emit_block(
        &self,
        out: &mut String,
        block: &MonoBlock,
        depth: usize,
        deferred: &mut Vec<String>,
    ) -> CResult<()> {
        let indent = "    ".repeat(depth);
        for stmt in &block.stmts {
            self.emit_stmt(out, stmt, depth, deferred, &indent)?;
        }
        Ok(())
    }

    fn emit_stmt(
        &self,
        out: &mut String,
        stmt: &MonoStmt,
        depth: usize,
        deferred: &mut Vec<String>,
        indent: &str,
    ) -> CResult<()> {
        match &stmt.kind {
            MonoStmtKind::Let(l) => self.emit_let(out, l, indent)?,

            MonoStmtKind::Return(r) => {
                // Emit deferred in LIFO order before return
                for d in deferred.iter().rev() {
                    out.push_str(&format!("{indent}{};\n", d));
                }
                match r.values.len() {
                    0 => out.push_str(&format!("{indent}return;\n")),
                    1 => {
                        let v = self.emit_expr(&r.values[0])?;
                        out.push_str(&format!("{indent}return {};\n", v));
                    }
                    _ => {
                        // Multi-return: heap-allocate a __TupleN struct
                        let n = r.values.len();
                        out.push_str(&format!(
                            "{indent}__Tuple{n}* __ret = (__Tuple{n}*)malloc(sizeof(__Tuple{n}));\n"
                        ));
                        for (i, v) in r.values.iter().enumerate() {
                            let ve = self.emit_expr(v)?;
                            let vty = self.c_ty(&v.ty);
                            // Pointer-sized values (void*, strings, structs, error) store directly.
                            // Scalar values (int64_t, float, bool) need a malloc wrapper.
                            let is_scalar = matches!(v.ty, SemTy::Int | SemTy::Float | SemTy::Bool);
                            if is_scalar {
                                out.push_str(&format!(
                                    "{indent}{{ {vty}* __f{i} = ({vty}*)malloc(sizeof({vty})); *__f{i} = {ve}; __ret->f{i} = __f{i}; }}\n"
                                ));
                            } else {
                                // Pointer types: store directly (NULL-safe)
                                out.push_str(&format!("{indent}__ret->f{i} = (void*)({ve});\n"));
                            }
                        }
                        // @error "msg" — panic before returning a non-null error (f1 slot)
                        if n >= 2 {
                            if let Some(ref emsg) = *self.current_fn_error_msg.borrow() {
                                if emsg.contains("{err}") {
                                    let prefix = emsg.replace("{err}", "");
                                    let prefix_escaped = prefix.replace('"', "\\\"");
                                    out.push_str(&format!(
                                        "{indent}if (__ret->f1 != NULL) {{ char __err_buf[512]; snprintf(__err_buf, sizeof(__err_buf), \"{prefix_escaped}%s\", (char*)__ret->f1); haki_panic(__err_buf); }}\n"
                                    ));
                                } else {
                                    let escaped = emsg.replace('"', "\\\"");
                                    out.push_str(&format!(
                                        "{indent}if (__ret->f1 != NULL) {{ haki_panic(\"{escaped}\"); }}\n"
                                    ));
                                }
                            }
                        }
                        out.push_str(&format!("{indent}return __ret;\n"));
                    }
                }
            }

            MonoStmtKind::Yield(_) => {} // handled by block-as-expr in caller

            MonoStmtKind::Defer(e) => {
                let es = self.emit_expr(e)?;
                deferred.push(es);
            }

            MonoStmtKind::Continue => out.push_str(&format!("{indent}continue;\n")),
            MonoStmtKind::Break    => out.push_str(&format!("{indent}break;\n")),

            MonoStmtKind::If(i) => {
                let cond_raw = self.emit_expr(&i.cond)?;
                // Strip redundant outer parens to avoid -Wparentheses warnings
                let cond = if cond_raw.starts_with('(') && cond_raw.ends_with(')') {
                    cond_raw[1..cond_raw.len()-1].to_string()
                } else { cond_raw };
                out.push_str(&format!("{indent}if ({cond}) {{
"));
                self.emit_block(out, &i.then_block, depth + 1, deferred)?;
                out.push_str(&format!("{indent}}}\n"));
                if let Some(els) = &i.else_branch {
                    out.push_str(&format!("{indent}else {{\n"));
                    match els {
                        MonoElse::Block(b) => self.emit_block(out, b, depth + 1, deferred)?,
                        MonoElse::If(inner) => {
                            let inner_stmt = MonoStmt {
                                kind: MonoStmtKind::If(*inner.clone()),
                                span: stmt.span,
                            };
                            self.emit_stmt(out, &inner_stmt, depth + 1, deferred,
                                          &"    ".repeat(depth + 1))?;
                        }
                    }
                    out.push_str(&format!("{indent}}}\n"));
                }
            }

            MonoStmtKind::While(w) => {
                let cond_raw = self.emit_expr(&w.cond)?;
                let cond = if cond_raw.starts_with('(') && cond_raw.ends_with(')') {
                    cond_raw[1..cond_raw.len()-1].to_string()
                } else { cond_raw };
                out.push_str(&format!("{indent}while ({cond}) {{\n"));
                self.emit_block(out, &w.body, depth + 1, deferred)?;
                out.push_str(&format!("{indent}}}\n"));
            }

            MonoStmtKind::For(f) => {
                let is_chan = matches!(&f.var_ty, SemTy::Generic(n, _) if n == "Chan")
                    || matches!(&f.iter.ty, SemTy::Generic(n, _) if n == "Chan");

                // ── Map iteration: for k, v in map { } ──────────────────────────
                let is_map = matches!(&f.iter.ty, SemTy::Generic(n, _) if n == "Map");
                if is_map {
                    // for k, v in map — uses haki_map_capacity/entry_key/entry_value
                    let map_expr = self.emit_expr(&f.iter)?;
                    let map_var  = format!("__map_{}", c_name(&f.var.name));
                    let idx_var  = format!("__mi_{}", c_name(&f.var.name));
                    let val_ty   = self.c_ty(&f.var_ty);
                    let var_c    = c_name(&f.var.name);

                    out.push_str(&format!("{indent}{{ void* {map_var} = (void*)({map_expr});
"));
                    out.push_str(&format!(
                        "{indent}    int64_t __cap_{var_c} = haki_map_capacity({map_var});
"
                    ));
                    out.push_str(&format!(
                        "{indent}    for (int64_t {idx_var} = 0; {idx_var} < __cap_{var_c}; {idx_var}++) {{
"
                    ));
                    out.push_str(&format!(
                        "{indent}        const char* __key_{var_c} = haki_map_entry_key({map_var}, {idx_var});
"
                    ));
                    out.push_str(&format!("{indent}        if (!__key_{var_c}) continue;
"));
                    if let Some(ref kv) = f.index_var {
                        let key_c = c_name(&kv.name);
                        out.push_str(&format!(
                            "{indent}        const char* {key_c} = __key_{var_c};
"
                        ));
                    }
                    // Value cast depends on type
                    let val_cast = match &f.var_ty {
                        SemTy::Int   => format!("({val_ty})(intptr_t)haki_map_entry_value({map_var}, {idx_var})"),
                        SemTy::Bool  => format!("({val_ty})(intptr_t)haki_map_entry_value({map_var}, {idx_var})"),
                        SemTy::Float => format!("*({val_ty}*)haki_map_entry_value({map_var}, {idx_var})"),
                        _            => format!("({val_ty})haki_map_entry_value({map_var}, {idx_var})"),
                    };
                    out.push_str(&format!("{indent}        {val_ty} {var_c} = {val_cast};
"));
                    self.emit_block(out, &f.body, depth + 2, deferred)?;
                    out.push_str(&format!("{indent}    }}
"));
                    out.push_str(&format!("{indent}}}
"));
                } else if is_chan {
                    // for msg in ch { body }
                    // → { void* __ch = ch;
                    //     while (1) {
                    //       ElemTy msg = (ElemTy)haki_chan_receive(__ch);
                    //       if (msg == NULL && haki_chan_is_closed(__ch)) break;
                    //       body
                    //     } }
                    let ch_expr = self.emit_expr(&f.iter)?;
                    let ch_var  = format!("__ch_{}", c_name(&f.var.name));
                    let elem_ty = self.c_ty(&match &f.var_ty {
                        SemTy::Generic(_, args) if !args.is_empty() => args[0].clone(),
                        other => other.clone(),
                    });
                    let var_c   = c_name(&f.var.name);
                    out.push_str(&format!("{indent}{{ void* {ch_var} = (void*)({ch_expr});\n"));
                    out.push_str(&format!("{indent}    while (1) {{\n"));
                    out.push_str(&format!(
                        "{indent}        void* __raw_{var_c} = haki_chan_receive({ch_var});\n"
                    ));
                    out.push_str(&format!(
                        "{indent}        if (__raw_{var_c} == NULL && haki_chan_is_closed({ch_var})) break;\n"
                    ));
                    out.push_str(&format!(
                        "{indent}        {elem_ty} {var_c} = ({elem_ty})__raw_{var_c};\n"
                    ));
                    self.emit_block(out, &f.body, depth + 2, deferred)?;
                    out.push_str(&format!("{indent}    }}\n"));
                    out.push_str(&format!("{indent}}}\n"));
                } else {
                    // Standard array for loop
                    let arr = self.emit_expr(&f.iter)?;
                    let idx_var = format!("__i_{}", c_name(&f.var.name));
                    let arr_var = format!("__arr_{}", c_name(&f.var.name));
                    let elem_ty = self.c_ty(&f.var_ty);

                    out.push_str(&format!("{indent}{{ void* {arr_var} = {arr};\n"));
                    out.push_str(&format!(
                        "{indent}    int64_t __len_{0} = haki_array_length({arr_var});\n",
                        c_name(&f.var.name)
                    ));
                    out.push_str(&format!(
                        "{indent}    for (int64_t {idx_var} = 0; {idx_var} < __len_{0}; {idx_var}++) {{\n",
                        c_name(&f.var.name)
                    ));
                    out.push_str(&format!(
                        "{indent}        {elem_ty} {0} = *({elem_ty}*)haki_array_get({arr_var}, {idx_var});\n",
                        c_name(&f.var.name)
                    ));

                    if let Some(ref iv) = f.index_var {
                        out.push_str(&format!(
                            "{indent}        int64_t {} = {idx_var};\n",
                            c_name(&iv.name)
                        ));
                    }

                    self.emit_block(out, &f.body, depth + 2, deferred)?;
                    out.push_str(&format!("{indent}    }}\n"));
                    out.push_str(&format!("{indent}}}\n"));
                }
            }

            MonoStmtKind::Match(m) => {
                let scrutinee = self.emit_expr(&m.scrutinee)?;
                match m.kind {
                    MonoMatchKind::Int => {
                        // Use if-else chain when any arm has a guard (guards can't express
                        // fallthrough in C switch). Use switch when no guards are present.
                        let has_any_guard = m.arms.iter().any(|a| a.guard.is_some());
                        if has_any_guard {
                            // If-else chain: `_ if cond` → `else if (cond)`, `_` → `else`
                            let sc_var = format!("__sc_int_{}", depth);
                            out.push_str(&format!("{indent}{{ int64_t {sc_var} = (int64_t)({scrutinee});\n"));
                            let mut first = true;
                            for arm in &m.arms {
                                let kw = if first { "if" } else { "} else if" };
                                match &arm.pattern {
                                    MonoPattern::Int(n) => {
                                        first = false;
                                        if let Some(g) = &arm.guard {
                                            let gc = self.emit_expr(g)?;
                                            out.push_str(&format!("{indent}    {kw} ({sc_var} == {n}LL && ({gc})) {{\n"));
                                        } else {
                                            out.push_str(&format!("{indent}    {kw} ({sc_var} == {n}LL) {{\n"));
                                        }
                                        self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                    }
                                    MonoPattern::Named(s) if s == "_" => {
                                        if let Some(g) = &arm.guard {
                                            let gc = self.emit_expr(g)?;
                                            if first { first = false; out.push_str(&format!("{indent}    if ({gc}) {{\n")); }
                                            else { out.push_str(&format!("{indent}    }} else if ({gc}) {{\n")); }
                                        } else {
                                            if first { first = false; out.push_str(&format!("{indent}    {{\n")); }
                                            else { out.push_str(&format!("{indent}    }} else {{\n")); }
                                        }
                                        self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                    }
                                    _ => {}
                                }
                            }
                            out.push_str(&format!("{indent}    }}\n{indent}}}\n"));
                        } else {
                            // No guards — standard C switch statement
                            out.push_str(&format!("{indent}switch ((int64_t)({scrutinee})) {{\n"));
                            let mut has_default = false;
                            for arm in &m.arms {
                                match &arm.pattern {
                                    MonoPattern::Int(n) => {
                                        out.push_str(&format!("{indent}    case {n}LL: {{\n"));
                                        self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                        out.push_str(&format!("{indent}    break; }}\n"));
                                    }
                                    MonoPattern::Named(s) if s == "_" => {
                                        has_default = true;
                                        out.push_str(&format!("{indent}    default: {{\n"));
                                        self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                        out.push_str(&format!("{indent}    break; }}\n"));
                                    }
                                    _ => {}
                                }
                            }
                            if !has_default {
                                out.push_str(&format!("{indent}    default: break;\n"));
                            }
                            out.push_str(&format!("{indent}}}\n"));
                        }
                    }
                    MonoMatchKind::String => {
                        // String match → if-else strcmp chain (guard merged into condition)
                        let mut first = true;
                        for arm in &m.arms {
                            match &arm.pattern {
                                MonoPattern::String(s) => {
                                    let kw = if first { "if" } else { "} else if" };
                                    first = false;
                                    if let Some(g) = &arm.guard {
                                        let gc = self.emit_expr(g)?;
                                        out.push_str(&format!(
                                            "{indent}{kw} (strcmp((const char*)({scrutinee}), \"{s}\") == 0 && ({gc})) {{\n"
                                        ));
                                    } else {
                                        out.push_str(&format!(
                                            "{indent}{kw} (strcmp((const char*)({scrutinee}), \"{s}\") == 0) {{\n"
                                        ));
                                    }
                                    self.emit_block(out, &arm.body, depth + 1, deferred)?;
                                }
                                MonoPattern::Named(s) if s == "_" => {
                                    let kw = if first { "" } else { "} else " };
                                    first = false;
                                    if let Some(g) = &arm.guard {
                                        let gc = self.emit_expr(g)?;
                                        out.push_str(&format!("{indent}{kw}if ({gc}) {{\n"));
                                    } else {
                                        out.push_str(&format!("{indent}{kw}{{\n"));
                                    }
                                    self.emit_block(out, &arm.body, depth + 1, deferred)?;
                                }
                                _ => {}
                            }
                        }
                        out.push_str(&format!("{indent}}}
"));
                    }
                    MonoMatchKind::Enum => {
                        let sc_var = "__sc".to_string();
                        out.push_str(&format!("{indent}{{ void* {sc_var} = (void*){scrutinee};\n"));
                        out.push_str(&format!("{indent}    int64_t __tag = ((int64_t*){sc_var})[0];\n"));
                        out.push_str(&format!("{indent}    void* __payload = ((void**){sc_var})[1];\n"));

                        // Group consecutive arms that share the same discriminant so that
                        // guarded and unguarded arms on the same variant chain correctly:
                        //   Good(v) if v > 100 { ... }    ← guarded
                        //   Good(v) { ... }               ← fallthrough unguarded
                        // Both go inside a single `if (__tag == disc) { ... }` block.
                        let mut group_start = 0usize;
                        let mut first_disc_block = true;
                        while group_start < m.arms.len() {
                            let disc_key = match &m.arms[group_start].pattern {
                                MonoPattern::Named(s) if s == "_" => None, // wildcard
                                MonoPattern::Named(pname) => Some(self.variant_discriminant_by_name(pname)),
                                _ => Some(0usize), // Int/String shouldn't appear here
                            };
                            // Collect all consecutive arms sharing the same discriminant
                            let group_end = {
                                let mut end = group_start + 1;
                                while end < m.arms.len() {
                                    let next_key = match &m.arms[end].pattern {
                                        MonoPattern::Named(s) if s == "_" => None,
                                        MonoPattern::Named(pname) => Some(self.variant_discriminant_by_name(pname)),
                                        _ => Some(0usize),
                                    };
                                    if next_key == disc_key && disc_key.is_some() {
                                        end += 1;
                                    } else {
                                        break;
                                    }
                                }
                                end
                            };

                            // Emit outer discriminant check
                            let outer_kw = if first_disc_block { "if" } else { "} else if" };
                            first_disc_block = false;
                            match disc_key {
                                None => { out.push_str(&format!("{indent}    }} else {{\n")); }
                                Some(disc) => {
                                    out.push_str(&format!("{indent}    {outer_kw} (__tag == {disc}LL) {{\n"));
                                }
                            }

                            // Emit bindings (shared across all arms in this group)
                            let first_arm = &m.arms[group_start];
                            if first_arm.bindings.len() == 1 {
                                let bt = self.c_ty(&first_arm.binding_tys[0]);
                                out.push_str(&format!(
                                    "{indent}        {bt} {} = *({bt}*)__payload;\n",
                                    c_name(&first_arm.bindings[0].name)
                                ));
                            } else if first_arm.bindings.len() > 1 {
                                for (bi, (binding, bty)) in first_arm.bindings.iter().zip(first_arm.binding_tys.iter()).enumerate() {
                                    let bt = self.c_ty(bty);
                                    out.push_str(&format!(
                                        "{indent}        {bt} {} = *({bt}*)(((void**)__payload)[{bi}]);\n",
                                        c_name(&binding.name)
                                    ));
                                }
                            }

                            let group = &m.arms[group_start..group_end];
                            if group.len() == 1 {
                                // Single arm in group — guard wraps just the body
                                let arm = &group[0];
                                if let Some(g) = &arm.guard {
                                    let gc = self.emit_expr(g)?;
                                    out.push_str(&format!("{indent}        if ({gc}) {{\n"));
                                    self.emit_block(out, &arm.body, depth + 3, deferred)?;
                                    out.push_str(&format!("{indent}        }}\n"));
                                } else {
                                    self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                }
                            } else {
                                // Multiple arms with same discriminant — chain guards as if-else
                                let mut inner_first = true;
                                for arm in group {
                                    // Re-extract bindings per arm if names differ
                                    if arm.bindings.len() == 1
                                        && arm.bindings[0].name != first_arm.bindings.first().map(|b| b.name.as_str()).unwrap_or("")
                                    {
                                        let bt = self.c_ty(&arm.binding_tys[0]);
                                        out.push_str(&format!(
                                            "{indent}        {bt} {} = *({bt}*)__payload;\n",
                                            c_name(&arm.bindings[0].name)
                                        ));
                                    }
                                    if let Some(g) = &arm.guard {
                                        let gc = self.emit_expr(g)?;
                                        let kw = if inner_first { "if" } else { "} else if" };
                                        inner_first = false;
                                        out.push_str(&format!("{indent}        {kw} ({gc}) {{\n"));
                                        self.emit_block(out, &arm.body, depth + 3, deferred)?;
                                    } else {
                                        let kw = if inner_first { "" } else { "} else {" };
                                        inner_first = false;
                                        if kw.is_empty() {
                                            out.push_str(&format!("{indent}        {{\n"));
                                            self.emit_block(out, &arm.body, depth + 3, deferred)?;
                                        } else {
                                            out.push_str(&format!("{indent}        {kw}\n"));
                                            self.emit_block(out, &arm.body, depth + 3, deferred)?;
                                        }
                                    }
                                }
                                out.push_str(&format!("{indent}        }}\n")); // close last if/else
                            }

                            group_start = group_end;
                        }
                        out.push_str(&format!("{indent}    }}\n{indent}}}\n"));
                    }
                    MonoMatchKind::Class => {
                        // Class hierarchy match — emit first arm as fallback
                        out.push_str(&format!("{indent}/* class match */
"));
                        if let Some(arm) = m.arms.first() {
                            self.emit_block(out, &arm.body, depth + 1, deferred)?;
                        }
                    }
                }
            }

            MonoStmtKind::Select(sel) => {
                // select { binding = ch.receive() { body } timeout(ms) { body } }
                // Lowers to:
                //   HakiChan* __sel_chans[N] = { ch0, ch1, ... };
                //   int __sel_ops[N]  = { 0, 0, ... };    // 0=recv
                //   void* __sel_vals[N] = { NULL, ... };
                //   int __sel_ready = haki_select(N, __sel_chans, __sel_ops, __sel_vals);
                //   if (__sel_ready == 0) { ElemTy binding = (ElemTy)__sel_vals[0]; body }
                //   else if (__sel_ready == 1) { ... }
                //   else { /* all closed */ }
                let n = sel.arms.len();
                let uid = sel.span.lo;

                if n == 0 {
                    // Empty select — no-op
                } else {
                    out.push_str(&format!("{indent}{{\n"));
                    // Declare channel array
                    out.push_str(&format!(
                        "{indent}    void* __sel_chans_{uid}[{n}];\n"
                    ));
                    out.push_str(&format!(
                        "{indent}    int __sel_ops_{uid}[{n}];\n"
                    ));
                    out.push_str(&format!(
                        "{indent}    void* __sel_vals_{uid}[{n}];\n"
                    ));

                    // Fill channel + op arrays
                    // ch_expr is `ch.recv()` — extract the channel pointer (first arg of recv call)
                    for (i, (_, _, ch_expr, _)) in sel.arms.iter().enumerate() {
                        let ch = if let MonoExprKind::Call(fname, fargs) = &ch_expr.kind {
                            if (fname.contains("chan_receive") || fname.contains("__recv") || fname.contains("__receive"))
                               && !fargs.is_empty() {
                                self.emit_expr(&fargs[0])?
                            } else {
                                self.emit_expr(ch_expr)?
                            }
                        } else {
                            self.emit_expr(ch_expr)?
                        };
                        out.push_str(&format!(
                            "{indent}    __sel_chans_{uid}[{i}] = (void*)({ch});\n"
                        ));
                        out.push_str(&format!(
                            "{indent}    __sel_ops_{uid}[{i}] = 0;\n"  // 0=receive
                        ));
                        out.push_str(&format!(
                            "{indent}    __sel_vals_{uid}[{i}] = NULL;\n"
                        ));
                    }

                    // Call haki_select — pass timeout_ms (-1 = block forever)
                    let timeout_ms_str = if let Some((ms_expr, _)) = &sel.timeout {
                        self.emit_expr(ms_expr).unwrap_or_else(|_| "-1".into())
                    } else {
                        "-1".into()
                    };
                    out.push_str(&format!(
                        "{indent}    int __sel_ready_{uid} = haki_select({n}, (HakiChan**)__sel_chans_{uid}, __sel_ops_{uid}, __sel_vals_{uid}, (int64_t)({timeout_ms_str}));\n"
                    ));

                    // Dispatch arms
                    for (i, (binding, bind_ty, _, body)) in sel.arms.iter().enumerate() {
                        let kw = if i == 0 { "if" } else { "else if" };
                        let elem_ty = self.c_ty(bind_ty);
                        let bname   = c_name(&binding.name);
                        out.push_str(&format!(
                            "{indent}    {kw} (__sel_ready_{uid} == {i}) {{\n"
                        ));
                        out.push_str(&format!(
                            "{indent}        {elem_ty} {bname} = ({elem_ty})__sel_vals_{uid}[{i}];\n"
                        ));
                        self.emit_block(out, body, depth + 2, deferred)?;
                        out.push_str(&format!("{indent}    }}\n"));
                    }

                    // Timeout arm — fires when haki_select returns -1
                    if let Some((_, timeout_body)) = &sel.timeout {
                        out.push_str(&format!(
                            "{indent}    else if (__sel_ready_{uid} == -1) {{\n"
                        ));
                        self.emit_block(out, timeout_body, depth + 2, deferred)?;
                        out.push_str(&format!("{indent}    }}\n"));
                    }

                    out.push_str(&format!("{indent}}}\n"));
                }
            }

            MonoStmtKind::Panic(msg) => {
                let ms = self.emit_expr(msg)?;
                out.push_str(&format!("{indent}haki_panic({ms});\n"));
            }

            MonoStmtKind::Expr(e) => {
                let es = self.emit_expr(e)?;
                if !es.is_empty() {
                    // Field/var expressions as statements get (void) cast to suppress
                    // -Wunused-value warnings (e.g. `a->name;` → `(void)(a->name);`)
                    let is_pure_access = matches!(e.kind,
                        MonoExprKind::Field(_, _) | MonoExprKind::Var(_)
                    );
                    if is_pure_access {
                        out.push_str(&format!("{indent}(void)({es});
"));
                    } else {
                        out.push_str(&format!("{indent}({es});
"));
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_let(&self, out: &mut String, l: &MonoLetStmt, indent: &str) -> CResult<()> {
        // Special case: Construct literals (struct/class construction) need sequential statements
        if let MonoExprKind::Construct(type_name, named_args) = &l.init.kind {
            // Error construction → haki_error_new(message)
            if type_name == "Error" {
                if let [(Binding::Name(id), _)] = l.bindings.as_slice() {
                    let nm = c_name(&id.name);
                    if nm != "_" {
                        let msg = named_args.iter()
                            .find(|a| a.name == "message")
                            .map(|a| self.emit_expr(&a.value))
                            .transpose()?
                            .unwrap_or_else(|| "\"error\"".into());
                        let cause = named_args.iter()
                            .find(|a| a.name == "cause");
                        let call = if let Some(c) = cause {
                            let cv = self.emit_expr(&c.value)?;
                            format!("haki_error_new_with_cause({msg}, {cv})")
                        } else {
                            format!("haki_error_new({msg})")
                        };
                        out.push_str(&format!("{indent}void* {nm} = {call};\n"));
                        return Ok(());
                    }
                }
            }
            if let [(Binding::Name(id), ty)] = l.bindings.as_slice() {
                let nm = c_name(&id.name);
                if nm != "_" {
                    let cn = c_name(type_name);
                    let ct = self.c_ty(ty);
                    let is_class = self.prog.classes.iter().any(|c| c.name == *type_name);
                    let alloc = if is_class {
                        format!("({cn}*)haki_alloc(sizeof({cn}))")
                    } else {
                        format!("({cn}*)malloc(sizeof({cn}))")
                    };
                    out.push_str(&format!("{indent}{ct} {nm} = {alloc};\n"));
                    if is_class {
                        out.push_str(&format!("{indent}{nm}->__arc_count = 1;\n"));
                    }
                    // Collect field types for pointer-to-int casting
                    let let_field_types: std::collections::HashMap<String, SemTy> =
                        self.prog.classes.iter()
                            .find(|c| c.name == *type_name)
                            .map(|c| c.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect())
                            .or_else(|| self.prog.structs.iter()
                                .find(|s| s.name == *type_name)
                                .map(|s| s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()))
                            .unwrap_or_default();

                    for na in named_args {
                        let ve = self.emit_expr(&na.value)?;
                        let field_ty = let_field_types.get(&na.name);
                        let cast_ve = match field_ty {
                            Some(SemTy::Int) if na.value.ty != SemTy::Int
                                && na.value.ty != SemTy::Float
                                && na.value.ty != SemTy::Bool => {
                                format!("(int64_t)(void*)({ve})")
                            }
                            _ => ve,
                        };
                        out.push_str(&format!("{indent}{nm}->{} = {cast_ve};\n", c_name(&na.name)));
                    }
                    return Ok(());
                }
            }
        }

        // Special case: array literals
        if let MonoExprKind::Array(elems) = &l.init.kind {
            if let [(Binding::Name(id), ty)] = l.bindings.as_slice() {
                let nm = c_name(&id.name);
                if nm != "_" {
                    let elem_ty = match ty {
                        SemTy::Generic(_, args) if !args.is_empty() => self.c_ty(&args[0]),
                        _ => "void*".into(),
                    };
                    out.push_str(&format!("{indent}void* {nm} = haki_array_new({});\n", array_elem_size(&elem_ty)));
                    for el in elems {
                        let ev = self.emit_expr(el)?;
                        out.push_str(&format!(
                            "{indent}{{ {elem_ty} __el = ({ev}); haki_array_append({nm}, &__el); }}\n"
                        ));
                    }
                    return Ok(());
                }
            }
        }

        let init = self.emit_expr(&l.init)?;
        match l.bindings.as_slice() {
            [(Binding::Name(id), ty)] => {
                let ct = self.c_ty(ty);
                let nm = c_name(&id.name);
                if nm == "_" {
                    out.push_str(&format!("{indent}(void)({init});\n"));
                } else {
                    out.push_str(&format!("{indent}{ct} {nm} = {init};\n"));
                    // Register closure-typed locals in fn_locals so calls use fat-pointer emission.
                    // The binding ty may be void* (from fat-pointer ABI), so also check
                    // if the init expression is a haki_make_closure call.
                    if matches!(ty, SemTy::Closure(_, _) | SemTy::Fn(_, _))
                        || matches!(l.init.ty, SemTy::Closure(_, _) | SemTy::Fn(_, _))
                        || init.starts_with("haki_make_closure(")
                    {
                        self.fn_locals.borrow_mut().insert(id.name.clone());
                    }
                }
            }
            bindings if bindings.len() > 1 => {
                let n = bindings.len();
                let mb_name = format!("__mb_{}", l.span.lo);
                out.push_str(&format!("{indent}__Tuple{n}* {mb_name} = (__Tuple{n}*)({init});\n"));
                for (i, (binding, ty)) in bindings.iter().enumerate() {
                    if let Binding::Name(id) = binding {
                        let ct = self.c_ty(ty);
                        let nm = c_name(&id.name);
                        if nm != "_" {
                            let is_scalar = matches!(ty, SemTy::Int | SemTy::Float | SemTy::Bool);
                            if is_scalar {
                                // Scalar: f{i} points to heap-allocated value
                                out.push_str(&format!(
                                    "{indent}{ct} {nm} = *({ct}*){mb_name}->f{i};\n"
                                ));
                            } else {
                                // Pointer: f{i} IS the value (may be NULL)
                                out.push_str(&format!(
                                    "{indent}{ct} {nm} = ({ct}){mb_name}->f{i};\n"
                                ));
                            }
                        }
                    }
                }
            }
            _ => {
                out.push_str(&format!("{indent}(void)({init});\n"));
            }
        }
        Ok(())
    }

    // ── Expression emission ───────────────────────────────────────────────────

    fn emit_expr(&self, e: &MonoExpr) -> CResult<String> {
        match &e.kind {
            MonoExprKind::Int(n) => Ok(format!("((int64_t){n}LL)")),
            MonoExprKind::Float(f) => Ok(format!("{f}")),
            MonoExprKind::Bool(b) => Ok(if *b { "1".into() } else { "0".into() }),
            MonoExprKind::String(s) => Ok(format!("\"{}\"", escape_c_string(s))),
            MonoExprKind::Null => Ok("NULL".into()),
            MonoExprKind::Var(name) => Ok(self.emit_var(name, &e.ty)),

            MonoExprKind::Unary(op, inner) => {
                let ie = self.emit_expr(inner)?;
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                Ok(format!("({op_str}{ie})"))
            }

            MonoExprKind::Binary(op, l, r) => {
                // String concatenation handled specially
                if matches!(op, BinaryOp::Add) && matches!(l.ty, SemTy::String) {
                    let ls = self.emit_expr(l)?;
                    let rs = self.emit_expr(r)?;
                    return Ok(format!("haki_string_concat({ls}, {rs})"));
                }
                // Optional<int/bool/float> != null / == null → use 0 as null sentinel
                // (int64_t values stored as non-pointer, so NULL comparison is wrong)
                let is_prim_optional = |ty: &SemTy| matches!(ty,
                    SemTy::Optional(inner) if matches!(inner.as_ref(), SemTy::Int | SemTy::Float | SemTy::Bool)
                );
                if (matches!(op, BinaryOp::Eq) || matches!(op, BinaryOp::Ne))
                    && (matches!(r.kind, MonoExprKind::Null) || matches!(l.kind, MonoExprKind::Null))
                    && (is_prim_optional(&l.ty) || is_prim_optional(&r.ty))
                {
                    let (val_expr, val_ty) = if matches!(r.kind, MonoExprKind::Null) {
                        (self.emit_expr(l)?, &l.ty)
                    } else {
                        (self.emit_expr(r)?, &r.ty)
                    };
                    let null_val = match val_ty {
                        SemTy::Optional(inner) if matches!(inner.as_ref(), SemTy::Float) => "(double)0.0",
                        _ => "0",
                    };
                    let cmp = if matches!(op, BinaryOp::Eq) { "==" } else { "!=" };
                    return Ok(format!("({val_expr} {cmp} {null_val})"));
                }
                // String equality
                if (matches!(op, BinaryOp::Eq) || matches!(op, BinaryOp::Ne))
                    && matches!(l.ty, SemTy::String) {
                    let ls = self.emit_expr(l)?;
                    let rs = self.emit_expr(r)?;
                    let cmp = format!("(strcmp({ls}, {rs}) == 0)");
                    return Ok(if matches!(op, BinaryOp::Ne) {
                        format!("(!{cmp})")
                    } else {
                        cmp
                    });
                }
                let le_raw = self.emit_expr(l)?;
                let re_raw = self.emit_expr(r)?;
                // Mixed int/f64: cast the int operand to double for C
                let (le, re) = match (&l.ty, &r.ty) {
                    (SemTy::Float, SemTy::Int)  => (le_raw, format!("(double)({re_raw})")),
                    (SemTy::Int,   SemTy::Float) => (format!("(double)({le_raw})"), re_raw),
                    _ => (le_raw, re_raw),
                };
                let op_str = match op {
                    BinaryOp::Add => "+",  BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",  BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Eq  => "==", BinaryOp::Ne  => "!=",
                    BinaryOp::Lt  => "<",  BinaryOp::Le  => "<=",
                    BinaryOp::Gt  => ">",  BinaryOp::Ge  => ">=",
                    BinaryOp::And => "&&", BinaryOp::Or  => "||",
                };
                Ok(format!("({le} {op_str} {re})"))
            }

            MonoExprKind::Field(recv, field) => {
                let re = self.emit_expr(recv)?;
                // Error fields go through runtime accessors
                if matches!(recv.ty, SemTy::Named(ref n) if n == "Error")
                    || matches!(recv.ty, SemTy::Optional(ref inner) if matches!(**inner, SemTy::Named(ref n) if n == "Error"))
                {
                    return Ok(match field.as_str() {
                        "message" => format!("haki_error_message({re})"),
                        "cause"   => format!("haki_error_cause({re})"),
                        _         => format!("/* unknown Error field {field} */NULL"),
                    });
                }
                // .length on string/Array/Map → runtime call
                if field == "length" {
                    match &recv.ty {
                        SemTy::String =>
                            return Ok(format!("((int64_t)strlen({re}))")),
                        SemTy::Generic(n, _) if n == "Array" =>
                            return Ok(format!("haki_array_length({re})")),
                        SemTy::Generic(n, _) if n == "Map" =>
                            return Ok(format!("haki_map_length({re})")),
                        // void* arrays (e.g. from argv()) — use array length
                        SemTy::Named(n) if n == "Array" =>
                            return Ok(format!("haki_array_length({re})")),
                        _ => {}
                    }
                }
                Ok(format!("{re}->{}", c_name(field)))
            }

            // Optional chaining: `recv?.field` → `(recv != NULL ? recv->field : NULL)`
            MonoExprKind::OptionalField(recv, field) => {
                let re = self.emit_expr(recv)?;
                let cn = c_name(field);
                Ok(format!("(({re}) != NULL ? ({re})->{cn} : NULL)"))
            }

            // Optional chaining: `recv?.method(args)` → `(recv != NULL ? recv->method(args) : NULL)`
            MonoExprKind::OptionalMethodCall(recv, call_name, args) => {
                let re = self.emit_expr(recv)?;
                let cn = c_name(call_name);
                let arg_strs: CResult<Vec<String>> = args.iter().map(|a| self.emit_expr(a)).collect();
                let arg_strs = arg_strs?;
                let args_joined = arg_strs.join(", ");
                // Method calls go through the regular C function call ABI (receiver as first arg)
                let full_name = c_name(call_name);
                Ok(format!("(({re}) != NULL ? {full_name}({re}{}{args_joined}) : NULL)",
                    if args.is_empty() { "" } else { ", " }))
            }

            MonoExprKind::Call(name, args) => self.emit_call(name, args, &e.ty),

            MonoExprKind::Construct(name, named_args) => {
                // Error construction → haki_error_new
                if name == "Error" {
                    let msg = named_args.iter()
                        .find(|a| a.name == "message")
                        .map(|a| self.emit_expr(&a.value))
                        .transpose()?
                        .unwrap_or_else(|| "\"error\"".into());
                    let cause = named_args.iter().find(|a| a.name == "cause");
                    return Ok(if let Some(c) = cause {
                        let cv = self.emit_expr(&c.value)?;
                        format!("haki_error_new_with_cause({msg}, {cv})")
                    } else {
                        format!("haki_error_new({msg})")
                    });
                }
                let cn = c_name(name);
                let is_class = self.prog.classes.iter().any(|c| c.name == *name);
                let size_expr = format!("sizeof({cn})");
                let alloc = if is_class {
                    format!("({cn}*)haki_alloc({size_expr})")
                } else {
                    format!("({cn}*)malloc({size_expr})")
                };
                // Build as a compound expression using a helper
                // Emit as a statement block via a GNU extension comma expression
                // Better: emit a helper variable in caller scope
                // We use a temp inline struct initializer where possible
                if named_args.is_empty() {
                    return Ok(format!("({alloc})"));
                }
                // Emit as: (({ cn* __t = alloc; __t->f = v; ...; __t; }))
                let mut parts = vec![format!("{cn}* __c_{cn} = {alloc}; ")];
                if is_class {
                    parts.push(format!("__c_{cn}->__arc_count = 1; "));
                }
                // Look up the class/struct field types (including inherited) for type-correct casting
                let class_field_types: std::collections::HashMap<String, SemTy> = {
                    let mut map = std::collections::HashMap::new();
                    if let Some(cls) = self.prog.classes.iter().find(|c| c.name == *name) {
                        // Include parent fields first
                        if let Some(ref sup_name) = cls.superclass {
                            if let Some(sup) = self.prog.classes.iter().find(|c| c.name == *sup_name) {
                                for f in &sup.fields { map.insert(f.name.clone(), f.ty.clone()); }
                            }
                        }
                        for f in &cls.fields { map.insert(f.name.clone(), f.ty.clone()); }
                    } else if let Some(s) = self.prog.structs.iter().find(|s| s.name == *name) {
                        for f in &s.fields { map.insert(f.name.clone(), f.ty.clone()); }
                    }
                    map
                };

                for na in named_args {
                    let ve = self.emit_expr(&na.value)?;
                    // If field expects int64_t but value is a pointer, cast it
                    let field_ty = class_field_types.get(&na.name);
                    let cast_ve = match field_ty {
                        Some(SemTy::Int) if na.value.ty != SemTy::Int
                            && na.value.ty != SemTy::Float
                            && na.value.ty != SemTy::Bool => {
                            format!("(int64_t)(void*)({ve})")
                        }
                        _ => ve,
                    };
                    parts.push(format!("__c_{cn}->{} = {cast_ve}; ", c_name(&na.name)));
                }
                parts.push(format!("__c_{cn};"));
                Ok(format!("({{ {} }})", parts.join("")))
            }

            MonoExprKind::Index(arr, idx) => {
                let ae = self.emit_expr(arr)?;
                let ie = self.emit_expr(idx)?;
                let elem_ty = self.c_ty(&e.ty);
                Ok(format!("(*({elem_ty}*)haki_array_get({ae}, {ie}))"))
            }

            MonoExprKind::Array(elems) => {
                let elem_ty = match &e.ty {
                    SemTy::Generic(_, args) if !args.is_empty() => self.c_ty(&args[0]),
                    _ => "void*".into(),
                };
                if elems.is_empty() {
                    return Ok(format!("haki_array_new({})", array_elem_size(&elem_ty)));
                }
                // Use span-unique names to avoid collisions when arrays are nested
                let uid = e.span.lo;
                let al = format!("__al_{uid}");
                let mut parts = vec![
                    format!("void* {al} = haki_array_new({});", array_elem_size(&elem_ty)),
                ];
                for (i, el) in elems.iter().enumerate() {
                    let ev = self.emit_expr(el)?;
                    let el_name = format!("__el_{uid}_{i}");
                    parts.push(format!(
                        "{{ {elem_ty} {el_name} = ({ev}); haki_array_append({al}, &{el_name}); }}"
                    ));
                }
                // GNU statement expressions require the last expression to end with ;
                parts.push(format!("{al};"));
                Ok(format!("({{ {} }})", parts.join(" ")))
            }

            MonoExprKind::If(i) => {
                // If-as-expression: use ternary for simple cases, GNU statement-expr otherwise
                let cond = self.emit_expr(&i.cond)?;
                // Find yield in then and else blocks
                let then_val = self.yield_val(&i.then_block);
                let else_val = i.else_branch.as_ref()
                    .and_then(|eb| if let MonoElse::Block(b) = eb { self.yield_val(b) } else { None });
                match (then_val, else_val) {
                    (Some(tv), Some(ev)) => {
                        Ok(format!("(({cond}) ? ({tv}) : ({ev}))"))
                    }
                    _ => {
                        // Emit as GNU statement expression
                        Ok(format!("/* if-expr: use statement form */({cond} ? 0 : 0)"))
                    }
                }
            }

            MonoExprKind::Match(m) => {
                // Match-as-expression: emit with GNU statement expr
                let scrutinee = self.emit_expr(&m.scrutinee)?;
                let result_ty = self.c_ty(&e.ty);
                let mut parts = vec![format!("{result_ty} __match_result; ")];

                // Preamble depends on match kind
                match m.kind {
                    MonoMatchKind::Int => {
                        parts.push(format!("int64_t __msc_int = (int64_t)({scrutinee}); "));
                    }
                    MonoMatchKind::String => {
                        parts.push(format!("const char* __msc_str = (const char*)({scrutinee}); "));
                    }
                    MonoMatchKind::Enum | MonoMatchKind::Class => {
                        parts.push(format!("void* __msc = (void*){scrutinee}; "));
                        parts.push(format!("int64_t __mtag = ((int64_t*)__msc)[0]; "));
                        parts.push(format!("void* __mpayload = ((void**)__msc)[1]; "));
                    }
                }

                for (ai, arm) in m.arms.iter().enumerate() {
                    let prefix = if ai == 0 { "if".to_string() } else { "} else if".to_string() };
                    // Build pattern condition (guard merged in for String/Int; separate for Enum)
                    match &arm.pattern {
                        MonoPattern::Named(s) if s == "_" => {
                            if let Some(g) = &arm.guard {
                                let gc = self.emit_expr(g)?;
                                parts.push(format!("}} else if ({gc}) {{"));
                            } else {
                                parts.push("} else {".into());
                            }
                        }
                        MonoPattern::Named(pname) => {
                            let disc = self.variant_discriminant_by_name(pname);
                            parts.push(format!("{prefix} (__mtag == {disc}LL) {{"));
                        }
                        MonoPattern::Int(n) => {
                            if let Some(g) = &arm.guard {
                                let gc = self.emit_expr(g)?;
                                parts.push(format!("{prefix} (__msc_int == {n}LL && ({gc})) {{"));
                            } else {
                                parts.push(format!("{prefix} (__msc_int == {n}LL) {{"));
                            }
                        }
                        MonoPattern::String(s) => {
                            if let Some(g) = &arm.guard {
                                let gc = self.emit_expr(g)?;
                                parts.push(format!("{prefix} (strcmp(__msc_str, \"{s}\") == 0 && ({gc})) {{"));
                            } else {
                                parts.push(format!("{prefix} (strcmp(__msc_str, \"{s}\") == 0) {{"));
                            }
                        }
                    }
                    // Unpack payload (enum only)
                    if arm.bindings.len() == 1 && matches!(m.kind, MonoMatchKind::Enum) {
                        let bt = self.c_ty(&arm.binding_tys[0]);
                        parts.push(format!(
                            "{bt} {} = *({bt}*)__mpayload;",
                            c_name(&arm.bindings[0].name)
                        ));
                    } else if arm.bindings.len() > 1 && matches!(m.kind, MonoMatchKind::Enum) {
                        for (bi, (binding, bty)) in arm.bindings.iter().zip(arm.binding_tys.iter()).enumerate() {
                            let bt = self.c_ty(bty);
                            // __mpayload is void** — each slot __pl[bi] is a void* pointing
                            // to the actual field value. Extract via pointer-to-pointer:
                            //   ((void**)__mpayload)[bi]  gives void* (pointer to field)
                            //   *({bt}*)...               dereferences to get the field value
                            parts.push(format!(
                                "{bt} {} = *({bt}*)((void**)__mpayload)[{bi}];",
                                c_name(&binding.name)
                            ));
                        }
                    }
                    // Emit arm body; find yield (wrapped in guard for Enum pattern)
                    let has_enum_guard = matches!(&arm.pattern, MonoPattern::Named(s) if s != "_")
                        && arm.guard.is_some()
                        && matches!(m.kind, MonoMatchKind::Enum);
                    if has_enum_guard {
                        let gc = self.emit_expr(arm.guard.as_ref().unwrap())?;
                        if let Some(yv) = self.yield_val(&arm.body) {
                            parts.push(format!("if ({gc}) {{ __match_result = {yv}; }}"));
                        }
                    } else if let Some(yv) = self.yield_val(&arm.body) {
                        parts.push(format!("__match_result = {yv};"));
                    }
                }
                parts.push("}".into());
                parts.push("__match_result;".into());
                Ok(format!("({{ {} }})", parts.join(" ")))
            }

            MonoExprKind::Block(b) => {
                // Emit try-desugared blocks as GNU C statement expressions.
                // Declarations and statements are separated to satisfy C99 ordering.
                // We emit: ({ decls...; stmts...; yield_val; })
                let mut deferred: Vec<String> = vec![];
                let mut decls = String::new();
                let mut stmts = String::new();
                for stmt in &b.stmts {
                    if matches!(&stmt.kind, MonoStmtKind::Yield(_)) { break; }
                    let is_decl = matches!(&stmt.kind, MonoStmtKind::Let(_));
                    if is_decl {
                        self.emit_stmt(&mut decls, stmt, 1, &mut deferred, "").ok();
                    } else {
                        self.emit_stmt(&mut stmts, stmt, 1, &mut deferred, "").ok();
                    }
                }
                let decls_flat = decls.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
                let stmts_flat = stmts.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
                if let Some(yv) = self.yield_val(b) {
                    let mut body = String::new();
                    if !decls_flat.is_empty() { body.push_str(&decls_flat); body.push(' '); }
                    if !stmts_flat.is_empty() { body.push_str(&stmts_flat); body.push(' '); }
                    Ok(format!("({{ {body}{yv}; }})"))
                } else {
                    Ok("0".into())
                }
            }

            MonoExprKind::Assign(target, val) => {
                let te = self.emit_expr(target)?;
                let ve = self.emit_expr(val)?;
                Ok(format!("({te} = {ve})"))
            }

            MonoExprKind::Async(inner) => {
                // Async: generate a thunk + args struct, spawn on the thread pool.
                let span_id = inner.span.lo;
                let thunk_id  = format!("__haki_thunk_{span_id}");
                let args_id   = format!("__haki_args_{span_id}");
                let struct_id = format!("__HakiThunkArgs_{span_id}");

                if let MonoExprKind::Call(fn_name, fn_args) = &inner.kind {
                    let arg_fields: Vec<String> = fn_args.iter().enumerate()
                        .map(|(i, a)| format!("{} arg{i};", self.c_ty(&a.ty)))
                        .collect();
                    let arg_unpacks: Vec<String> = fn_args.iter().enumerate()
                        .map(|(i, a)| {
                            let ct = self.c_ty(&a.ty);
                            format!("{ct} __a{i} = __sa->arg{i};")
                        })
                        .collect();
                    let call_args_str: Vec<String> = (0..fn_args.len())
                        .map(|i| format!("__a{i}"))
                        .collect();

                    let ret_c_ty = self.c_ty(&inner.ty);
                    let cast_str = if ret_c_ty == "void" || ret_c_ty.is_empty() {
                        String::new()
                    } else {
                        format!("({ret_c_ty})")
                    };

                    // Thunk definition emitted before the calling function:
                    let fields_str = if arg_fields.is_empty() {
                        "int _dummy;".to_string()
                    } else {
                        arg_fields.join(" ")
                    };
                    let unpacks_str = arg_unpacks.join(" ");
                    let call_str = call_args_str.join(", ");
                    let fn_c = c_name(fn_name);

                    let thunk_def = format!(
                        "typedef struct {{ {fields_str} }} {struct_id};\n\
                         static void* {thunk_id}(void* __vargs) {{\n\
                             {struct_id}* __sa = ({struct_id}*)__vargs;\n\
                             {unpacks_str}\n\
                             void* __r = (void*)(intptr_t){cast_str}{fn_c}({call_str});\n\
                             free(__vargs); return __r;\n\
                         }}\n"
                    );
                    self.async_thunks.borrow_mut().push(thunk_def);

                    // Inline spawn: malloc args struct, fill fields, spawn task
                    let mut parts = vec![];
                    parts.push(format!(
                        "{struct_id}* {args_id} = ({struct_id}*)malloc(sizeof({struct_id}));"
                    ));
                    for (i, a) in fn_args.iter().enumerate() {
                        let av = self.emit_expr(a)?;
                        parts.push(format!("{args_id}->arg{i} = {av};"));
                    }
                    parts.push(format!(
                        "HakiTask* __task_{span_id} = haki_task_spawn({thunk_id}, {args_id}); __task_{span_id};"
                    ));
                    Ok(format!("({{ {} }})", parts.join(" ")))
                } else {
                    // Non-call async (edge case)
                    let ie = self.emit_expr(inner)?;
                    Ok(format!("({{ (void)({ie}); haki_task_spawn(NULL, NULL); }})"))
                }
            }
        }
    }

    fn emit_var(&self, name: &str, ty: &SemTy) -> String {
        // Check if this is an enum variant (unit variant used as value)
        if let Some((_, disc, v)) = self.find_variant(name) {
            if v.fields.is_empty() {
                // Unit variant — emit as a {tag, NULL} struct on the heap
                let enum_ty = self.c_ty(ty);
                return format!(
                    "({{ {}* __ev = ({}*)malloc(sizeof(int64_t)*2); \
                     ((int64_t*)__ev)[0] = {disc}LL; ((void**)__ev)[1] = NULL; __ev; }})",
                    if enum_ty.ends_with('*') { &enum_ty[..enum_ty.len()-1] } else { "void" },
                    if enum_ty.ends_with('*') { &enum_ty[..enum_ty.len()-1] } else { "void" }
                );
            }
        }
        // In a class method, bare field names are accessed via self->
        if self.self_fields.borrow().contains(name) {
            return format!("self->{}", c_name(name));
        }
        // Mutable capture: declared as T* varname = (T*)__env — dereference on read
        if self.mutable_captures.borrow().contains(name) {
            return format!("(*{})", c_name(name));
        }
        c_name(name)
    }

    fn emit_call(&self, name: &str, args: &[MonoExpr], ret_ty: &SemTy) -> CResult<String> {
        // ── HTTP struct constructors → C compound literals ──────────────────────
        // HttpResponse(status, body, contentType) → (HttpResponse){status, body, contentType}
        if name == "HttpResponse" && (args.len() == 2 || args.len() == 3) {
            let s = self.emit_expr(&args[0])?;
            let b = self.emit_expr(&args[1])?;
            let ct = if args.len() == 3 {
                self.emit_expr(&args[2])?
            } else {
                "\"\"".to_string()
            };
            return Ok(format!("haki_make_http_response((int){s}, {b}, {ct})"));
        }

        // ── Closure / fn-param invocation ────────────────────────────────────
        // __haki_invoke_closure_TYPE(closure, args...) → fat pointer call
        if name.starts_with("__haki_invoke_closure_") {
            if args.is_empty() { return Ok("NULL".into()); }
            let closure = self.emit_expr(&args[0])?;
            let call_args = args[1..].iter()
                .map(|a| self.emit_expr(a))
                .collect::<CResult<Vec<_>>>()?;
            let args_str = call_args.join(", ");
            // Fat pointer: void*[2] = { fn_ptr, env_ptr }
            // Call: ((RetType(*)(void*, ArgTypes))((void**)(closure))[0])(((void**)(closure))[1], args)
            let (ret_cast, ret_c_ty) = match name {
                n if n.ends_with("_int")   => ("(int64_t)", "int64_t"),
                n if n.ends_with("_float") => ("(double)",  "double"),
                n if n.ends_with("_bool")  => ("(int8_t)",  "int8_t"),
                n if n.ends_with("_str")   => ("(const char*)", "const char*"),
                _                          => ("", "void*"),
            };
            let sep = if call_args.is_empty() { "" } else { ", " };
            return Ok(format!(
                "({ret_cast}(({ret_c_ty}(*)(void*{sep2}{arg_types}))                 ((void**)({closure}))[0])(((void**)({closure}))[1]{sep}{args_str}))",
                ret_cast = ret_cast,
                ret_c_ty = ret_c_ty,
                sep2 = if call_args.is_empty() { "" } else { ", " },
                arg_types = args[1..].iter().map(|a| self.c_ty(&a.ty)).collect::<Vec<_>>().join(", "),
                closure = closure,
                sep = sep,
                args_str = args_str,
            ));
        }

        // ── Chan construction ─────────────────────────────────────────────────
        // Chan<T>(capacity: N) → haki_chan_new(N)
        // Chan<T>()            → haki_chan_new(0)  (unbounded)
        // Also intercept "Chan" direct calls (from generic constructor path)
        if name == "Chan" || name.starts_with("Chan__") || name == "sync__Chan" || name.starts_with("sync__Chan__") {
            let capacity = if args.is_empty() { "0".to_string() }
            else { self.emit_expr(&args[0])? };
            return Ok(format!("haki_chan_new({capacity})"));
        }

        // haki_chan_send(ch, val) — encode value as void*.
        // Primitives (int/float/bool): pack via intptr_t.
        // Pointer types (string, structs): cast directly.
        if name == "haki_chan_send" && args.len() == 2 {
            let ch  = self.emit_expr(&args[0])?;
            let val = self.emit_expr(&args[1])?;
            let val_ty = &args[1].ty;
            let encoded = match val_ty {
                SemTy::String
                | SemTy::Optional(_)
                | SemTy::Named(_)
                | SemTy::Generic(_, _) => format!("(void*)({val})"),
                _ => format!("(void*)(intptr_t)({val})"),
            };
            return Ok(format!("haki_chan_send({ch}, {encoded})"));
        }
        // haki_chan_receive(ch) — returns void* but must be cast to the element type.
        // Use intptr_t as intermediate for primitives (int/bool/float) to avoid
        // strict-aliasing UB; for pointer types (string, structs) cast directly.
        if name == "haki_chan_receive" && args.len() == 1 {
            let ch = self.emit_expr(&args[0])?;
            let cast = match ret_ty {
                SemTy::String
                | SemTy::Optional(_)
                | SemTy::Named(_)
                | SemTy::Generic(_, _) => {
                    let c = self.c_ty(ret_ty);
                    format!("(({c})haki_chan_receive({ch}))")
                }
                _ => {
                    // Int, Bool, Float — pack as intptr_t
                    let c = self.c_ty(ret_ty);
                    format!("(({c})(intptr_t)haki_chan_receive({ch}))")
                }
            };
            return Ok(cast);
        }
        if name == "haki_chan_new" {
            let capacity = if args.is_empty() {
                "0".to_string()
            } else {
                self.emit_expr(&args[0])?
            };
            return Ok(format!("haki_chan_new({capacity})"));
        }

        // ── Builtin intercepts ────────────────────────────────────────────────

        // print(any)
        // string_to_int / string_to_float -> emit as haki_string_to_int(...)
        if (name == "string_to_int" || name == "string_to_float") && args.len() == 1 {
            let c_fn = if name == "string_to_int" { "haki_string_to_int" } else { "haki_string_to_float" };
            return Ok(format!("{}({})", c_fn, self.emit_expr(&args[0])?));
        }
        if name == "print" && args.len() == 1 {
            let a = self.emit_expr(&args[0])?;
            return Ok(match &args[0].ty {
                SemTy::Int   => format!("haki_print_int({a})"),
                SemTy::Float => format!("haki_print_float({a})"),
                SemTy::Bool  => format!("haki_print_bool({a})"),
                _            => format!("haki_print({a})"),
            });
        }
        if name == "print_int"   { return Ok(format!("haki_print_int({})", self.emit_expr(&args[0])?)); }
        if name == "print_float" { return Ok(format!("haki_print_float({})", self.emit_expr(&args[0])?)); }
        if name == "print_bool"  { return Ok(format!("haki_print_bool({})", self.emit_expr(&args[0])?)); }
        if name == "panic"       { return Ok(format!("haki_panic({})", self.emit_expr(&args[0])?)); }

        if name == "int_to_string"   { return Ok(format!("haki_int_to_string({})", self.emit_expr(&args[0])?)); }
        if name == "float_to_string" { return Ok(format!("haki_float_to_string({})", self.emit_expr(&args[0])?)); }
        if name == "bool_to_string"  { return Ok(format!("haki_bool_to_string({})",  self.emit_expr(&args[0])?)); }
        if name == "string_length"   { return Ok(format!("haki_string_length({})", self.emit_expr(&args[0])?)); }
        if name == "argv"            { return Ok("haki_argv()".into()); }

        // readFile / writeFile / fileExists
        if name == "readFile"   {
            let p = self.emit_expr(&args[0])?;
            return Ok(format!("haki_read_file({p})"));
        }

        // Map() zero-arg constructor — used by bootstrap compiler
        // Map<K,V> is always string-keyed; val_size = sizeof(void*)
        if name == "Map" && args.is_empty() {
            return Ok("haki_map_new(sizeof(void*))".into());
        }

        // Map getOrDefault — handles monomorphized names like
        // Map__string__T__getOrDefault that the bootstrap generates
        if name.contains("__getOrDefault") && args.len() == 3 {
            let map = self.emit_expr(&args[0])?;
            let key = self.emit_expr(&args[1])?;
            let def = self.emit_expr(&args[2])?;
            // Determine return type from map's value type parameter
            let val_cast = if let SemTy::Generic(_, targs) = &args[0].ty {
                match targs.get(1) {
                    Some(SemTy::Int)   => "(int64_t)(intptr_t)",
                    Some(SemTy::Bool)  => "(int8_t)(intptr_t)",
                    Some(SemTy::Float) => "(double)(intptr_t)",
                    _ => "(const char*)",
                }
            } else { "(const char*)" };
            let def_cast = match &args[2].ty {
                SemTy::Int | SemTy::Bool => format!("(void*)(intptr_t)({def})"),
                _ => def,
            };
            return Ok(format!("({val_cast}haki_map_get_or_default({map}, {key}, {def_cast}))"));
        }
        if name == "writeFile"  {
            let p = self.emit_expr(&args[0])?;
            let c = self.emit_expr(&args[1])?;
            return Ok(format!("haki_write_file({p}, {c})"));
        }
        if name == "fileExists" {
            return Ok(format!("haki_file_exists({})", self.emit_expr(&args[0])?));
        }

        // std/fs path checks — return bool (int8_t in C)
        if name == "fs__exists" || name == "haki_fs_path_exists" {
            return Ok(format!("((int8_t)(haki_fs_exists({}) != 0))", self.emit_expr(&args[0])?));
        }
        if name == "fs__isDir" || name == "haki_fs_path_is_dir" {
            return Ok(format!("((int8_t)(haki_fs_is_dir({}) != 0))", self.emit_expr(&args[0])?));
        }
        if name == "fs__isFile" || name == "haki_fs_path_is_file" {
            return Ok(format!("((int8_t)(haki_fs_is_file({}) != 0))", self.emit_expr(&args[0])?));
        }
        if name == "fs__size" {
            return Ok(format!("haki_fs_size({})", self.emit_expr(&args[0])?));
        }

        // thread_spawn
        if name == "thread_spawn" {
            let f = self.emit_expr(&args[0])?;
            return Ok(format!("haki_thread_spawn({f})"));
        }

        // std/env, std/time, std/process, std/regex — intercept before
        // the generic __get/__set/__append pattern matchers below, which
        // would otherwise match module-prefixed names like env__get.
        // ── HttpServer method intercepts ─────────────────────────────────────
        // ── Router intercepts ────────────────────────────────────────────────
        if name == "Router" && args.is_empty() {
            return Ok("haki_router_new()".into());
        }
        if name.contains("Router__get") && args.len() == 3 {
            let router = self.emit_expr(&args[0])?;
            let pat    = self.emit_expr(&args[1])?;
            let handler= self.emit_expr(&args[2])?;
            return Ok(format!("haki_router_add({router}, \"GET\", {pat}, (HakiRouteHandler)({handler}))"));
        }
        if name.contains("Router__post") && args.len() == 3 {
            let router = self.emit_expr(&args[0])?;
            let pat    = self.emit_expr(&args[1])?;
            let handler= self.emit_expr(&args[2])?;
            return Ok(format!("haki_router_add({router}, \"POST\", {pat}, (HakiRouteHandler)({handler}))"));
        }
        if name.contains("Router__put") && args.len() == 3 {
            let router = self.emit_expr(&args[0])?;
            let pat    = self.emit_expr(&args[1])?;
            let handler= self.emit_expr(&args[2])?;
            return Ok(format!("haki_router_add({router}, \"PUT\", {pat}, (HakiRouteHandler)({handler}))"));
        }
        if name.contains("Router__delete") && args.len() == 3 {
            let router = self.emit_expr(&args[0])?;
            let pat    = self.emit_expr(&args[1])?;
            let handler= self.emit_expr(&args[2])?;
            return Ok(format!("haki_router_add({router}, \"DELETE\", {pat}, (HakiRouteHandler)({handler}))"));
        }
        if name.contains("Router__handle") && args.len() == 2 {
            let router = self.emit_expr(&args[0])?;
            let req    = self.emit_expr(&args[1])?;
            return Ok(format!("haki_router_dispatch({router}, {req})"));
        }
        if name == "routerDispatch" && args.len() == 2 {
            let router = self.emit_expr(&args[0])?;
            let req    = self.emit_expr(&args[1])?;
            return Ok(format!("haki_router_dispatch({router}, {req})"));
        }

        // ── HTTP helper intercepts ───────────────────────────────────────────
        if name == "requestParam" && args.len() == 2 {
            let req = self.emit_expr(&args[0])?;
            let key = self.emit_expr(&args[1])?;
            return Ok(format!("haki_request_param({req}, {key})"));
        }
        if name == "jsonDecodeGet" && args.len() == 2 {
            let json = self.emit_expr(&args[0])?;
            let key  = self.emit_expr(&args[1])?;
            return Ok(format!("haki_json_decode_get({json}, {key})"));
        }

        // ── HttpServer intercepts ─────────────────────────────────────────────
        if name.contains("HttpServer__listen") || (name.ends_with("__listen") && args.len() == 1) {
            let server = self.emit_expr(&args[0])?;
            return Ok(format!("haki_http_server_listen({server})"));
        }
        if name == "HttpServer" && args.len() == 2 {
            let port = self.emit_expr(&args[0])?;
            let handler = self.emit_expr(&args[1])?;
            return Ok(format!("haki_http_server_new({port}, (HttpHandler)({handler}))"));
        }

        // Task<T>.await() → haki_task_await(task) returning void*
        // Cast result based on return type: int → (int64_t)(intptr_t), else direct
        if name.contains("__await") && args.len() == 1 {
            let task = self.emit_expr(&args[0])?;
            let cast = match ret_ty {
                SemTy::Int   => "(int64_t)(intptr_t)",
                SemTy::Float => "(double)(intptr_t)",
                SemTy::Bool  => "(int8_t)(intptr_t)",
                _            => "",
            };
            return Ok(format!("({cast}haki_task_await({task}))"));
        }
        // Mutex<T>.lock() → haki_mutex_lock
        if name.contains("Mutex__") && name.contains("__lock") {
            let mutex = self.emit_expr(&args[0])?;
            return Ok(format!("haki_mutex_lock({mutex})"));
        }
        // TaskGroup<T>.add(task) / .spawn(task) → haki_taskgroup_add(group, task)
        // The task is a HakiTask* from async fn(...); taskgroup_add just registers it.
        if (name.contains("haki_taskgroup_spawn") || name.contains("__spawn") || name.contains("__add"))
            && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "TaskGroup")
        {
            let group = self.emit_expr(&args[0])?;
            let task  = self.emit_expr(&args[1])?;
            return Ok(format!("haki_taskgroup_add({group}, {task})"));
        }

        if name == "haki_env_get"   { return Ok(format!("haki_env_get({})",   self.emit_expr(&args[0])?)); }
        if name == "haki_env_set"   { return Ok(format!("haki_env_set({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name == "haki_env_unset" { return Ok(format!("haki_env_unset({})", self.emit_expr(&args[0])?)); }
        if name == "haki_env_cwd"   { return Ok(format!("haki_env_cwd()")); }
        if name == "haki_env_chdir" { return Ok(format!("haki_env_chdir({})", self.emit_expr(&args[0])?)); }
        if name == "haki_time_now_ms"   { return Ok(format!("haki_time_now_ms()")); }
        if name == "haki_time_sleep_ms" { return Ok(format!("haki_time_sleep_ms({})", self.emit_expr(&args[0])?)); }
        if name == "haki_time_format"   { return Ok(format!("haki_time_format({})", self.emit_expr(&args[0])?)); }
        if name == "haki_process_run"   { return Ok(format!("haki_process_run({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name == "haki_process_exec"  { return Ok(format!("haki_process_exec({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name == "haki_process_shell" { return Ok(format!("haki_process_shell({})", self.emit_expr(&args[0])?)); }
        if name == "haki_process_exit"  { return Ok(format!("haki_process_exit({})", self.emit_expr(&args[0])?)); }
        if name == "haki_regex_matches"     { return Ok(format!("haki_regex_matches({}, {})",     self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name == "haki_regex_find"        { return Ok(format!("haki_regex_find({}, {})",        self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name == "haki_regex_replace_all" { return Ok(format!("haki_regex_replace_all({}, {}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?, self.emit_expr(&args[2])?)); }
        if name == "haki_regex_split"       { return Ok(format!("haki_regex_split({}, {})",       self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }

        // Array methods: Array__T__method (must NOT match fs__appendFile)
        if name.contains("__append") && !name.contains("appendFile") && !name.contains("AppendFile") {
            let arr = self.emit_expr(&args[0])?;
            let val = self.emit_expr(&args[1])?;
            // GNU statement expressions, function calls, and arithmetic expressions are
            // not lvalues — taking &(expr) is illegal when expr isn't addressable.
            // Use a temp variable whenever the value is not a simple identifier or cast.
            // Simple identifiers: no spaces, no parens, no operators.
            let is_simple_ident = val.chars().all(|c| c.is_alphanumeric() || c == '_');
            let needs_temp = !is_simple_ident;
            if needs_temp {
                let elem_ty = self.c_ty(&args[1].ty);
                return Ok(format!(
                    "{{ {} __append_tmp = ({}); haki_array_append_val({}, &__append_tmp); }}",
                    elem_ty, val, arr
                ));
            }
            return Ok(format!("haki_array_append_val({}, &({}))", arr, val));
        }
        if name.contains("__length") && !matches!(args[0].ty, SemTy::String) { return Ok(format!("haki_array_length({})", self.emit_expr(&args[0])?)); }

        // Array typed method intercepts
        if name.contains("__removeLast") && args.len() == 1 {
            let arr = self.emit_expr(&args[0])?;
            // haki_array_remove_last returns void* pointing INTO the array's inline storage
            // The array stores values by memcpy (elem_size bytes), so dereference the pointer
            // Determine element type — may be SemTy::Int or SemTy::Named("int")
            let elem_ty = match &args[0].ty {
                SemTy::Generic(_, type_args) if !type_args.is_empty() => Some(&type_args[0]),
                _ => None,
            };
            let cast = match elem_ty {
                Some(SemTy::Int) => "*(int64_t*)",
                Some(SemTy::Float) => "*(double*)",
                Some(SemTy::Bool) => "*(int8_t*)",
                Some(SemTy::Named(n)) if n == "int" => "*(int64_t*)",
                Some(SemTy::Named(n)) if n == "float" => "*(double*)",
                Some(SemTy::Named(n)) if n == "bool" => "*(int8_t*)",
                _ => "*(const char**)",
            };
            return Ok(format!("({cast}haki_array_remove_last({arr}))"));
        }
        if name.contains("__removeAt") && args.len() == 2 {
            let arr = self.emit_expr(&args[0])?;
            let idx = self.emit_expr(&args[1])?;
            return Ok(format!("haki_array_remove_at({arr}, {idx})"));
        }
        if name.contains("__indexOf") && args.len() == 2 && !matches!(args[0].ty, SemTy::String) {
            let arr = self.emit_expr(&args[0])?;
            let val = self.emit_expr(&args[1])?;
            let fn_name = match args[0].ty {
                SemTy::Generic(_, ref targs) if targs.first() == Some(&SemTy::String) =>
                    "haki_array_index_of_str",
                _ => "haki_array_index_of_int",
            };
            return Ok(format!("{fn_name}({arr}, {val})"));
        }
        if name.contains("__contains") && args.len() == 2 && !matches!(args[0].ty, SemTy::String) {
            let arr = self.emit_expr(&args[0])?;
            let val = self.emit_expr(&args[1])?;
            let fn_name = match args[0].ty {
                SemTy::Generic(_, ref targs) if targs.first() == Some(&SemTy::String) =>
                    "haki_array_contains_str",
                _ => "haki_array_contains_int",
            };
            return Ok(format!("{fn_name}({arr}, {val})"));
        }
        if name.contains("__first") && args.len() == 1 {
            let arr = self.emit_expr(&args[0])?;
            return Ok(format!("haki_array_first({arr})"));
        }
        if name.contains("__last") && args.len() == 1 {
            let arr = self.emit_expr(&args[0])?;
            return Ok(format!("haki_array_last({arr})"));
        }
        // Array __get: requires exactly 2 args and receiver is array type
        if name.contains("__get") && args.len() == 2 && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Array") {
            return Ok(format!("haki_array_get({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }

        // Map methods — check receiver is Map type to avoid matching module functions
        if name.contains("__set") && args.len() == 3
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            let map = self.emit_expr(&args[0])?;
            let key = self.emit_expr(&args[1])?;
            let val = self.emit_expr(&args[2])?;
            // Map stores values as void* — cast int/bool to void* via intptr_t
            let val_cast = match &args[2].ty {
                SemTy::Int | SemTy::Bool => format!("(void*)(intptr_t)({val})"),
                SemTy::Float => format!("(void*)(intptr_t)(*(int64_t*)&({val}))"),
                _ => val,
            };
            return Ok(format!("haki_map_set({map}, {key}, {val_cast})"));
        }
        if name.contains("__has") && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            return Ok(format!("haki_map_has({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }
        if name.contains("__delete") && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            return Ok(format!("haki_map_delete({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }
        if name.contains("__get") && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            let map = self.emit_expr(&args[0])?;
            let key = self.emit_expr(&args[1])?;
            // Cast void* result to the Map's value type so clang doesn't warn
            let val_c_ty = if let SemTy::Generic(_, ref vargs) = args[0].ty {
                if vargs.len() >= 2 { self.c_ty(&vargs[1]) } else { "void*".to_string() }
            } else { "void*".to_string() };
            if val_c_ty == "void*" {
                return Ok(format!("haki_map_get({map}, {key})"));
            } else {
                return Ok(format!("(({val_c_ty})(intptr_t)haki_map_get({map}, {key}))"));
            }
        }

        // String methods (Type__method mangled)
        if name == "string__substring" || name.ends_with("__substring") {
            return Ok(format!("haki_string_substring({}, {}, {})",
                self.emit_expr(&args[0])?, self.emit_expr(&args[1])?, self.emit_expr(&args[2])?));
        }
        if name.ends_with("__length") && args.len() == 1 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_length({})", self.emit_expr(&args[0])?));
        }
        // __contains: only intercept for string receiver (Array has its own runtime fn)
        if name.ends_with("__contains") && args.len() >= 2 {
            if matches!(args[0].ty, SemTy::String) {
                return Ok(format!("haki_string_contains({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
            }
            // Array contains — fall through to Array__T__contains runtime fn
        }
        if name.ends_with("__split")      { return Ok(format!("haki_string_split({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__trim")       { return Ok(format!("haki_string_trim({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__trimStart")  { return Ok(format!("haki_string_trim_start({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__trimEnd")    { return Ok(format!("haki_string_trim_end({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__toUpper")    { return Ok(format!("haki_string_to_upper({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__toLower")    { return Ok(format!("haki_string_to_lower({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__indexOf") && args.len() >= 2 {
            if matches!(args[0].ty, SemTy::String) {
                return Ok(format!("haki_string_index_of({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
            }
        }
        if name.ends_with("__replace")    { return Ok(format!("haki_string_replace({}, {}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?, self.emit_expr(&args[2])?)); }
        if name.ends_with("__startsWith") { return Ok(format!("haki_string_starts_with({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__endsWith")   { return Ok(format!("haki_string_ends_with({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__join") && args.len() == 2 { return Ok(format!("haki_array_join({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__length") && args.len() == 1 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_length({})", self.emit_expr(&args[0])?));
        }
        if name.ends_with("__isEmpty") && args.len() == 1 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_is_empty({})", self.emit_expr(&args[0])?));
        }
        if name.ends_with("__charAt") && args.len() == 2 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_char_at({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }
        if name.ends_with("__charCodeAt") && args.len() == 2 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_char_code_at({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }

        // haki_make_closure: env_ptr must be void* — cast int/bool captures
        if name == "haki_make_closure" && args.len() == 2 {
            let fn_ptr = self.emit_expr(&args[0])?;
            let env_raw = self.emit_expr(&args[1])?;
            // Mutable capture: env_raw is "__addr_varname" — emit &varname (address-of)
            let env_cast = if let Some(var_name) = env_raw.strip_prefix("__addr_") {
                format!("(void*)(&{var_name})")
            } else {
                // Cast any primitive to void* via intptr_t
                let needs_cast = matches!(args[1].ty,
                    SemTy::Int | SemTy::Bool | SemTy::Named(_)
                ) && !matches!(args[1].ty, SemTy::String);
                if needs_cast {
                    format!("(void*)(intptr_t)({env_raw})")
                } else if matches!(args[1].ty, SemTy::Float) {
                    format!("(void*)(intptr_t)(*(int64_t*)&({env_raw}))")
                } else {
                    env_raw
                }
            };
            return Ok(format!("haki_make_closure((void*){fn_ptr}, {env_cast})"));
        }

        // Enum variant construction: VariantName(payload)
        if let Some((_, disc, variant)) = self.find_variant(name) {
            return self.emit_variant_construct(name, disc, &variant.fields, args);
        }

        // Error construction
        if name == "Error" {
            if args.len() == 1 {
                return Ok(format!("haki_error_new({})", self.emit_expr(&args[0])?));
            }
        }

        // ARC retain/release
        if name == "__haki_retain"  { return Ok(format!("haki_retain({})", self.emit_expr(&args[0])?)); }
        if name == "__haki_release" { return Ok(format!("haki_release({})", self.emit_expr(&args[0])?)); }

        // General function call
        let arg_strs: Vec<String> = args.iter()
            .map(|a| self.emit_expr(a))
            .collect::<CResult<_>>()?;

        // Check if this looks like a local fn-type variable (closure param).
        // Heuristic: name is a simple identifier that doesn't match any C runtime
        // pattern and appears to be a closure (no __ prefix, short name).
        // The actual check: if the name is not a mangled function (no __ except leading)
        // and the return type suggests it came from a fn-type local, emit fat-ptr call.
        // We detect this by checking if the FIRST argument's type matches the closure var.
        // Simpler: check if name is in our self.fn_names set (built from emitted functions).
        // For now use the known-globals approach: if not in runtime globals list, treat as closure.
        // Only emit fat-pointer call for local fn-type variables (closure params).
        // These are identified by: short lowercase names with no underscores,
        // that are NOT known top-level function names in the program.
        // Top-level user functions are registered in self.fn_names.
        let is_fn_local_var = !name.starts_with("haki_")
            && !name.starts_with("__")
            && !name.contains("__")
            && self.fn_locals.borrow().contains(name);

        if is_fn_local_var {
            // Emit as fat pointer call: ((RetType(*)(void*, ArgTypes))((void**)(name))[0])(((void**)(name))[1], args)
            let ret_c_ty = self.c_ty(ret_ty);
            let ret_c_ty = if ret_c_ty.is_empty() || ret_c_ty == "void" { "void*".to_string() } else { ret_c_ty };
            let sep = if arg_strs.is_empty() { "" } else { ", " };
            let arg_types: Vec<String> = args.iter().map(|a| self.c_ty(&a.ty)).collect();
            return Ok(format!(
                "(({ret_c_ty}(*)( void*{sep2}{arg_types}))((void**)({name}))[0])(((void**)({name}))[1]{sep}{arg_strs})",
                ret_c_ty = ret_c_ty,
                sep2 = if arg_strs.is_empty() { "" } else { ", " },
                arg_types = arg_types.join(", "),
                name = c_name(name),
                sep = sep,
                arg_strs = arg_strs.join(", "),
            ));
        }

        Ok(format!("{}({})", c_name(name), arg_strs.join(", ")))
    }

    fn emit_variant_construct(
        &self,
        _name: &str,
        disc: usize,
        fields: &[haki_ast::Ty],
        args: &[MonoExpr],
    ) -> CResult<String> {
        if fields.is_empty() {
            // Unit variant
            return Ok(format!(
                "({{ void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); \
                 ((int64_t*)__ev)[0] = {disc}LL; ((void**)__ev)[1] = NULL; __ev; }})"
            ));
        }
        if args.len() == 1 {
            let av = self.emit_expr(&args[0])?;
            let ty = self.c_ty(&args[0].ty);
            return Ok(format!(
                "({{ {ty}* __pl = ({ty}*)malloc(sizeof({ty})); *__pl = {av}; \
                 void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); \
                 ((int64_t*)__ev)[0] = {disc}LL; ((void**)__ev)[1] = __pl; __ev; }})"
            ));
        }
        // Multi-field payload: pack into a flat array
        let mut parts = vec![
            format!("void* __ev = malloc(sizeof(int64_t)+sizeof(void*)); ((int64_t*)__ev)[0] = {disc}LL;"),
            format!("void** __pl = (void**)malloc({} * sizeof(void*));", args.len()),
        ];
        for (i, a) in args.iter().enumerate() {
            let av = self.emit_expr(a)?;
            let ty = self.c_ty(&a.ty);
            parts.push(format!("{{ {ty}* __f = ({ty}*)malloc(sizeof({ty})); *__f = {av}; __pl[{i}] = __f; }}"));
        }
        parts.push("((void**)__ev)[1] = __pl; __ev;".into());
        Ok(format!("({{ {} }})", parts.join(" ")))
    }

    /// Extract the yield expression value from a block.
    /// Used for if-as-expression and match-arm yield values.
    fn yield_val(&self, block: &MonoBlock) -> Option<String> {
        for stmt in &block.stmts {
            if let MonoStmtKind::Yield(e) = &stmt.kind {
                return self.emit_expr(e).ok();
            }
        }
        None
    }

    fn find_variant(&self, name: &str) -> Option<(String, usize, haki_ast::EnumVariant)> {
        for (ename, edef) in &self.prog.enum_defs {
            if let Some((idx, v)) = edef.variants.iter().enumerate().find(|(_, v)| {
                v.name.name == name ||
                v.name.name.split("__").last().map_or(false, |b| b == name)
            }) {
                return Some((ename.clone(), idx, v.clone()));
            }
        }
        None
    }

    fn variant_discriminant(&self, name: &str) -> usize {
        self.find_variant(name).map(|(_, d, _)| d).unwrap_or(0)
    }

    fn variant_discriminant_by_name(&self, name: &str) -> usize {
        self.variant_discriminant(name)
    }

    fn tuple_type_name(&self, vals: &[MonoExpr]) -> String {
        // Generate a name for an anonymous tuple struct
        // We emit these inline as flat arrays cast to the right type
        format!("__Tuple{}", vals.len())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Collect arities of all tuple returns found in a block.
fn collect_tuple_arities_block(block: &MonoBlock, out: &mut std::collections::BTreeSet<usize>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            MonoStmtKind::Return(r) if r.values.len() > 1 => { out.insert(r.values.len()); }
            MonoStmtKind::If(i) => {
                collect_tuple_arities_block(&i.then_block, out);
                if let Some(els) = &i.else_branch {
                    match els {
                        MonoElse::Block(b) => collect_tuple_arities_block(b, out),
                        MonoElse::If(inner) => collect_tuple_arities_block(&inner.then_block, out),
                    }
                }
            }
            MonoStmtKind::While(w) => collect_tuple_arities_block(&w.body, out),
            MonoStmtKind::For(f) => collect_tuple_arities_block(&f.body, out),
            _ => {}
        }
    }
}

/// Map a Haki AST TyKind to a C type string for extern "c" declarations.
/// Return the correct sizeof argument for array element allocation.
///
/// All named Haki types (structs, classes, enums) are represented as C pointers
/// (`T*`). Arrays of these types store pointers, so the element size must be
/// `sizeof(void*)` — NOT `sizeof(void)` or `sizeof(T)`.
///
/// Scalar types (int64_t, double, int8_t) are stored by value and use their
/// own sizeof. String (`const char*`) is a pointer → `sizeof(void*)`.
fn array_elem_size(elem_ty: &str) -> String {
    // If the C type string contains a `*` it's a pointer type — use sizeof(void*)
    // to ensure correct 8-byte slot regardless of the concrete type.
    if elem_ty.contains('*') || elem_ty == "void" {
        "sizeof(void*)".into()
    } else {
        format!("sizeof({elem_ty})")
    }
}

fn ast_ty_to_c(kind: &haki_ast::TyKind) -> String {
    match kind {
        haki_ast::TyKind::Named(id) => match id.name.as_str() {
            "int"    => "int64_t".into(),
            "float" | "f64" => "double".into(),
            "bool"   => "int".into(),
            "string" => "const char*".into(),
            "void"   => "void".into(),
            _        => "void*".into(),   // opaque handle for named types
        },
        haki_ast::TyKind::Optional(_) => "void*".into(),
        haki_ast::TyKind::Generic(_, _) => "void*".into(),
        haki_ast::TyKind::Tuple(_) => "void*".into(),
        haki_ast::TyKind::Fn(_, _) => "void*".into(),
    }
}

/// Convert a Haki identifier to a safe C identifier.
/// Replaces `__` (already mangled), handles keywords.
fn c_name(name: &str) -> String {
    let s = name.replace("::", "__");
    // Avoid C reserved words
    match s.as_str() {
        "int" | "float" | "double" | "char" | "short" | "long" | "void"
        | "return" | "if" | "else" | "while" | "for" | "break" | "continue"
        | "struct" | "typedef" | "static" | "extern" | "register" | "auto"
        | "switch" | "case" | "default" | "do" | "goto" | "sizeof"
        | "volatile" | "const" | "enum" | "union" | "signed" | "unsigned"
        | "restrict" | "inline" | "_Bool" | "_Complex"
        => format!("haki_{s}"),
        _ => s,
    }
}

/// Escape a string for C string literal syntax.
fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '"'  => out.push_str("\\\""),
            '\\'  => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c    => out.push(c),
        }
    }
    out
}


