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
const SO_HTTP_TYPES: &str = concat!(
    "/* ── HTTP types for .so handlers (MHD-free) ── */\n",
    "#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\n\n",
    "typedef struct {\n",
    "    const char* method;\n    const char* path;\n",
    "    const char* body;\n    size_t body_len;\n    void* connection;\n",
    "} HttpRequest;\n\n",
    "typedef struct {\n",
    "    int64_t status;\n    const char* body;\n    const char* content_type;\n",
    "} HttpResponse;\n\n"
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
pub fn emit_c(prog: &MonoProgram) -> CResult<String> {
    emit_c_impl(prog, false)
}

/// Emit C source targeting a shared library (.so) for mod_haki.
/// Suppresses `main()`, emits `haki_handle_request`, `haki_response_free`,
/// and `haki_abi_version` instead.
pub fn emit_c_so(prog: &MonoProgram) -> CResult<String> {
    emit_c_impl(prog, true)
}

fn emit_c_impl(prog: &MonoProgram, target_so: bool) -> CResult<String> {
    let mut out = String::with_capacity(64 * 1024);
    let cx = Cx { prog, self_fields: std::cell::RefCell::new(std::collections::HashSet::new()) };

    // Header
    if target_so {
        out.push_str("/* Generated by hakic --target so. Build: gcc -shared -fPIC -O2 -lpthread -lm -o handler.so this.c */\n");
    } else {
        out.push_str("/* Generated by hakic --emit-c. Compile: gcc -O2 -lpthread -lm -o out this.c */\n");
    }
    let core_runtime = RUNTIME_C_SOURCE
        .split("/* ── HTTP Server")
        .next()
        .unwrap_or(RUNTIME_C_SOURCE);
    out.push_str(core_runtime);
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

    // For --target so: inject the HTTP type definitions BEFORE function prototypes
    // so that HttpRequest/HttpResponse are defined when the compiler sees them
    if target_so {
        out.push_str(SO_HTTP_TYPES);
    }

    // Forward-declare all functions (so call order doesn't matter)
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
        cx.emit_fn(&mut out, f)?;
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
    /// When non-empty, `emit_var` prefixes matching names with `self->`.
    self_fields: std::cell::RefCell<std::collections::HashSet<String>>,
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
                    "Task"   => "void*".into(),
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
        if let Some(ref sup) = c.superclass {
            // Inherit superclass fields by value-embedding
            out.push_str(&format!("    {} __super;\n", c_name(sup)));
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

    fn emit_fn(&self, out: &mut String, f: &MonoFn) -> CResult<()> {
        out.push_str(&self.fn_prototype(f)?);
        out.push_str(" {\n");
        // main: call haki_runtime_init with argc/argv
        if f.name == "main" {
            out.push_str("    haki_runtime_init(argc, argv);\n");
        }
        // Closure capture unpacking:
        // For each captured variable, unpack from __env and also expand
        // any class fields of the capture into bare names so the body can
        // access them directly (e.g. `count` from `self.count`).
        if !f.captures.is_empty() {
            for (cap_name, cap_ty, _is_weak) in &f.captures {
                let c_type = self.c_ty(cap_ty);
                // Unpack the capture from __env
                // __env is the first capture cast directly (single-capture fast path)
                // For multiple captures we'd need a struct; for now handle single capture
                out.push_str(&format!("    {c_type} {cn} = ({c_type})__env;\n",
                    cn = c_name(cap_name)));
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
                                cn = c_name(cap_name),
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
                let cond = self.emit_expr(&i.cond)?;
                out.push_str(&format!("{indent}if ({cond}) {{\n"));
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
                let cond = self.emit_expr(&w.cond)?;
                out.push_str(&format!("{indent}while ({cond}) {{\n"));
                self.emit_block(out, &w.body, depth + 1, deferred)?;
                out.push_str(&format!("{indent}}}\n"));
            }

            MonoStmtKind::For(f) => {
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

            MonoStmtKind::Match(m) => {
                let scrutinee = self.emit_expr(&m.scrutinee)?;
                match m.kind {
                    MonoMatchKind::Int => {
                        // Integer match → C switch statement
                        out.push_str(&format!("{indent}switch ((int64_t)({scrutinee})) {{
"));
                        let mut has_default = false;
                        for arm in &m.arms {
                            match &arm.pattern {
                                MonoPattern::Int(n) => {
                                    out.push_str(&format!("{indent}    case {n}LL: {{
"));
                                    self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                    out.push_str(&format!("{indent}    break; }}
"));
                                }
                                MonoPattern::Named(s) if s == "_" => {
                                    has_default = true;
                                    out.push_str(&format!("{indent}    default: {{
"));
                                    self.emit_block(out, &arm.body, depth + 2, deferred)?;
                                    out.push_str(&format!("{indent}    break; }}
"));
                                }
                                _ => {}
                            }
                        }
                        if !has_default {
                            out.push_str(&format!("{indent}    default: break;
"));
                        }
                        out.push_str(&format!("{indent}}}
"));
                    }
                    MonoMatchKind::String => {
                        // String match → if-else strcmp chain
                        let mut first = true;
                        for arm in &m.arms {
                            match &arm.pattern {
                                MonoPattern::String(s) => {
                                    let kw = if first { "if" } else { "} else if" };
                                    first = false;
                                    out.push_str(&format!(
                                        "{indent}{kw} (strcmp((const char*)({scrutinee}), \"{s}\") == 0) {{\n"
                                    ));
                                    self.emit_block(out, &arm.body, depth + 1, deferred)?;
                                }
                                MonoPattern::Named(s) if s == "_" => {
                                    let kw = if first { "" } else { "} else " };
                                    first = false;
                                    out.push_str(&format!("{indent}{kw}{{
"));
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
                        out.push_str(&format!("{indent}{{ void* {sc_var} = (void*){scrutinee};
"));
                        out.push_str(&format!("{indent}    int64_t __tag = ((int64_t*){sc_var})[0];
"));
                        out.push_str(&format!("{indent}    void* __payload = ((void**){sc_var})[1];
"));
                        for (ai, arm) in m.arms.iter().enumerate() {
                            let prefix = if ai == 0 { "if".to_string() } else { "} else if".to_string() };
                            match &arm.pattern {
                                MonoPattern::Named(s) if s == "_" => {
                                    out.push_str(&format!("{indent}    }} else {{
"));
                                }
                                MonoPattern::Named(pname) => {
                                    let disc = self.variant_discriminant_by_name(pname);
                                    out.push_str(&format!("{indent}    {prefix} (__tag == {disc}LL) {{
"));
                                }
                                _ => {
                                    out.push_str(&format!("{indent}    {prefix} (1) {{
"));
                                }
                            }
                            if arm.bindings.len() == 1 {
                                let bt = self.c_ty(&arm.binding_tys[0]);
                                out.push_str(&format!(
                                    "{indent}        {bt} {} = *({bt}*)__payload;
",
                                    c_name(&arm.bindings[0].name)
                                ));
                            } else if arm.bindings.len() > 1 {
                                let struct_name = format!("__PayloadTuple{}", arm.binding_tys.len());
                                for (bi, (binding, bty)) in arm.bindings.iter().zip(arm.binding_tys.iter()).enumerate() {
                                    let bt = self.c_ty(bty);
                                    out.push_str(&format!(
                                        "{indent}        {bt} {} = (({struct_name}*)__payload)->f{bi};
",
                                        c_name(&binding.name)
                                    ));
                                }
                            }
                            self.emit_block(out, &arm.body, depth + 2, deferred)?;
                        }
                        out.push_str(&format!("{indent}    }}
{indent}}}
"));
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

            MonoStmtKind::Panic(msg) => {
                let ms = self.emit_expr(msg)?;
                out.push_str(&format!("{indent}haki_panic({ms});\n"));
            }

            MonoStmtKind::Expr(e) => {
                let es = self.emit_expr(e)?;
                if !es.is_empty() {
                    out.push_str(&format!("{indent}{};\n", es));
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
                let le = self.emit_expr(l)?;
                let re = self.emit_expr(r)?;
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
                // .length on Array or Map → runtime call
                if field == "length" {
                    match &recv.ty {
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
                // Look up the class/struct field types so we can cast pointer→int when needed
                let class_field_types: std::collections::HashMap<String, SemTy> =
                    self.prog.classes.iter()
                        .find(|c| c.name == *name)
                        .map(|c| c.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect())
                        .or_else(|| self.prog.structs.iter()
                            .find(|s| s.name == *name)
                            .map(|s| s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()))
                        .unwrap_or_default();

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
                    match &arm.pattern {
                        MonoPattern::Named(s) if s == "_" => {
                            parts.push("} else {".into());
                        }
                        MonoPattern::Named(pname) => {
                            let disc = self.variant_discriminant_by_name(pname);
                            parts.push(format!("{prefix} (__mtag == {disc}LL) {{"));
                        }
                        MonoPattern::Int(n) => {
                            parts.push(format!("{prefix} (__msc_int == {n}LL) {{"));
                        }
                        MonoPattern::String(s) => {
                            parts.push(format!("{prefix} (strcmp(__msc_str, \"{s}\") == 0) {{"));
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
                    // Emit arm body; find yield
                    if let Some(yv) = self.yield_val(&arm.body) {
                        parts.push(format!("__match_result = {yv};"));
                    }
                }
                parts.push("}".into());
                parts.push("__match_result;".into());
                Ok(format!("({{ {} }})", parts.join(" ")))
            }

            MonoExprKind::Block(b) => {
                if let Some(yv) = self.yield_val(b) {
                    Ok(format!("({{ {yv} }})"))
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
                // Async: wrap in task spawn — simplified for C backend
                let ie = self.emit_expr(inner)?;
                Ok(format!("haki_task_spawn_simple((void(*)(void*))({ie}), NULL)"))
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
        c_name(name)
    }

    fn emit_call(&self, name: &str, args: &[MonoExpr], ret_ty: &SemTy) -> CResult<String> {
        // ── Builtin intercepts ────────────────────────────────────────────────

        // print(any)
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
        if name.contains("__getOrDefault") {
            let map = self.emit_expr(&args[0])?;
            let key = self.emit_expr(&args[1])?;
            let def = self.emit_expr(&args[2])?;
            return Ok(format!("haki_map_get_or_default({map}, {key}, (void*)({def}))"));
        }
        if name == "writeFile"  {
            let p = self.emit_expr(&args[0])?;
            let c = self.emit_expr(&args[1])?;
            return Ok(format!("haki_write_file({p}, {c})"));
        }
        if name == "fileExists" {
            return Ok(format!("haki_file_exists({})", self.emit_expr(&args[0])?));
        }

        // thread_spawn
        if name == "thread_spawn" {
            let f = self.emit_expr(&args[0])?;
            return Ok(format!("haki_thread_spawn({f})"));
        }

        // std/env, std/time, std/process, std/regex — intercept before
        // the generic __get/__set/__append pattern matchers below, which
        // would otherwise match module-prefixed names like env__get.
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

        // Array methods: Array__T__method
        if name.contains("__append") {
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
        if name.contains("__length")  { return Ok(format!("haki_array_length({})", self.emit_expr(&args[0])?)); }
        // Array __get: requires exactly 2 args and receiver is array type
        if name.contains("__get") && args.len() == 2 && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Array") {
            return Ok(format!("haki_array_get({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }

        // Map methods — check receiver is Map type to avoid matching module functions
        if name.contains("__set") && args.len() == 3
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            return Ok(format!("haki_map_set({}, {}, {})",
                self.emit_expr(&args[0])?, self.emit_expr(&args[1])?, self.emit_expr(&args[2])?));
        }
        if name.contains("__has") && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            return Ok(format!("haki_map_has({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }
        if name.contains("__get") && args.len() == 2
            && matches!(args[0].ty, SemTy::Generic(ref n, _) if n == "Map") {
            return Ok(format!("haki_map_get({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?));
        }

        // String methods (Type__method mangled)
        if name == "string__substring" || name.ends_with("__substring") {
            return Ok(format!("haki_string_substring({}, {}, {})",
                self.emit_expr(&args[0])?, self.emit_expr(&args[1])?, self.emit_expr(&args[2])?));
        }
        if name.ends_with("__length") && args.len() == 1 && matches!(args[0].ty, SemTy::String) {
            return Ok(format!("haki_string_length({})", self.emit_expr(&args[0])?));
        }
        if name.ends_with("__contains") { return Ok(format!("haki_string_contains({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__split")    { return Ok(format!("haki_string_split({}, {})", self.emit_expr(&args[0])?, self.emit_expr(&args[1])?)); }
        if name.ends_with("__trim")     { return Ok(format!("haki_string_trim({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__toUpper")  { return Ok(format!("haki_string_to_upper({})", self.emit_expr(&args[0])?)); }
        if name.ends_with("__toLower")  { return Ok(format!("haki_string_to_lower({})", self.emit_expr(&args[0])?)); }

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
            "float"  => "double".into(),
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


