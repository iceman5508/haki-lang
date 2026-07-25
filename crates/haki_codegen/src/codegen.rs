/// codegen.rs — LLVM IR emitter for Haki v0.1.

use std::collections::HashMap;
use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    types::BasicType,
    values::{BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, PointerValue},
    IntPredicate, FloatPredicate,
};
use haki_ast::{BinaryOp, Binding, Mut, UnaryOp};
use haki_typeck::typed_ast::SemTy;
use haki_mono::mono_ast::*;
use crate::arc::ArcRuntime;
use crate::layout::Layout;
use crate::types::TypeMap;
use crate::error::{CodeGenError, CodeGenResult};

// ── Variable slot ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct VarSlot<'ctx> {
    ptr:       PointerValue<'ctx>,
    ty:        SemTy,
    #[allow(dead_code)]
    mutability: Mut,
    field_gep: Option<PointerValue<'ctx>>,
    /// True when ptr holds a `{ fn_ptr, env_ptr }` fat pointer (closure).
    is_closure: bool,
}

// ── Scope ─────────────────────────────────────────────────────────────────────

struct Scope<'ctx> {
    vars: HashMap<String, VarSlot<'ctx>>,
}
impl<'ctx> Scope<'ctx> {
    fn new() -> Self { Self { vars: HashMap::new() } }
}

// ── CodeGen ───────────────────────────────────────────────────────────────────

/// Stdlib function name → C runtime name mapping.
/// When a Haki call to e.g. `print` is lowered, the IR uses `haki_print`.
const STDLIB_NAME_MAP: &[(&str, &str)] = &[
    ("print",           "haki_print"),
    ("print_int",       "haki_print_int"),
    ("print_float",     "haki_print_float"),
    ("print_bool",      "haki_print_bool"),
    ("int_to_string",   "haki_int_to_string"),
    ("float_to_string", "haki_float_to_string"),
    ("bool_to_string",  "haki_bool_to_string"),
    ("string_length",   "haki_string_length"),
    ("string_concat",   "haki_string_concat"),
];

#[allow(dead_code)]
fn stdlib_c_name(haki_name: &str) -> &str {
    for (haki, c) in STDLIB_NAME_MAP {
        if *haki == haki_name { return c; }
    }
    haki_name
}

pub struct CodeGen<'ctx> {
    ctx:        &'ctx Context,
    module:     Module<'ctx>,
    builder:    Builder<'ctx>,
    tmap:       TypeMap<'ctx>,
    layout:     Layout<'ctx>,
    arc:        ArcRuntime<'ctx>,
    fns:        HashMap<String, FunctionValue<'ctx>>,
    scopes:     Vec<Scope<'ctx>>,
    current_fn: Option<FunctionValue<'ctx>>,
    program:    MonoProgram,
    /// Expressions deferred in the current function via `defer`.
    /// Emitted in LIFO order before every `return` in the function.
    deferred:   Vec<MonoExpr>,
    /// Stack of (cond_bb, exit_bb) for nested loops — supports continue/break.
    loop_stack: Vec<(inkwell::basic_block::BasicBlock<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(ctx: &'ctx Context, module_name: &str, program: &MonoProgram) -> Self {
        let module  = ctx.create_module(module_name);
        let builder = ctx.create_builder();
        let tmap    = TypeMap::new(ctx);
        let layout  = Layout::build(ctx, program).expect("layout");
        let arc     = ArcRuntime::declare(ctx, &module);
        Self {
            ctx, module, builder, tmap, layout, arc,
            fns: HashMap::new(),
            scopes: vec![Scope::new()],
            current_fn: None,
            program: program.clone(),
            deferred: Vec::new(),
            loop_stack: Vec::new(),
        }
    }

    pub fn emit(&mut self) -> CodeGenResult<()> {
        self.declare_stdlib_externs();
        self.declare_all()?;
        let fns = self.program.fns.clone();
        for f in &fns { self.emit_fn(f)?; }
        let structs = self.program.structs.clone();
        for s in &structs { for m in &s.methods.clone() { self.emit_fn(m)?; } }
        let classes = self.program.classes.clone();
        for c in &classes { for m in &c.methods.clone() { self.emit_fn(m)?; } }
        let impls = self.program.impls.clone();
        for i in &impls { for m in &i.methods.clone() { self.emit_fn(m)?; } }
        Ok(())
    }

    pub fn ir_string(&self) -> String {
        self.module.print_to_string().to_string()
    }

    // ── Declaration pass ──────────────────────────────────────────────────

    /// Declare all stdlib extern functions in the IR module.
    /// These resolve to C functions in haki_runtime.c at link time.
    fn declare_stdlib_externs(&mut self) {
        let ptr  = self.tmap.ptr();
        let void = self.ctx.void_type();
        let i64  = self.ctx.i64_type();
        let i8   = self.ctx.bool_type(); // i1 for bool

        let externs: &[(&str, inkwell::types::FunctionType)] = &[
            // Print
            ("haki_print",           void.fn_type(&[ptr.into()], false)),
            ("haki_print_int",       void.fn_type(&[i64.into()], false)),
            ("haki_print_float",     void.fn_type(&[self.ctx.f64_type().into()], false)),
            ("haki_print_bool",      void.fn_type(&[i8.into()], false)),
            // String
            ("haki_int_to_string",   ptr.fn_type(&[i64.into()], false)),
            ("haki_float_to_string", ptr.fn_type(&[self.ctx.f64_type().into()], false)),
            ("haki_bool_to_string",  ptr.fn_type(&[i8.into()], false)),
            ("haki_string_length",   i64.fn_type(&[ptr.into()], false)),
            ("haki_string_concat",   ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            // Panic
            ("haki_panic",           void.fn_type(&[ptr.into()], false)),
            // Array — all take/return opaque ptr; elem_size tells C the stride
            ("haki_array_new",          ptr.fn_type(&[i64.into()], false)),
            ("haki_array_from",         ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false)),
            ("haki_array_append",       void.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_array_get",          ptr.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_array_length",       i64.fn_type(&[ptr.into()], false)),
            ("haki_array_free",         void.fn_type(&[ptr.into()], false)),
            ("haki_array_remove_last",  ptr.fn_type(&[ptr.into()], false)),
            ("haki_array_remove_at",    ptr.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_array_contains_int", i64.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_array_contains_str", i64.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_array_index_of_int", i64.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_array_index_of_str", i64.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_array_first",        ptr.fn_type(&[ptr.into()], false)),
            ("haki_array_last",         ptr.fn_type(&[ptr.into()], false)),
            ("haki_array_join",         ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            // Map — string keys, opaque value blobs
            ("haki_map_new",    ptr.fn_type(&[i64.into()], false)),
            ("haki_map_set",    void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_map_get",    ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_map_has",    i8.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_map_delete", void.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_map_length", i64.fn_type(&[ptr.into()], false)),
            ("haki_map_free",   void.fn_type(&[ptr.into()], false)),
            ("haki_map_get_or_default", ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            // Thread — OS-level 1:1 pthread wrapper
            ("haki_thread_spawn", ptr.fn_type(&[ptr.into()], false)),
            ("haki_thread_join",  void.fn_type(&[ptr.into()], false)),
            // Task<T> — async call-site + thread pool
            ("haki_task_spawn",  ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_task_await",  ptr.fn_type(&[ptr.into()], false)),
            ("haki_task_detach", void.fn_type(&[ptr.into()], false)),
            // Mutex<T>
            ("haki_mutex_new",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_mutex_lock",   ptr.fn_type(&[ptr.into()], false)),
            ("haki_mutex_unlock", void.fn_type(&[ptr.into()], false)),
            ("haki_mutex_free",   void.fn_type(&[ptr.into()], false)),
            // String methods (v0.3)
            ("haki_string_split",       ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_string_trim",        ptr.fn_type(&[ptr.into()], false)),
            ("haki_string_trim_start",  ptr.fn_type(&[ptr.into()], false)),
            ("haki_string_trim_end",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_string_to_upper",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_string_to_lower",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_string_contains",    i8.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_string_starts_with", i8.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_string_ends_with",   i8.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_string_replace",     ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_string_index_of",    i64.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_string_substring",   ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false)),
            // File I/O (v0.3)
            ("haki_file_read",   void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_file_write",  ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_file_exists", i8.fn_type(&[ptr.into()], false)),
            // HTTP server (v0.4)
            ("haki_http_server_new",      ptr.fn_type(&[i64.into(), ptr.into()], false)),
            ("haki_http_server_listen",   void.fn_type(&[ptr.into()], false)),
            ("haki_http_request_path",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_http_request_method",  ptr.fn_type(&[ptr.into()], false)),
            ("haki_http_request_body",    ptr.fn_type(&[ptr.into()], false)),
            ("haki_http_response_new",    ptr.fn_type(&[i64.into(), ptr.into()], false)),
            ("haki_http_response_json",   ptr.fn_type(&[i64.into(), ptr.into()], false)),
            ("haki_http_response_typed",  ptr.fn_type(&[i64.into(), ptr.into(), ptr.into()], false)),
            ("haki_http_response_status", i64.fn_type(&[ptr.into()], false)),
            ("haki_http_response_body",   ptr.fn_type(&[ptr.into()], false)),
            // Router (v0.4.5)
            ("haki_router_new",      ptr.fn_type(&[], false)),
            ("haki_router_add",      void.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_router_dispatch", ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_request_param",   ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_request_query",   ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            // Phase 4: template + static files
            ("haki_render_template", void.fn_type(&[ptr.into(), ptr.into(), ptr.into(), i64.into(), ptr.into(), ptr.into()], false)),
            ("haki_render_template_map", void.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_serve_file",      ptr.fn_type(&[ptr.into()], false)),
            // Phase 5: JSON decode
            ("haki_json_decode",     void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_json_decode_get", ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            // Error type
            ("haki_error_new",            ptr.fn_type(&[ptr.into()], false)),
            ("haki_error_new_with_cause", ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_error_wrap",           ptr.fn_type(&[ptr.into()], false)),
            ("haki_error_message",        ptr.fn_type(&[ptr.into()], false)),
            ("haki_error_cause",          ptr.fn_type(&[ptr.into()], false)),
            // haki_ui — v0.5
            ("haki_text_new",      ptr.fn_type(&[ptr.into()], false)),
            ("haki_text_set",      void.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_button_new",    ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_textfield_new", ptr.fn_type(&[ptr.into(), ptr.into()], false)),
            ("haki_vstack_new",    ptr.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_hstack_new",    ptr.fn_type(&[ptr.into(), i64.into()], false)),
            ("haki_spacer_new",    ptr.fn_type(&[], false)),
            ("haki_app_run",       void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            ("haki_ui_request_rerender", void.fn_type(&[], false)),
            // JSON (v0.4)
            ("haki_json_string", ptr.fn_type(&[ptr.into()], false)),
            ("haki_json_int",    ptr.fn_type(&[i64.into()], false)),
            ("haki_json_float",  ptr.fn_type(&[self.ctx.f64_type().into()], false)),
            ("haki_json_bool",   ptr.fn_type(&[i8.into()], false)),
            ("haki_json_concat", ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false)),
            // malloc / free
            ("malloc",  ptr.fn_type(&[i64.into()], false)),
            ("free",    void.fn_type(&[ptr.into()], false)),
        ];

        for (name, ft) in externs {
            if self.module.get_function(name).is_none() {
                let fv = self.module.add_function(name, *ft, None);
                self.fns.insert(name.to_string(), fv);
            }
        }

        // Register Haki-level aliases that map to C names.
        for (haki_name, c_name) in STDLIB_NAME_MAP {
            if let Some(&fv) = self.fns.get(*c_name) {
                self.fns.entry(haki_name.to_string()).or_insert(fv);
            }
        }
    }

    fn declare_all(&mut self) -> CodeGenResult<()> {
        let all: Vec<MonoFn> = {
            let mut v = self.program.fns.clone();
            for s in &self.program.structs { v.extend(s.methods.clone()); }
            for c in &self.program.classes { v.extend(c.methods.clone()); }
            for i in &self.program.impls   { v.extend(i.methods.clone()); }
            v
        };
        for f in &all { self.declare_fn(f)?; }
        Ok(())
    }

    fn declare_fn(&mut self, f: &MonoFn) -> CodeGenResult<FunctionValue<'ctx>> {
        if let Some(&fv) = self.fns.get(&f.name) { return Ok(fv); }
        let param_tys: Vec<_> = f.params.iter()
            .map(|p| self.tmap.metadata(&p.ty))
            .collect::<CodeGenResult<_>>()?;

        // `main` must be declared as `i32 main()` per the C ABI.
        // The OS expects an integer exit code; void main leaves the exit
        // code undefined (whatever happens to be in eax).
        let fv = if f.name == "main" {
            let i32_ty = self.ctx.i32_type();
            let ptr = self.tmap.ptr();
            // Declare main as i32 main(i32 argc, ptr argv) so we can pass
            // argc/argv to haki_runtime_init. User Haki code sees no parameters.
            let ft = i32_ty.fn_type(&[i32_ty.into(), ptr.into()], false);
            self.module.add_function("main", ft, None)
        } else {
            match self.tmap.return_type(&f.return_ty)? {
                None => {
                    let ft = self.ctx.void_type().fn_type(&param_tys, false);
                    self.module.add_function(&f.name, ft, None)
                }
                Some(rt) => {
                    let ft = rt.fn_type(&param_tys, false);
                    self.module.add_function(&f.name, ft, None)
                }
            }
        };
        self.fns.insert(f.name.clone(), fv);
        Ok(fv)
    }

    // ── Function body ─────────────────────────────────────────────────────

    fn emit_fn(&mut self, f: &MonoFn) -> CodeGenResult<()> {
        let fv = *self.fns.get(&f.name)
            .ok_or_else(|| CodeGenError::UnknownFn(f.name.clone()))?;
        self.current_fn = Some(fv);
        self.scopes = vec![Scope::new()];
        self.deferred.clear();

        let entry = self.ctx.append_basic_block(fv, "entry");
        self.builder.position_at_end(entry);

        // For `main`: call haki_runtime_init with real argc/argv from OS.
        if f.name == "main" {
            let init_fn = self.module.get_function("haki_runtime_init").unwrap_or_else(|| {
                let ptr = self.tmap.ptr();
                let i32_ty = self.ctx.i32_type();
                let ft = self.ctx.void_type().fn_type(&[i32_ty.into(), ptr.into()], false);
                self.module.add_function("haki_runtime_init", ft, None)
            });
            // argc is param 0, argv is param 1 of the LLVM main function.
            let argc = fv.get_nth_param(0).unwrap();
            let argv = fv.get_nth_param(1).unwrap();
            let _ = self.builder.build_call(
                init_fn,
                &[argc.into(), argv.into()],
                "runtime_init",
            );
        }

        // Bind parameters.
        for (i, param) in f.params.iter().enumerate() {
            let lp = fv.get_nth_param(i as u32)
                .ok_or_else(|| CodeGenError::BuildError(format!("param {i} on {}", f.name)))?;
            lp.set_name(&param.name);
            let ll_ty = match self.tmap.basic(&param.ty) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let slot = self.builder.build_alloca(ll_ty, &param.name)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(slot, lp)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            // fn-typed params hold fat pointers under the uniform ABI convention.
            let is_fn_param = matches!(param.ty, SemTy::Fn(_, _) | SemTy::Closure(_, _));
            self.scopes.last_mut().unwrap().vars.insert(
                param.name.clone(),
                VarSlot { ptr: slot, ty: param.ty.clone(), mutability: Mut::Const,
                          field_gep: None, is_closure: is_fn_param },
            );
        }

        // If this function is a closure (has captures), unpack the env struct
        // into named locals. The first param is `__env: ptr`.
        if !f.captures.is_empty() {
            if let Some(env_param) = f.params.first() {
                if env_param.name == "__env" {
                    // Load __env pointer.
                    let env_ptr_slot = self.scopes.last().unwrap().vars.get("__env")
                        .map(|s| s.ptr);
                    if let Some(env_slot) = env_ptr_slot {
                        let ptr = self.tmap.ptr();
                        let env_ptr = self.builder.build_load(ptr, env_slot, "env_ptr")
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                            .into_pointer_value();

                        // Build the env struct type from capture types.
                        let cap_tys: Vec<_> = f.captures.iter()
                            .map(|(_, ty, _)| self.tmap.basic(ty).unwrap_or(ptr.into()))
                            .collect();
                        let env_struct_ty = self.ctx.struct_type(&cap_tys, false);

                        // GEP each field and store as a named local.
                        for (i, (cap_name, cap_ty, _)) in f.captures.iter().enumerate() {
                            if let Ok(field_ty) = self.tmap.basic(cap_ty) {
                                let fp = self.builder.build_struct_gep(
                                    env_struct_ty, env_ptr, i as u32, &format!("env_{i}")
                                ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                                let val = self.builder.build_load(field_ty, fp, cap_name)
                                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                                let slot = self.builder.build_alloca(field_ty, cap_name)
                                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                                self.builder.build_store(slot, val)
                                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                                // fn-typed captures are fat pointers — mark accordingly.
                                let is_fn_cap = matches!(cap_ty, SemTy::Fn(_, _) | SemTy::Closure(_, _));
                                self.scopes.last_mut().unwrap().vars.insert(
                                    cap_name.clone(),
                                    VarSlot { ptr: slot, ty: cap_ty.clone(), mutability: Mut::Const,
                                              field_gep: None, is_closure: is_fn_cap },
                                );
                            }
                        }
                    }
                }
            }
        }

        // If this is a method (first param is `self`), inject all fields of
        // the receiver type as named locals — exactly what the typechecker does.
        // This lets bare field names like `x` resolve inside method bodies.
        if let Some(self_param) = f.params.first() {
            if self_param.name == "self" {
                let type_name = match &self_param.ty {
                    SemTy::Named(n) => Some(n.clone()),
                    _ => None,
                };
                if let Some(ref tname) = type_name {
                    // Collect field info without borrowing self mutably.
                    // Walk the full inheritance chain so superclass fields
                    // are also visible inside subclass methods.
                    let field_data: Vec<(String, SemTy, usize, bool)> = {
                        let mut out = vec![];
                        for s in &self.program.structs {
                            if &s.name == tname {
                                for (fi, field) in s.fields.iter().enumerate() {
                                    out.push((field.name.clone(), field.ty.clone(), fi, false));
                                }
                            }
                        }
                        // For classes, walk the inheritance chain.
                        // Collect classes in order from most-base to most-derived.
                        let mut chain: Vec<String> = vec![];
                        let mut cur = tname.clone();
                        loop {
                            chain.push(cur.clone());
                            let parent = self.program.classes.iter()
                                .find(|c| c.name == cur)
                                .and_then(|c| c.superclass.clone());
                            match parent {
                                Some(p) => cur = p,
                                None => break,
                            }
                        }
                        chain.reverse(); // base first
                        let mut field_offset = 0usize;
                        for class_name in &chain {
                            for c in &self.program.classes {
                                if &c.name == class_name {
                                    for (fi, field) in c.fields.iter().enumerate() {
                                        out.push((field.name.clone(), field.ty.clone(), field_offset + fi, true));
                                    }
                                    field_offset += c.fields.len();
                                }
                            }
                        }
                        out
                    };

                    // Load each field from self and bind it as a local.
                    for (field_name, field_ty, field_pos, is_class) in field_data {
                        let Ok(ll_field_ty) = self.tmap.basic(&field_ty) else { continue; };
                        let llvm_idx = (field_pos + if is_class { 1 } else { 0 }) as u32;

                        // Get self pointer from its alloca slot.
                        let self_slot = match self.scopes.last().unwrap().vars.get("self") {
                            Some(v) => v.ptr,
                            None => continue,
                        };
                        let self_ptr_ty = self.tmap.ptr();
                        let self_val = match self.builder.build_load(self_ptr_ty, self_slot, "self_ptr") {
                            Ok(v) => v.into_pointer_value(),
                            Err(_) => continue,
                        };

                        // GEP into self to get field pointer.
                        let struct_ty = match self.layout.get(tname) {
                            Some(t) => t,
                            None => continue,
                        };
                        let field_ptr = match self.builder.build_struct_gep(
                            struct_ty, self_val, llvm_idx, &format!("self_{field_name}")
                        ) {
                            Ok(p) => p,
                            Err(_) => continue,
                        };

                        // Load the field value and bind it.
                        let field_val = match self.builder.build_load(
                            ll_field_ty, field_ptr, &field_name
                        ) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        // Store into a local slot. Keep the GEP pointer so
                        // assignments inside the method body write back to the struct.
                        let field_slot = match self.builder.build_alloca(ll_field_ty, &field_name) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let _ = self.builder.build_store(field_slot, field_val);
                        self.scopes.last_mut().unwrap().vars.insert(
                            field_name,
                            VarSlot { ptr: field_slot, ty: field_ty, mutability: Mut::Let, field_gep: Some(field_ptr), is_closure: false },
                        );
                    }
                }
            }
        }

        self.emit_block(&f.body)?;

        // Add a terminator if the current insert block lacks one.
        // If the block has no predecessors it is dead (all real paths already
        // returned) — emit `unreachable` rather than a type-incorrect `ret void`.
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                let has_preds = bb.get_first_use().is_some();
                let name = bb.get_name().to_str().unwrap_or("");
                let is_dead_merge = !has_preds
                    && (name.contains("merge") || name.contains("exit")
                        || name.contains("arm"));
                if is_dead_merge {
                    self.builder.build_unreachable()
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                } else if f.name == "main" {
                    // main() must return i32 0 — the OS exit code.
                    let zero = self.ctx.i32_type().const_int(0, false);
                    self.builder.build_return(Some(&zero))
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                } else {
                    self.builder.build_return(None)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }
            }
        }
        self.current_fn = None;
        Ok(())
    }

    // ── Block ─────────────────────────────────────────────────────────────

    fn emit_block(&mut self, block: &MonoBlock) -> CodeGenResult<()> {
        self.scopes.push(Scope::new());
        for stmt in &block.stmts { self.emit_stmt(stmt)?; }
        self.emit_scope_release();
        self.scopes.pop();
        Ok(())
    }

    fn emit_scope_release(&self) {
        let scope = self.scopes.last().unwrap();
        for slot in scope.vars.values() {
            if TypeMap::is_arc_type(&slot.ty, &self.layout) {
                let ptr_ty = self.tmap.ptr();
                if let Ok(v) = self.builder.build_load(ptr_ty, slot.ptr, "rel_load") {
                    let _ = self.builder.build_call(self.arc.release, &[v.into()], "release");
                }
            }
        }
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &MonoStmt) -> CodeGenResult<()> {
        match &stmt.kind {
            MonoStmtKind::Let(l)    => self.emit_let(l),
            MonoStmtKind::Return(r) => self.emit_return(r),
            MonoStmtKind::Yield(_)  => Ok(()),
            MonoStmtKind::Defer(e)  => {
                self.deferred.push(*e.clone());
                Ok(())
            }
            MonoStmtKind::Continue => {
                if let Some(&(cond_bb, _)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(cond_bb)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    // Absorb unreachable code into a dead block.
                    let fv = self.current_fn.unwrap();
                    let dead = self.ctx.append_basic_block(fv, "continue.dead");
                    self.builder.position_at_end(dead);
                }
                Ok(())
            }
            MonoStmtKind::Break => {
                if let Some(&(_, exit_bb)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(exit_bb)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    let fv = self.current_fn.unwrap();
                    let dead = self.ctx.append_basic_block(fv, "break.dead");
                    self.builder.position_at_end(dead);
                }
                Ok(())
            }
            MonoStmtKind::If(i)     => { self.emit_if(i)?; Ok(()) }
            MonoStmtKind::While(w)  => self.emit_while(w),
            MonoStmtKind::For(f)    => self.emit_for(f),
            MonoStmtKind::Match(m)  => { self.emit_match(m)?; Ok(()) }
            MonoStmtKind::Panic(e)  => self.emit_panic(e),
            MonoStmtKind::Expr(e)   => { self.emit_expr(e)?; Ok(()) }
        }
    }

    fn emit_let(&mut self, l: &MonoLetStmt) -> CodeGenResult<()> {
        // Under the uniform fat-ptr ABI, ALL fn(T)->R typed values are fat pointers
        // {fn_ptr, env_ptr}. This includes: fn_lit vars, HOF return values, params.
        // Mark any fn/closure typed binding as is_closure so the indirect call
        // path correctly GEPs the fat struct to extract fn_ptr and env_ptr.
        let is_fat_ptr = match &l.bindings.as_slice() {
            [(_, ty)] => matches!(ty, SemTy::Fn(_, _) | SemTy::Closure(_, _)),
            _ => false,
        };

        let init = self.emit_expr(&l.init)?;
        match l.bindings.as_slice() {
            [(binding, ty)] => {
                let Ok(ll_ty) = self.tmap.basic(ty) else { return Ok(()); };
                let slot = self.builder.build_alloca(ll_ty, "let")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                if let Some(v) = init {
                    self.builder.build_store(slot, v)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    if TypeMap::is_arc_type(ty, &self.layout) {
                        let _ = self.builder.build_call(self.arc.retain, &[v.into()], "retain");
                    }
                }
                if let Binding::Name(id) = binding {
                    // Always use the alloca slot — even for fn_lits.
                    // The alloca holds the fat ptr address (a pointer-to-fat-struct).
                    // This matches the param binding convention (alloca → fat ptr value).
                    self.scopes.last_mut().unwrap().vars.insert(
                        id.name.clone(),
                        VarSlot { ptr: slot, ty: ty.clone(), mutability: l.mutability,
                                  field_gep: None, is_closure: is_fat_ptr },
                    );
                }
            }
            bindings => {
                // Multi-binding: init is a pointer to a tuple struct.
                // Build a struct type from the binding types for GEP.
                let field_ll_tys: Vec<_> = bindings.iter()
                    .filter_map(|(_, ty)| self.tmap.basic(ty).ok())
                    .collect();
                let tuple_struct_ty = self.ctx.struct_type(&field_ll_tys, false);

                for (idx, (binding, ty)) in bindings.iter().enumerate() {
                    let Binding::Name(id) = binding else { continue; };
                    let Ok(ll_ty) = self.tmap.basic(ty) else { continue; };
                    let slot = self.builder.build_alloca(ll_ty, &id.name)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    if let Some(tuple_ptr) = init {
                        if let Ok(fp) = self.builder.build_struct_gep(
                            tuple_struct_ty, tuple_ptr.into_pointer_value(), idx as u32, "t_field"
                        ) {
                            if let Ok(v) = self.builder.build_load(ll_ty, fp, "t_load") {
                                let _ = self.builder.build_store(slot, v);
                            }
                        }
                    }
                    self.scopes.last_mut().unwrap().vars.insert(
                        id.name.clone(), VarSlot { ptr: slot, ty: ty.clone(), mutability: l.mutability, field_gep: None, is_closure: false },
                    );
                }
            }
        }
        Ok(())
    }

    fn emit_return(&mut self, r: &MonoReturnStmt) -> CodeGenResult<()> {
        // Release all locals before returning.
        for scope in &self.scopes {
            for slot in scope.vars.values() {
                if TypeMap::is_arc_type(&slot.ty, &self.layout) {
                    let ptr_ty = self.tmap.ptr();
                    if let Ok(v) = self.builder.build_load(ptr_ty, slot.ptr, "ret_rel") {
                        let _ = self.builder.build_call(self.arc.release, &[v.into()], "release");
                    }
                }
            }
        }

        // main() is declared as i32 — always return i32 0.
        let in_main = self.current_fn
            .and_then(|f| f.get_name().to_str().ok().map(|s| s == "main"))
            .unwrap_or(false);

        if in_main {
            let zero = self.ctx.i32_type().const_int(0, false);
            self.builder.build_return(Some(&zero))
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(());
        }

        match r.values.as_slice() {
            [] => {
                // Emit deferred expressions in LIFO order before returning.
                let deferred = self.deferred.clone();
                for expr in deferred.iter().rev() { let _ = self.emit_expr(expr); }
                self.builder.build_return(None)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
            [single] => {
                if let Some(v) = self.emit_expr(single)? {
                    let deferred = self.deferred.clone();
                    for expr in deferred.iter().rev() { let _ = self.emit_expr(expr); }
                    self.builder.build_return(Some(&v))
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                } else {
                    let deferred = self.deferred.clone();
                    for expr in deferred.iter().rev() { let _ = self.emit_expr(expr); }
                    self.builder.build_return(None)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }
            }
            multi => {
                // Emit deferred expressions before returning.
                let deferred = self.deferred.clone();
                for expr in deferred.iter().rev() { let _ = self.emit_expr(expr); }

                // Full multi-return: allocate a { T1, T2, ... } struct on the heap,                // fill each field, and return a pointer to it.
                // The caller uses GEP to destructure: `const a, b = fn()`.
                let ptr = self.tmap.ptr();
                let i64 = self.ctx.i64_type();

                // Build the LLVM struct type from the return types.
                let mut field_types: Vec<inkwell::types::BasicTypeEnum> = Vec::new();
                for val in multi.iter() {
                    let ll_ty = match self.tmap.basic(&val.ty) {
                        Ok(t) => t,
                        Err(_) => ptr.into(), // fallback for complex types
                    };
                    field_types.push(ll_ty);
                }
                let struct_ty = self.ctx.struct_type(&field_types, false);

                // Heap-allocate the struct (malloc + cast).
                let size_val = struct_ty.size_of()
                    .ok_or_else(|| CodeGenError::BuildError("unsized tuple".into()))?;
                let malloc_fn = self.module.get_function("malloc").ok_or_else(|| {
                    CodeGenError::UnknownFn("malloc".into())
                })?;
                let malloc_size = self.builder.build_int_z_extend(size_val, i64, "tuple_size")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let raw = self.builder.build_call(malloc_fn, &[malloc_size.into()], "tuple_alloc")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                    .try_as_basic_value().left()
                    .unwrap_or(ptr.const_null().into());

                // Store each value into the struct fields.
                for (i, val_expr) in multi.iter().enumerate() {
                    if let Some(val) = self.emit_expr(val_expr)? {
                        let field_ptr = self.builder.build_struct_gep(
                            struct_ty, raw.into_pointer_value(),
                            i as u32, &format!("ret_field_{i}")
                        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        self.builder.build_store(field_ptr, val)
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    }
                }

                self.builder.build_return(Some(&raw))
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn emit_while(&mut self, w: &MonoWhile) -> CodeGenResult<()> {
        let fv = self.current_fn.unwrap();
        let cond_bb  = self.ctx.append_basic_block(fv, "while.cond");
        let body_bb  = self.ctx.append_basic_block(fv, "while.body");
        let exit_bb  = self.ctx.append_basic_block(fv, "while.exit");

        self.loop_stack.push((cond_bb, exit_bb));

        self.builder.build_unconditional_branch(cond_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.position_at_end(cond_bb);
        let cond = self.emit_expr(&w.cond)?.unwrap().into_int_value();
        self.builder.build_conditional_branch(cond, body_bb, exit_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.position_at_end(body_bb);
        self.emit_block(&w.body)?;
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(cond_bb)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }
        self.loop_stack.pop();
        self.builder.position_at_end(exit_bb);
        Ok(())
    }

    fn emit_for(&mut self, f: &MonoFor) -> CodeGenResult<()> {
        let fv = self.current_fn.unwrap();
        let i64_ty = self.ctx.i64_type();

        let arr_val = self.emit_expr(&f.iter)?.unwrap();
        let arr_ptr = arr_val.into_pointer_value();

        // Allocate loop counter.
        let i_slot = self.builder.build_alloca(i64_ty, "for.i")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(i_slot, i64_ty.const_int(0, false))
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Get array length via haki_array_length(ptr) → i64
        let len_fn = *self.fns.get("haki_array_length")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_length".into()))?;
        let arr_len_call = self.builder.build_call(len_fn, &[arr_ptr.into()], "arr_len")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let arr_len = arr_len_call.try_as_basic_value().left().unwrap().into_int_value();

        let cond_bb = self.ctx.append_basic_block(fv, "for.cond");
        let body_bb = self.ctx.append_basic_block(fv, "for.body");
        let incr_bb = self.ctx.append_basic_block(fv, "for.incr");  // continue target
        let exit_bb = self.ctx.append_basic_block(fv, "for.exit");

        // continue → incr_bb (re-evaluate condition after increment)
        // break    → exit_bb
        self.loop_stack.push((incr_bb, exit_bb));

        self.builder.build_unconditional_branch(cond_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        self.builder.position_at_end(cond_bb);
        let i_val = self.builder.build_load(i64_ty, i_slot, "for.i")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_int_value();
        let cond = self.builder.build_int_compare(IntPredicate::SLT, i_val, arr_len, "for.cond")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_conditional_branch(cond, body_bb, exit_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        self.scopes.push(Scope::new());

        // If `for i, v in arr` — expose the counter as the index variable.
        if let Some(idx) = &f.index_var {
            let idx_slot = self.builder.build_alloca(i64_ty, &idx.name)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            // Reload current i (we're now in the body block).
            let i_cur = self.builder.build_load(i64_ty, i_slot, "idx_val")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(idx_slot, i_cur)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.scopes.last_mut().unwrap().vars.insert(
                idx.name.clone(),
                VarSlot { ptr: idx_slot, ty: SemTy::Int, mutability: Mut::Const, field_gep: None, is_closure: false },
            );
        }

        // Get element pointer via haki_array_get(ptr, i64) → ptr, then load
        if let Ok(elem_ty) = self.tmap.basic(&f.var_ty) {
            let get_fn = *self.fns.get("haki_array_get")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_array_get".into()))?;
            let elem_ptr_call = self.builder.build_call(
                get_fn, &[arr_ptr.into(), i_val.into()], "for_get"
            ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let elem_ptr = elem_ptr_call.try_as_basic_value()
                .left().unwrap().into_pointer_value();
            let elem_val = self.builder.build_load(elem_ty, elem_ptr, "for_elem")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let var_slot = self.builder.build_alloca(elem_ty, &f.var.name)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(var_slot, elem_val)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.scopes.last_mut().unwrap().vars.insert(
                f.var.name.clone(),
                VarSlot { ptr: var_slot, ty: f.var_ty.clone(), mutability: Mut::Const,
                          field_gep: None,
                          is_closure: matches!(f.var_ty, SemTy::Fn(_, _) | SemTy::Closure(_, _)) },
            );
        }

        for stmt in &f.body.stmts { self.emit_stmt(stmt)?; }
        self.emit_scope_release();
        self.scopes.pop();

        // Body falls through to incr_bb (also the continue target).
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(incr_bb)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }

        // Increment i in incr_bb, then loop back to cond.
        self.builder.position_at_end(incr_bb);
        let i_cur = self.builder.build_load(i64_ty, i_slot, "i_cur")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_int_value();
        let i_next = self.builder.build_int_add(i_cur, i64_ty.const_int(1, false), "i_next")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(i_slot, i_next)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_unconditional_branch(cond_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        self.loop_stack.pop();
        self.builder.position_at_end(exit_bb);
        Ok(())
    }

    fn emit_panic(&mut self, msg: &MonoExpr) -> CodeGenResult<()> {
        let panic_fn = self.module.get_function("haki_panic").unwrap_or_else(|| {
            let ptr = self.tmap.ptr();
            let ft  = self.ctx.void_type().fn_type(&[ptr.into()], false);
            self.module.add_function("haki_panic", ft, None)
        });
        let msg_val = self.emit_expr(msg)?.unwrap_or_else(|| {
            self.tmap.ptr().const_null().into()
        });
        self.builder.build_call(panic_fn, &[msg_val.into()], "panic")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_unreachable()
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(())
    }

    // ── If ────────────────────────────────────────────────────────────────

    fn emit_if(&mut self, i: &MonoIf) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let fv = self.current_fn.unwrap();
        let cond  = self.emit_expr(&i.cond)?.unwrap().into_int_value();
        let then_bb  = self.ctx.append_basic_block(fv, "if.then");
        let else_bb  = self.ctx.append_basic_block(fv, "if.else");
        let merge_bb = self.ctx.append_basic_block(fv, "if.merge");

        self.builder.build_conditional_branch(cond, then_bb, else_bb)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Then block
        self.builder.position_at_end(then_bb);
        let mut then_yield: Option<BasicValueEnum> = None;
        self.scopes.push(Scope::new());
        for stmt in &i.then_block.stmts {
            if let MonoStmtKind::Yield(e) = &stmt.kind { then_yield = self.emit_expr(e)?; }
            else { self.emit_stmt(stmt)?; }
        }
        self.emit_scope_release();
        self.scopes.pop();
        let then_exit = self.builder.get_insert_block().unwrap();
        if then_exit.get_terminator().is_none() {
            self.builder.build_unconditional_branch(merge_bb)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        }

        // Else block
        self.builder.position_at_end(else_bb);
        let mut else_yield: Option<BasicValueEnum> = None;
        if let Some(else_br) = &i.else_branch {
            match else_br {
                MonoElse::Block(b) => {
                    self.scopes.push(Scope::new());
                    for stmt in &b.stmts {
                        if let MonoStmtKind::Yield(e) = &stmt.kind { else_yield = self.emit_expr(e)?; }
                        else { self.emit_stmt(stmt)?; }
                    }
                    self.emit_scope_release();
                    self.scopes.pop();
                }
                MonoElse::If(inner) => { else_yield = self.emit_if(inner)?; }
            }
        }
        let else_exit = self.builder.get_insert_block().unwrap();
        if else_exit.get_terminator().is_none() {
            self.builder.build_unconditional_branch(merge_bb)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        }

        self.builder.position_at_end(merge_bb);

        // If no predecessor branched to merge_bb, all paths terminated
        // (returned/panicked). Emit unreachable so the block is valid IR.
        let merge_has_preds = then_exit.get_terminator()
            .map_or(false, |t| t.get_num_operands() > 0
                && t.get_operand(0).map_or(false, |_| true));
        // Simpler check: if both exits already had terminators that were NOT
        // unconditional branches to merge, merge is dead.
        let then_terminated = then_exit.get_terminator().is_some();
        let else_terminated = else_exit.get_terminator().is_some();
        // Check if the terminators are branches TO merge_bb.
        // We approximate: if both blocks terminated AND neither branched to
        // merge_bb (i.e. they returned/unreachable'd), the merge is dead.
        let then_br_to_merge = then_exit.get_terminator()
            .map_or(false, |t| format!("{:?}", t).contains("if.merge"));
        let else_br_to_merge = else_exit.get_terminator()
            .map_or(false, |t| format!("{:?}", t).contains("if.merge"));
        let _ = merge_has_preds;

        if then_terminated && else_terminated && !then_br_to_merge && !else_br_to_merge {
            // Dead merge block — all real paths terminated.
            self.builder.build_unreachable()
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(None);
        }

        // PHI if both branches yielded.
        if let (Some(tv), Some(ev)) = (then_yield, else_yield) {
            if let Ok(ll_ty) = self.tmap.basic(&i.ty) {
                let phi = self.builder.build_phi(ll_ty, "if.phi")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                phi.add_incoming(&[
                    (&tv as &dyn BasicValue, then_exit),
                    (&ev as &dyn BasicValue, else_exit),
                ]);
                return Ok(Some(phi.as_basic_value()));
            }
        }
        Ok(None)
    }

    // ── Match ─────────────────────────────────────────────────────────────

    fn emit_match(&mut self, m: &MonoMatch) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let fv = self.current_fn.unwrap();
        let scrutinee = self.emit_expr(&m.scrutinee)?;
        let merge_bb = self.ctx.append_basic_block(fv, "match.merge");
        let mut phi_parts: Vec<(BasicValueEnum<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        // Determine the scrutinee's enum type name for variant disambiguation.
        let scrutinee_enum_name = match &m.scrutinee.ty {
            SemTy::Named(n) => n.clone(),
            _ => String::new(),
        };

        // Determine if this is an enum match by checking if any arm's pattern
        // is a variant in the scrutinee's specific enum.
        let is_enum_match = m.arms.iter().any(|arm| {
            if arm.pattern == "_" { return false; }
            if !scrutinee_enum_name.is_empty() {
                self.find_variant_in(&arm.pattern, &scrutinee_enum_name).is_some()
            } else {
                self.find_variant(&arm.pattern).is_some()
            }
        });

        if is_enum_match {
            // Extract the discriminant tag from the { i64 tag, ptr payload } enum struct.
            let i64 = self.ctx.i64_type();
            let ptr = self.tmap.ptr();
            let enum_ty = self.ctx.struct_type(&[i64.into(), ptr.into()], false);
            let scrutinee_ptr = scrutinee.unwrap().into_pointer_value();
            let tag_ptr = self.builder.build_struct_gep(enum_ty, scrutinee_ptr, 0, "tag_ptr")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let tag = self.builder.build_load(i64, tag_ptr, "tag")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_int_value();

            // Build arm body blocks.
            let default_bb = self.ctx.append_basic_block(fv, "match.default");
            let mut arm_bbs: Vec<inkwell::basic_block::BasicBlock> = Vec::new();
            for (idx, _) in m.arms.iter().enumerate() {
                arm_bbs.push(self.ctx.append_basic_block(fv, &format!("match.arm{idx}")));
            }

            // Build switch: one case per non-wildcard arm.
            let mut cases: Vec<(inkwell::values::IntValue, inkwell::basic_block::BasicBlock)> = Vec::new();
            for (arm, &arm_bb) in m.arms.iter().zip(arm_bbs.iter()) {
                if arm.pattern == "_" { continue; }
                let variant_result = if !scrutinee_enum_name.is_empty() {
                    self.find_variant_in(&arm.pattern, &scrutinee_enum_name)
                } else {
                    self.find_variant(&arm.pattern)
                };
                if let Some((_, disc, _)) = variant_result {
                    cases.push((i64.const_int(disc as u64, false), arm_bb));
                }
            }
            // Wildcard arm becomes the default; if none, default falls through to merge.
            let wildcard_bb = m.arms.iter().zip(arm_bbs.iter())
                .find(|(a, _)| a.pattern == "_")
                .map(|(_, &bb)| bb)
                .unwrap_or(default_bb);

            self.builder.build_switch(tag, wildcard_bb, &cases)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

            // Emit default block — should be unreachable for exhaustive enum matches.
            // (Wildcards are handled as explicit switch cases, so this is dead.)
            self.builder.position_at_end(default_bb);
            self.builder.build_unreachable()
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

            // Emit each arm body.
            for (arm, arm_bb) in m.arms.iter().zip(arm_bbs.iter()) {
                self.builder.position_at_end(*arm_bb);
                self.scopes.push(Scope::new());

                // Extract payload pointer for binding.
                let payload_ptr_ptr = self.builder.build_struct_gep(
                    enum_ty, scrutinee_ptr, 1, "payload_ptr"
                ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let payload_ptr = self.builder.build_load(ptr, payload_ptr_ptr, "payload")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_pointer_value();

                // Bind payload fields.
                if arm.bindings.len() == 1 && arm.binding_tys.len() == 1 {
                    let b = &arm.bindings[0];
                    let bty = &arm.binding_tys[0];
                    if let Ok(field_ty) = self.tmap.basic(bty) {
                        let val = self.builder.build_load(field_ty, payload_ptr, &b.name)
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        let slot = self.builder.build_alloca(field_ty, &b.name)
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        self.builder.build_store(slot, val)
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        self.scopes.last_mut().unwrap().vars.insert(
                            b.name.clone(),
                            VarSlot { ptr: slot, ty: bty.clone(), mutability: Mut::Const, field_gep: None, is_closure: false },
                        );
                    }
                } else if arm.bindings.len() > 1 {
                    let field_tys: Vec<_> = arm.binding_tys.iter()
                        .map(|t| self.tmap.basic(t).unwrap_or(ptr.into()))
                        .collect();
                    let payload_struct_ty = self.ctx.struct_type(&field_tys, false);
                    for (i, (b, bty)) in arm.bindings.iter().zip(arm.binding_tys.iter()).enumerate() {
                        if let Ok(ft) = self.tmap.basic(bty) {
                            let fp = self.builder.build_struct_gep(
                                payload_struct_ty, payload_ptr, i as u32, &format!("pf_{i}")
                            ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            let val = self.builder.build_load(ft, fp, &b.name)
                                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            let slot = self.builder.build_alloca(ft, &b.name)
                                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            self.builder.build_store(slot, val)
                                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            self.scopes.last_mut().unwrap().vars.insert(
                                b.name.clone(),
                                VarSlot { ptr: slot, ty: bty.clone(), mutability: Mut::Const, field_gep: None, is_closure: false },
                            );
                        }
                    }
                }

                let mut arm_yield: Option<BasicValueEnum> = None;
                for stmt in &arm.body.stmts {
                    if let MonoStmtKind::Yield(e) = &stmt.kind { arm_yield = self.emit_expr(e)?; }
                    else { self.emit_stmt(stmt)?; }
                }
                self.emit_scope_release();
                self.scopes.pop();

                let arm_exit = self.builder.get_insert_block().unwrap();
                if arm_exit.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    if let Some(yv) = arm_yield { phi_parts.push((yv, arm_exit)); }
                }
            }
        } else {
            // Class hierarchy match — original unconditional fall-through logic.
            let first_arm_bb = self.ctx.append_basic_block(fv, "match.arm0");
            self.builder.build_unconditional_branch(first_arm_bb)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

            for (idx, arm) in m.arms.iter().enumerate() {
                self.builder.position_at_end(
                    if idx == 0 { first_arm_bb }
                    else { self.ctx.append_basic_block(fv, &format!("match.arm{idx}")) }
                );

                let body_bb = self.ctx.append_basic_block(fv, &format!("match.body{idx}"));
                self.builder.build_unconditional_branch(body_bb)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                self.builder.position_at_end(body_bb);
                self.scopes.push(Scope::new());

                if let Some(sv) = scrutinee {
                    if arm.bindings.len() == 1 && arm.binding_tys.len() == 1 {
                        let b = &arm.bindings[0];
                        let bty = &arm.binding_tys[0];
                        if let Ok(bind_ty) = self.tmap.basic(bty) {
                            let slot = self.builder.build_alloca(bind_ty, &b.name)
                                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            self.builder.build_store(slot, sv)
                                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                            self.scopes.last_mut().unwrap().vars.insert(
                                b.name.clone(),
                                VarSlot { ptr: slot, ty: bty.clone(), mutability: Mut::Const, field_gep: None, is_closure: false },
                            );
                        }
                    }
                }

                let mut arm_yield: Option<BasicValueEnum> = None;
                for stmt in &arm.body.stmts {
                    if let MonoStmtKind::Yield(e) = &stmt.kind { arm_yield = self.emit_expr(e)?; }
                    else { self.emit_stmt(stmt)?; }
                }
                self.emit_scope_release();
                self.scopes.pop();

                let arm_exit = self.builder.get_insert_block().unwrap();
                if arm_exit.get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    if let Some(yv) = arm_yield { phi_parts.push((yv, arm_exit)); }
                }
            }
        }

        self.builder.position_at_end(merge_bb);
        if !phi_parts.is_empty() {
            if let Ok(ll_ty) = self.tmap.basic(&m.ty) {
                let phi = self.builder.build_phi(ll_ty, "match.phi")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let incoming: Vec<(&dyn BasicValue, inkwell::basic_block::BasicBlock)> =
                    phi_parts.iter().map(|(v, b)| (v as &dyn BasicValue, *b)).collect();
                phi.add_incoming(&incoming);
                return Ok(Some(phi.as_basic_value()));
            }
        }
        Ok(None)
    }

    // ── Expressions ───────────────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        match &expr.kind {
            MonoExprKind::Int(n)    => Ok(Some(self.ctx.i64_type().const_int(*n as u64, true).into())),
            MonoExprKind::Float(f)  => Ok(Some(self.ctx.f64_type().const_float(*f).into())),
            MonoExprKind::Bool(b)   => Ok(Some(self.ctx.bool_type().const_int(*b as u64, false).into())),
            MonoExprKind::Null      => Ok(Some(self.tmap.ptr().const_null().into())),
            MonoExprKind::String(s) => {
                let g = self.builder.build_global_string_ptr(s, "str")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                Ok(Some(g.as_pointer_value().into()))
            }
            MonoExprKind::Var(name) => {
                // Unit enum variant: `Point` with no payload — emit a {tag, null} enum.
                if let Some((_, disc, _)) = self.find_variant(name) {
                    return self.emit_enum_variant(disc, &[]);
                }
                self.emit_load(name)
            }
            MonoExprKind::Unary(op, operand) => self.emit_unary(*op, operand),
            MonoExprKind::Binary(op, lhs, rhs) => self.emit_binary(*op, lhs, rhs, &expr.ty),
            MonoExprKind::Field(recv, fname)   => self.emit_field(recv, fname, &expr.ty),
            MonoExprKind::Call(name, args)     => self.emit_call(name, args),
            MonoExprKind::Construct(name, args)=> self.emit_construct(name, args),
            MonoExprKind::Index(recv, idx)     => self.emit_index(recv, idx),
            MonoExprKind::If(i)                => self.emit_if(i),
            MonoExprKind::Match(m)             => self.emit_match(m),
            MonoExprKind::Block(b)             => self.emit_block_expr(b),
            MonoExprKind::Array(elems)         => self.emit_array(elems, &expr.ty),
            MonoExprKind::Assign(t, v)         => self.emit_assign(t, v),
            MonoExprKind::Async(inner)         => self.emit_async(inner),
        }
    }

    fn emit_load(&mut self, name: &str) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // Check local scope first — a fn value stored in a variable.
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.vars.get(name) {
                let ll_ty = match self.tmap.basic(&slot.ty) {
                    Ok(t) => t,
                    Err(_) => return Ok(None),
                };
                let v = self.builder.build_load(ll_ty, slot.ptr, name)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(Some(v));
            }
        }

        // Fn literal — emit as fat pointer {fn_ptr, null_env} for uniform ABI.
        // Capturing closures: emit_closure_fat_ptr fills the env slot.
        // Non-capturing: null env — callee ignores it.
        if name.starts_with("__fn_lit_") {
            if let Some(mono_fn) = self.program.fns.iter().find(|f| f.name == name).cloned() {
                if !mono_fn.captures.is_empty() {
                    // Full closure — heap env struct.
                    return self.emit_closure_fat_ptr(&mono_fn);
                } else {
                    // Plain fn_lit — fat ptr with null env, heap-allocated for ABI uniformity.
                    return self.emit_plain_fn_fat_ptr(name);
                }
            }
        }

        // Plain function pointer (no captures) — return the fn ptr value.
        if let Some(&fv) = self.fns.get(name) {
            return Ok(Some(fv.as_global_value().as_pointer_value().into()));
        }
        Err(CodeGenError::UnknownVar(name.to_string()))
    }

    /// Emit a fat pointer `{ fn_ptr, null }` for a non-capturing fn_lit.
    /// The null env satisfies the uniform calling convention without overhead.
    fn emit_plain_fn_fat_ptr(&mut self, fn_name: &str) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let ptr = self.tmap.ptr();
        let i64 = self.ctx.i64_type();
        let fat_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);
        let malloc_fn = self.module.get_function("malloc")
            .ok_or_else(|| CodeGenError::UnknownFn("malloc".into()))?;
        let fat_sz = fat_ty.size_of()
            .ok_or_else(|| CodeGenError::BuildError("unsized fat ptr".into()))?;
        let sz64 = self.builder.build_int_z_extend(fat_sz, i64, "fat_sz")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let raw = self.builder.build_call(malloc_fn, &[sz64.into()], "fat_alloc")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap().into_pointer_value();

        let fn_ptr = self.fns.get(fn_name)
            .ok_or_else(|| CodeGenError::UnknownFn(fn_name.into()))?
            .as_global_value().as_pointer_value();

        let f0 = self.builder.build_struct_gep(fat_ty, raw, 0, "fn_slot")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f0, fn_ptr)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f1 = self.builder.build_struct_gep(fat_ty, raw, 1, "env_slot")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f1, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(Some(raw.into()))
    }

    /// Build a `{ fn_ptr, env_ptr }` fat pointer for a closure with captures.
    /// The env struct is heap-allocated; each captured variable is stored as a field.
    fn emit_closure_fat_ptr(&mut self, mono_fn: &MonoFn) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let ptr = self.tmap.ptr();
        let i64 = self.ctx.i64_type();
        let fat_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);

        // Allocate env struct: { cap0_ty, cap1_ty, ... }
        let mut cap_tys: Vec<inkwell::types::BasicTypeEnum> = Vec::new();
        for (_, ty, _) in &mono_fn.captures {
            cap_tys.push(self.tmap.basic(ty).unwrap_or(ptr.into()));
        }
        let env_struct_ty = self.ctx.struct_type(&cap_tys, false);
        let env_sz = env_struct_ty.size_of()
            .ok_or_else(|| CodeGenError::BuildError("unsized env".into()))?;
        let sz64 = self.builder.build_int_z_extend(env_sz, i64, "env_sz")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let malloc_fn = self.module.get_function("malloc")
            .ok_or_else(|| CodeGenError::UnknownFn("malloc".into()))?;
        let env_raw = self.builder.build_call(malloc_fn, &[sz64.into()], "env_alloc")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap();

        // Store each captured variable into the env struct.
        let fn_name = mono_fn.name.clone();
        let captures = mono_fn.captures.clone();
        for (i, (cap_name, cap_ty, _weak)) in captures.iter().enumerate() {
            let cap_val = self.emit_load(cap_name)?;
            if let Some(v) = cap_val {
                let fp = self.builder.build_struct_gep(
                    env_struct_ty, env_raw.into_pointer_value(), i as u32, &format!("cap_{i}")
                ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let _ = cap_ty;
                self.builder.build_store(fp, v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }

        // Build fat pointer { fn_ptr, env_ptr }.
        // Heap-allocate the fat pointer struct so it can be returned from functions.
        let fat_sz = fat_ty.size_of()
            .ok_or_else(|| CodeGenError::BuildError("unsized fat ptr".into()))?;
        let fat_sz64 = self.builder.build_int_z_extend(fat_sz, i64, "fat_sz")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let fat_raw = self.builder.build_call(malloc_fn, &[fat_sz64.into()], "fat_alloc")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap();
        let fat_alloc = fat_raw.into_pointer_value();
        let fn_ptr = self.fns.get(&fn_name)
            .ok_or_else(|| CodeGenError::UnknownFn(fn_name.clone()))?
            .as_global_value().as_pointer_value();
        let f0 = self.builder.build_struct_gep(fat_ty, fat_alloc, 0, "fn_slot")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f0, fn_ptr)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f1 = self.builder.build_struct_gep(fat_ty, fat_alloc, 1, "env_slot")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f1, env_raw)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(Some(fat_alloc.into()))
    }

    fn emit_unary(&mut self, op: UnaryOp, operand: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let v = self.emit_expr(operand)?.unwrap();
        Ok(Some(match op {
            UnaryOp::Neg => match v {
                BasicValueEnum::IntValue(i)   => self.builder.build_int_neg(i, "neg")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                BasicValueEnum::FloatValue(f) => self.builder.build_float_neg(f, "fneg")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("neg: wrong type".into())),
            },
            UnaryOp::Not => self.builder.build_not(v.into_int_value(), "not")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
        }))
    }

    fn emit_binary(
        &mut self, op: BinaryOp,
        lhs: &MonoExpr, rhs: &MonoExpr, ty: &SemTy,
    ) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if op == BinaryOp::Add && lhs.ty == SemTy::String {
            return self.emit_concat(lhs, rhs);
        }
        let lv = self.emit_expr(lhs)?.unwrap();
        let rv = self.emit_expr(rhs)?.unwrap();
        let _ = ty;
        let r: BasicValueEnum = match op {
            BinaryOp::Add => match (lv, rv) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    self.builder.build_int_add(l, r, "add").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    self.builder.build_float_add(l, r, "fadd").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("add".into())),
            },
            BinaryOp::Sub => match (lv, rv) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    self.builder.build_int_sub(l, r, "sub").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    self.builder.build_float_sub(l, r, "fsub").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("sub".into())),
            },
            BinaryOp::Mul => match (lv, rv) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    self.builder.build_int_mul(l, r, "mul").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    self.builder.build_float_mul(l, r, "fmul").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("mul".into())),
            },
            BinaryOp::Div => match (lv, rv) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    self.builder.build_int_signed_div(l, r, "sdiv").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                    self.builder.build_float_div(l, r, "fdiv").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("div".into())),
            },
            BinaryOp::Mod => match (lv, rv) {
                (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                    self.builder.build_int_signed_rem(l, r, "srem").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
                _ => return Err(CodeGenError::BuildError("mod".into())),
            },
            BinaryOp::Eq => self.cmp_eq(lv, rv, true,  &lhs.ty)?,
            BinaryOp::Ne => self.cmp_eq(lv, rv, false, &lhs.ty)?,
            BinaryOp::Lt => self.cmp_int_or_float(lv, rv, IntPredicate::SLT, FloatPredicate::OLT)?,
            BinaryOp::Le => self.cmp_int_or_float(lv, rv, IntPredicate::SLE, FloatPredicate::OLE)?,
            BinaryOp::Gt => self.cmp_int_or_float(lv, rv, IntPredicate::SGT, FloatPredicate::OGT)?,
            BinaryOp::Ge => self.cmp_int_or_float(lv, rv, IntPredicate::SGE, FloatPredicate::OGE)?,
            BinaryOp::And => self.builder.build_and(lv.into_int_value(), rv.into_int_value(), "and")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            BinaryOp::Or  => self.builder.build_or(lv.into_int_value(), rv.into_int_value(), "or")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
        };
        Ok(Some(r))
    }

    fn cmp_eq(&mut self, lv: BasicValueEnum<'ctx>, rv: BasicValueEnum<'ctx>, eq: bool, lhs_ty: &SemTy) -> CodeGenResult<BasicValueEnum<'ctx>> {
        // String equality must use strcmp, not pointer comparison.
        if *lhs_ty == SemTy::String {
            return self.emit_strcmp_eq(lv, rv, eq);
        }
        let (ip, fp) = if eq { (IntPredicate::EQ, FloatPredicate::OEQ) } else { (IntPredicate::NE, FloatPredicate::ONE) };
        Ok(match (lv, rv) {
            (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                self.builder.build_int_compare(ip, l, r, "icmp").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                self.builder.build_float_compare(fp, l, r, "fcmp").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            (BasicValueEnum::PointerValue(l), BasicValueEnum::PointerValue(r)) => {
                let i64 = self.ctx.i64_type();
                let li = self.builder.build_ptr_to_int(l, i64, "ptoi_l").map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let ri = self.builder.build_ptr_to_int(r, i64, "ptoi_r").map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                self.builder.build_int_compare(ip, li, ri, "pcmp").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into()
            }
            _ => return Err(CodeGenError::BuildError("eq/ne type mismatch".into())),
        })
    }

    /// Emit a string equality check using strcmp.
    fn emit_strcmp_eq(&mut self, lv: BasicValueEnum<'ctx>, rv: BasicValueEnum<'ctx>, eq: bool) -> CodeGenResult<BasicValueEnum<'ctx>> {
        // Declare strcmp if not already present.
        let strcmp_fn = self.module.get_function("strcmp").unwrap_or_else(|| {
            let ptr  = self.tmap.ptr();
            let i32  = self.ctx.i32_type();
            let ft   = i32.fn_type(&[ptr.into(), ptr.into()], false);
            self.module.add_function("strcmp", ft, None)
        });
        let result = self.builder.build_call(strcmp_fn, &[lv.into(), rv.into()], "strcmp")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let cmp_val = result.try_as_basic_value().left().unwrap().into_int_value();
        // strcmp returns 0 for equal. Convert to bool.
        let zero = self.ctx.i32_type().const_int(0, false);
        let pred = if eq { IntPredicate::EQ } else { IntPredicate::NE };
        Ok(self.builder.build_int_compare(pred, cmp_val, zero, "streq")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into())
    }

    fn cmp_int_or_float(
        &mut self, lv: BasicValueEnum<'ctx>, rv: BasicValueEnum<'ctx>,
        ip: IntPredicate, fp: FloatPredicate,
    ) -> CodeGenResult<BasicValueEnum<'ctx>> {
        Ok(match (lv, rv) {
            (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) =>
                self.builder.build_int_compare(ip, l, r, "icmp").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) =>
                self.builder.build_float_compare(fp, l, r, "fcmp").map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            _ => return Err(CodeGenError::BuildError("cmp type mismatch".into())),
        })
    }

    fn emit_concat(&mut self, lhs: &MonoExpr, rhs: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let f = self.module.get_function("haki_string_concat").unwrap_or_else(|| {
            let ptr = self.tmap.ptr();
            let ft  = ptr.fn_type(&[ptr.into(), ptr.into()], false);
            self.module.add_function("haki_string_concat", ft, None)
        });
        let lv = self.emit_expr(lhs)?.unwrap();
        let rv = self.emit_expr(rhs)?.unwrap();
        let r = self.builder.build_call(f, &[lv.into(), rv.into()], "concat")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    fn emit_field(&mut self, recv: &MonoExpr, fname: &str, _ty: &SemTy) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let recv_val = self.emit_expr(recv)?.unwrap().into_pointer_value();
        // Unwrap Optional — `e?.message` and `e.message` where e: Error? both give recv.ty = Optional(Error)
        let inner_ty = match &recv.ty {
            SemTy::Optional(inner) => inner.as_ref().clone(),
            other => other.clone(),
        };
        let type_name = match &inner_ty {
            SemTy::Named(n) | SemTy::Generic(n, _) => n.clone(),
            _ => return Ok(None),
        };

        // Special case: HttpRequest field access → C accessor functions
        if type_name == "HttpRequest" {
            let c_fn = match fname {
                "path"   => "haki_http_request_path",
                "method" => "haki_http_request_method",
                "body"   => "haki_http_request_body",
                _ => return Err(CodeGenError::BuildError(format!("no field {fname} on HttpRequest"))),
            };
            let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
            let r = self.builder.build_call(f, &[recv_val.into()], "req_field")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: HttpResponse field access → C accessor functions
        if type_name == "HttpResponse" {
            let c_fn = match fname {
                "status" => "haki_http_response_status",
                "body"   => "haki_http_response_body",
                _ => return Err(CodeGenError::BuildError(format!("no field {fname} on HttpResponse"))),
            };
            let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
            let r = self.builder.build_call(f, &[recv_val.into()], "resp_field")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: Error.message / Error.cause field → C accessors
        if type_name == "Error" {
            let c_fn = match fname {
                "message" => "haki_error_message",
                "cause"   => "haki_error_cause",
                _ => return Err(CodeGenError::BuildError(format!("no field {fname} on Error"))),
            };
            let f = *self.fns.get(c_fn)
                .ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
            let r = self.builder.build_call(f, &[recv_val.into()], "err_field")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: string.length → haki_string_length(ptr) → i64
        if type_name == "string" && fname == "length" {
            let len_fn = *self.fns.get("haki_string_length")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_string_length".into()))?;
            let r = self.builder.build_call(len_fn, &[recv_val.into()], "str_len")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: Array<T>.length → haki_array_length(ptr) → i64
        if type_name == "Array" && fname == "length" {
            let len_fn = *self.fns.get("haki_array_length")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_array_length".into()))?;
            let r = self.builder.build_call(len_fn, &[recv_val.into()], "arr_len")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: Map<K,V>.length → haki_map_length(ptr) → i64
        if type_name == "Map" && fname == "length" {
            let len_fn = *self.fns.get("haki_map_length")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_map_length".into()))?;
            let r = self.builder.build_call(len_fn, &[recv_val.into()], "map_len")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Special case: MutexGuard<T>.value — the C HakiMutexGuard struct
        // has layout { ptr mutex, ptr value }. Field index 1 = value pointer.
        if type_name == "MutexGuard" && fname == "value" {
            // The guard is a pointer to HakiMutexGuard { HakiMutex* mutex, void* value }.
            // Build an anonymous struct type matching the C layout.
            let ptr_ty = self.tmap.ptr();
            let guard_struct = self.ctx.struct_type(&[ptr_ty.into(), ptr_ty.into()], false);
            let value_field_ptr = self.builder.build_struct_gep(guard_struct, recv_val, 1, "guard_value_ptr")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            // Load the void* value pointer, then load the actual value from it.
            let void_ptr = self.builder.build_load(ptr_ty, value_field_ptr, "value_voidptr")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                .into_pointer_value();
            // The value type comes from the generic arg — for now treat as i64.
            let val_ty: inkwell::types::BasicTypeEnum = match _ty {
                SemTy::Int   => self.ctx.i64_type().into(),
                SemTy::Float => self.ctx.f64_type().into(),
                SemTy::Bool  => self.ctx.bool_type().into(),
                _            => self.ctx.i64_type().into(),
            };
            let val = self.builder.build_load(val_ty, void_ptr, "mutex_value")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(Some(val));
        }

        let pos = self.find_field_pos(&type_name, fname)?;
        let is_class = self.layout.is_class.get(&type_name).copied().unwrap_or(false);
        let idx = (pos + if is_class { 1 } else { 0 }) as u32;
        let struct_ty = self.layout.get(&type_name)
            .ok_or_else(|| CodeGenError::BuildError(format!("no layout for {type_name}")))?;
        let fp = self.builder.build_struct_gep(struct_ty, recv_val, idx, fname)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        if let Some(ty) = self.get_field_ty(&type_name, fname) {
            if let Ok(ll_ty) = self.tmap.basic(&ty) {
                let v = self.builder.build_load(ll_ty, fp, fname)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    fn emit_call(&mut self, name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // ── Enum variant construction ────────────────────────────────────────
        // `Ok(42)`, `Pending`, `Active(user)` — if name is a variant of any enum,
        // emit { tag, payload } instead of a function call.
        if let Some((_, discriminant, _)) = self.find_variant(name) {
            return self.emit_enum_variant(discriminant, args);
        }
        // `print(x)` dispatches to the right C function based on x's type.
        // This lets users write `print(42)` instead of `print_int(42)`.
        if name == "print" && args.len() == 1 {
            let arg = &args[0];
            let dispatch = match &arg.ty {
                SemTy::Int   => Some("haki_print_int"),
                SemTy::Float => Some("haki_print_float"),
                SemTy::Bool  => Some("haki_print_bool"),
                _            => None, // string, ptr, etc. → haki_print (existing)
            };
            if let Some(c_fn) = dispatch {
                let v = self.emit_expr(arg)?.unwrap();
                let f = *self.fns.get(c_fn)
                    .ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
                self.builder.build_call(f, &[v.into()], "print_any")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(None);
            }
        }
        if name.starts_with("Array__") {
            if name.ends_with("__append")     { return self.emit_array_append(args); }
            if name.ends_with("__removeLast") { return self.emit_array_remove_last(args); }
            if name.ends_with("__removeAt")   { return self.emit_array_remove_at(args); }
            if name.ends_with("__contains")   { return self.emit_array_contains(args); }
            if name.ends_with("__indexOf")    { return self.emit_array_index_of(args); }
            if name.ends_with("__first")      { return self.emit_array_endpoint("haki_array_first", args); }
            if name.ends_with("__last")       { return self.emit_array_endpoint("haki_array_last", args); }
            if name.ends_with("__join")       { return self.emit_array_join(args); }
        }

        // Intercept Map constructor and method calls
        if name == "Map" || name.starts_with("Map__") {
            if name == "Map" { return self.emit_map_new(); }
            if name.ends_with("__get")    { return self.emit_map_get(args); }
            if name.ends_with("__set")    { return self.emit_map_set(args); }
            if name.ends_with("__has")    { return self.emit_map_has(args); }
            if name.ends_with("__delete") { return self.emit_map_delete(args); }
            if name.ends_with("__getOrDefault") { return self.emit_map_get_or_default(args); }
        }

        // Intercept string method calls (string__split, string__trim, etc.)
        if name.starts_with("string__") {
            return self.emit_string_method(name, args);
        }

        // Intercept file I/O free functions
        if name == "readFile"   { return self.emit_read_file(args); }
        if name == "writeFile"  { return self.emit_write_file(args); }
        if name == "fileExists" { return self.emit_file_exists(args); }

        // argv() -> Array<string> — command-line arguments
        if name == "argv" {
            let arr_fn = self.module.get_function("haki_argv").unwrap_or_else(|| {
                let ptr = self.tmap.ptr();
                let ft  = ptr.fn_type(&[], false);
                self.module.add_function("haki_argv", ft, None)
            });
            let r = self.builder.build_call(arr_fn, &[], "argv_call")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Intercept HTTP server (v0.4)
        if name == "HttpServer" { return self.emit_http_server_new(args); }
        if name.starts_with("HttpServer__") && name.ends_with("__listen") {
            return self.emit_http_server_listen(args);
        }
        if name == "HttpResponse" { return self.emit_http_response_new(args); }

        // Intercept HttpRequest field access via method call pattern
        if name.starts_with("HttpRequest__") { return self.emit_http_request_field(name, args); }

        // Intercept Router (v0.4.5)
        if name == "Router" { return self.emit_router_new(); }
        if name.starts_with("Router__") { return self.emit_router_method(name, args); }
        if name == "requestParam"    { return self.emit_simple_call("haki_request_param",   args); }
        if name == "requestQuery"    { return self.emit_simple_call("haki_request_query",   args); }
        if name == "routerDispatch"  { return self.emit_simple_call("haki_router_dispatch", args); }

        // Phase 4: template + static files
        if name == "renderTemplate" { return self.emit_render_template(args); }
        if name == "serveFile"      { return self.emit_simple_call("haki_serve_file",  args); }

        // Phase 5: JSON decode
        if name == "jsonDecode"    { return self.emit_json_decode(args); }
        if name == "jsonDecodeGet" { return self.emit_simple_call("haki_json_decode_get", args); }

        // haki_ui — v0.5
        if name == "Spacer"  { return self.emit_simple_call("haki_spacer_new", &[]); }
        if name.starts_with("App__") && name.ends_with("__run") {
            return self.emit_app_run(args);
        }

        // Intercept JSON functions (v0.4)
        if name == "jsonString" { return self.emit_json_call("haki_json_string", args); }
        if name == "jsonInt"    { return self.emit_json_call("haki_json_int",    args); }
        if name == "jsonBool"   { return self.emit_json_call("haki_json_bool",   args); }

        // Intercept Thread method calls
        if name == "thread_spawn" { return self.emit_thread_spawn(args); }
        if name.starts_with("Thread__") && name.ends_with("__join") {
            return self.emit_thread_join(args);
        }

        // Intercept Task method calls
        if name.starts_with("Task__") && name.ends_with("__await") {
            return self.emit_task_await(args);
        }

        // Intercept Mutex constructor and method calls
        if name == "Mutex" { return self.emit_mutex_new(args); }
        if name.starts_with("Mutex__") && name.ends_with("__lock") {
            return self.emit_mutex_lock(args);
        }
        if name.starts_with("MutexGuard__") && name.ends_with("__unlock") {
            return self.emit_mutex_unlock(args);
        }

        // Direct call — look up by name in the function table.
        if let Some(&fv) = self.fns.get(name) {
            let arg_vals: Vec<BasicMetadataValueEnum> = args.iter().map(|a| {
                self.emit_expr(a).map(|v| v.map(|bv| bv.into())
                    .unwrap_or_else(|| self.tmap.ptr().const_null().into()))
            }).collect::<CodeGenResult<_>>()?;
            let r = self.builder.build_call(fv, &arg_vals, "call")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Indirect call — name is a local variable holding a function pointer or closure.
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.vars.get(name).cloned() {
                let (is_closure_slot, param_tys, ret_ty) = match &slot.ty {
                    SemTy::Fn(p, r)      => (slot.is_closure, p.clone(), *r.clone()),
                    SemTy::Closure(p, r) => (true,             p.clone(), *r.clone()),
                    _ => continue,
                };

                let ptr = self.tmap.ptr();

                // Closure fat pointer: slot.ptr is an alloca holding the fat ptr address.
                // Load the fat ptr first, then GEP into it.
                let (fn_ptr_val, env_arg): (inkwell::values::PointerValue, BasicValueEnum) =
                if is_closure_slot {
                    let fat_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);
                    // Load the fat ptr from the alloca slot.
                    let fat_ptr = self.builder.build_load(ptr, slot.ptr, "fat_ptr")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_pointer_value();
                    let fn_field = self.builder.build_struct_gep(fat_ty, fat_ptr, 0, "clo_fn")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    let fp = self.builder.build_load(ptr, fn_field, "fn_ptr_val")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into_pointer_value();
                    let env_field = self.builder.build_struct_gep(fat_ty, fat_ptr, 1, "clo_env")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    let ep = self.builder.build_load(ptr, env_field, "env_ptr_val")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    (fp, ep)
                } else {
                    // Plain fn ptr slot — load the function pointer.
                    let fp = self.builder.build_load(ptr, slot.ptr, "fn_ptr")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                        .into_pointer_value();
                    // Uniform convention: pass null env for non-closures.
                    (fp, ptr.const_null().into())
                };

                // Build LLVM function type — env ptr is always first param.
                let mut ll_params: Vec<inkwell::types::BasicMetadataTypeEnum> = vec![ptr.into()];
                ll_params.extend(param_tys.iter()
                    .filter_map(|p| self.tmap.basic(p).ok()
                        .map(inkwell::types::BasicMetadataTypeEnum::from)));
                let ll_ret = self.tmap.return_type(&ret_ty)?;
                let fn_ty = match ll_ret {
                    Some(r) => r.fn_type(&ll_params, false),
                    None    => self.ctx.void_type().fn_type(&ll_params, false),
                };

                // Arg list — env always first.
                let mut arg_vals: Vec<BasicMetadataValueEnum> = vec![env_arg.into()];
                for a in args {
                    let v = self.emit_expr(a)?.map(|bv| bv.into())
                        .unwrap_or_else(|| self.tmap.ptr().const_null().into());
                    arg_vals.push(v);
                }

                let r = self.builder.build_indirect_call(fn_ty, fn_ptr_val, &arg_vals, "indirect_call")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            }
        }

        Err(CodeGenError::UnknownFn(name.to_string()))
    }

    /// Emit Array.append(item)
    fn emit_array_append(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(None); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let item_val = self.emit_expr(&args[1])?;
        let append_fn = *self.fns.get("haki_array_append")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_append".into()))?;
        if let Some(item) = item_val {
            let item_slot = self.builder.build_alloca(item.get_type(), "append_item")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(item_slot, item)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_call(append_fn, &[arr_ptr.into(), item_slot.into()], "append")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        }
        Ok(None)
    }

    fn emit_array_remove_last(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let ptr = self.tmap.ptr();
        let i64 = self.ctx.i64_type();
        let tuple_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);

        let f = *self.fns.get("haki_array_remove_last")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_remove_last".into()))?;
        let raw = self.builder.build_call(f, &[arr_ptr.into()], "rm_last")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap_or(ptr.const_null().into());

        // Determine element type from the array's generic arg, load it.
        let elem_ty = match &args[0].ty {
            SemTy::Generic(_, targs) if !targs.is_empty() => {
                self.tmap.basic(&targs[0]).unwrap_or(ptr.into())
            }
            _ => ptr.into(),
        };
        let elem_val: BasicValueEnum = if raw.into_pointer_value().is_null() {
            match elem_ty {
                inkwell::types::BasicTypeEnum::IntType(t)   => t.const_int(0, false).into(),
                inkwell::types::BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
                _                                           => ptr.const_null().into(),
            }
        } else {
            self.builder.build_load(elem_ty, raw.into_pointer_value(), "elem")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?
        };

        // Null error pointer
        let tuple = self.builder.build_alloca(tuple_ty, "rm_tuple")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let sz = i64.const_int(0, false); let _ = sz;
        let f0 = self.builder.build_struct_gep(tuple_ty, tuple, 0, "t0")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        // Store elem as ptr (int2ptr if needed)
        let as_ptr = match elem_val {
            BasicValueEnum::IntValue(i) => self.builder
                .build_int_to_ptr(i, ptr, "e2p")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into(),
            BasicValueEnum::PointerValue(_) => elem_val,
            _ => ptr.const_null().into(),
        };
        self.builder.build_store(f0, as_ptr)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f1 = self.builder.build_struct_gep(tuple_ty, tuple, 1, "t1")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f1, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(Some(tuple.into()))
    }

    fn emit_array_remove_at(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // args: [array, index]
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let idx_val = self.emit_expr(&args[1])?.unwrap();
        let f = *self.fns.get("haki_array_remove_at")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_remove_at".into()))?;
        let r = self.builder.build_call(f, &[arr_ptr.into(), idx_val.into()], "rm_at")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        // Same tuple pattern as removeLast — return (T?, Error?) struct
        let ptr = self.tmap.ptr();
        let tuple_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);
        let raw = r.try_as_basic_value().left().unwrap_or(ptr.const_null().into());
        let tuple = self.builder.build_alloca(tuple_ty, "rmat_tuple")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f0 = self.builder.build_struct_gep(tuple_ty, tuple, 0, "t0")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f0, raw)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f1 = self.builder.build_struct_gep(tuple_ty, tuple, 1, "t1")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(f1, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(Some(tuple.into()))
    }

    fn emit_array_contains(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // args: [array, item]
        if args.len() < 2 { return Ok(Some(self.ctx.bool_type().const_int(0, false).into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let item = self.emit_expr(&args[1])?.unwrap();
        // Dispatch on item type
        let (c_fn, item_arg): (&str, BasicValueEnum) = match item {
            BasicValueEnum::IntValue(_)     => ("haki_array_contains_int", item),
            BasicValueEnum::PointerValue(_) => ("haki_array_contains_str", item),
            _                               => ("haki_array_contains_int", item),
        };
        let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
        let r = self.builder.build_call(f, &[arr_ptr.into(), item_arg.into()], "arr_has")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        // i64 → i1 bool
        let i64_v = r.try_as_basic_value().left().unwrap().into_int_value();
        let as_bool = self.builder.build_int_truncate(i64_v, self.ctx.bool_type(), "has_bool")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(Some(as_bool.into()))
    }

    fn emit_array_index_of(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.ctx.i64_type().const_int(u64::MAX, true).into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let item = self.emit_expr(&args[1])?.unwrap();
        let (c_fn, item_arg): (&str, BasicValueEnum) = match item {
            BasicValueEnum::IntValue(_)     => ("haki_array_index_of_int", item),
            BasicValueEnum::PointerValue(_) => ("haki_array_index_of_str", item),
            _                               => ("haki_array_index_of_int", item),
        };
        let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
        let r = self.builder.build_call(f, &[arr_ptr.into(), item_arg.into()], "arr_idx")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    fn emit_array_endpoint(&mut self, c_fn: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
        let r = self.builder.build_call(f, &[arr_ptr.into()], "arr_ep")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    fn emit_array_join(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let arr_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let sep_val = self.emit_expr(&args[1])?.unwrap();
        let f = *self.fns.get("haki_array_join")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_join".into()))?;
        let r = self.builder.build_call(f, &[arr_ptr.into(), sep_val.into()], "arr_join")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    // ── Enum variant construction ─────────────────────────────────────────

    /// Find which enum (if any) contains a variant named `name`.
    /// Returns (enum_name, discriminant_index, variant).
    fn find_variant(&self, name: &str) -> Option<(String, usize, haki_ast::EnumVariant)> {
        for (enum_name, enum_def) in &self.program.enum_defs {
            if let Some((idx, variant)) = enum_def.variants.iter().enumerate()
                .find(|(_, v)| {
                    v.name.name == name ||
                    // Handle bare name matching alias__Name variant
                    v.name.name.split("__").last().map_or(false, |bare| bare == name)
                })
            {
                return Some((enum_name.clone(), idx, variant.clone()));
            }
        }
        None
    }

    /// Like find_variant but restricts search to a specific enum type name.
    /// Handles both the exact name and the alias__Name convention.
    fn find_variant_in(&self, variant_name: &str, enum_type_name: &str) -> Option<(String, usize, haki_ast::EnumVariant)> {
        // Try exact enum name first, then stripped alias prefix.
        let try_names: Vec<&str> = vec![
            enum_type_name,
        ];
        for ename in try_names {
            if let Some(enum_def) = self.program.enum_defs.get(ename) {
                if let Some((idx, variant)) = enum_def.variants.iter().enumerate()
                    .find(|(_, v)| {
                        v.name.name == variant_name ||
                        v.name.name.split("__").last().map_or(false, |bare| bare == variant_name)
                    })
                {
                    return Some((ename.to_string(), idx, variant.clone()));
                }
            }
        }
        // Fall back to searching all enums.
        self.find_variant(variant_name)
    }

    /// Emit `VariantName(args...)` → `{ i64 tag, ptr payload }` heap struct.
    /// Unit variants: payload = null.
    /// Payload variants: malloc a struct of the field values.
    fn emit_enum_variant(
        &mut self,
        discriminant: usize,
        args: &[MonoExpr],
    ) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let i64 = self.ctx.i64_type();
        let ptr = self.tmap.ptr();
        let enum_ty = self.ctx.struct_type(&[i64.into(), ptr.into()], false);

        // Allocate the enum wrapper on the heap.
        let malloc_fn = self.module.get_function("malloc")
            .ok_or_else(|| CodeGenError::UnknownFn("malloc".into()))?;
        let enum_sz = enum_ty.size_of()
            .ok_or_else(|| CodeGenError::BuildError("unsized enum".into()))?;
        let sz64 = self.builder.build_int_z_extend(enum_sz, i64, "enum_sz")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let raw = self.builder.build_call(malloc_fn, &[sz64.into()], "enum_alloc")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap();

        // Store discriminant.
        let tag_ptr = self.builder.build_struct_gep(
            enum_ty, raw.into_pointer_value(), 0, "tag_ptr"
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(tag_ptr, i64.const_int(discriminant as u64, false))
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Build payload.
        let payload_ptr_slot = self.builder.build_struct_gep(
            enum_ty, raw.into_pointer_value(), 1, "payload_slot"
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        if args.is_empty() {
            // Unit variant — null payload.
            self.builder.build_store(payload_ptr_slot, ptr.const_null())
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        } else {
            // Payload: malloc a struct { T0, T1, ... } and fill fields.
            let mut field_tys: Vec<inkwell::types::BasicTypeEnum> = Vec::new();
            let mut field_vals: Vec<BasicValueEnum> = Vec::new();
            for arg in args {
                if let Some(v) = self.emit_expr(arg)? {
                    let ty = self.tmap.basic(&arg.ty).unwrap_or(ptr.into());
                    field_tys.push(ty);
                    field_vals.push(v);
                }
            }
            let payload_ty = self.ctx.struct_type(&field_tys, false);
            let payload_sz = payload_ty.size_of()
                .ok_or_else(|| CodeGenError::BuildError("unsized payload".into()))?;
            let psz64 = self.builder.build_int_z_extend(payload_sz, i64, "payload_sz")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let payload_raw = self.builder.build_call(malloc_fn, &[psz64.into()], "payload_alloc")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                .try_as_basic_value().left().unwrap();
            for (i, v) in field_vals.iter().enumerate() {
                let fp = self.builder.build_struct_gep(
                    payload_ty, payload_raw.into_pointer_value(), i as u32, &format!("pf_{i}")
                ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                self.builder.build_store(fp, *v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
            self.builder.build_store(payload_ptr_slot, payload_raw)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        }

        Ok(Some(raw))
    }

    /// Emit Map() constructor → haki_map_new(8) → ptr
    fn emit_map_new(&mut self) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let map_new = *self.fns.get("haki_map_new")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_new".into()))?;
        let val_size = self.ctx.i64_type().const_int(8, false);
        let r = self.builder.build_call(map_new, &[val_size.into()], "map_new")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit Map.getOrDefault(key, default) → V (never null)
    fn emit_map_get_or_default(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 3 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let map_ptr     = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let key_val     = self.emit_expr(&args[1])?.unwrap();
        let default_val = self.emit_expr(&args[2])?;

        let god_fn = *self.fns.get("haki_map_get_or_default")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_get_or_default".into()))?;

        // Store default into a slot so we can pass a pointer.
        let val_sem = match &args[0].ty {
            SemTy::Generic(_, type_args) if type_args.len() >= 2 => type_args[1].clone(),
            _ => SemTy::String,
        };
        let val_ll = self.tmap.basic(&val_sem)?;

        let default_ptr = if let Some(dv) = default_val {
            let slot = self.builder.build_alloca(dv.get_type(), "god_default")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(slot, dv)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            slot.into()
        } else {
            self.tmap.ptr().const_null().into()
        };

        let result = self.builder.build_call(
            god_fn, &[map_ptr.into(), key_val.into(), default_ptr], "god"
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Load the actual value from the returned pointer.
        let result_ptr = result.try_as_basic_value().left().unwrap().into_pointer_value();
        let val = self.builder.build_load(val_ll, result_ptr, "god_val")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(Some(val))
    }

    /// Emit Map.get(key) → V? (returns pointer to value or null)
    fn emit_map_get(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let map_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let key_val = self.emit_expr(&args[1])?.unwrap();
        let get_fn = *self.fns.get("haki_map_get")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_get".into()))?;
        // haki_map_get returns a ptr to the value, or null.
        // For v0.1 we return the ptr directly — caller dereferences if non-null.
        let r = self.builder.build_call(get_fn, &[map_ptr.into(), key_val.into()], "map_get")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Load the actual value from the pointer if it's non-null.
        // Determine value type from the Map's second generic arg.
        let val_sem = match &args[0].ty {
            SemTy::Generic(_, type_args) if type_args.len() >= 2 => type_args[1].clone(),
            _ => return Ok(r.try_as_basic_value().left()),
        };
        if let Ok(val_ll) = self.tmap.basic(&val_sem) {
            let raw_ptr = r.try_as_basic_value().left().unwrap().into_pointer_value();
            // Only load if not null — return null (Optional) for missing keys.
            // v0.1 simplified: just return the raw pointer (Optional<V> as ptr).
            let _ = val_ll;
            return Ok(Some(raw_ptr.into()));
        }
        Ok(r.try_as_basic_value().left())
    }

    /// Emit Map.set(key, value)
    fn emit_map_set(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 3 { return Ok(None); }
        let map_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let key_val = self.emit_expr(&args[1])?.unwrap();
        let val_val = self.emit_expr(&args[2])?;
        let set_fn = *self.fns.get("haki_map_set")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_set".into()))?;
        if let Some(v) = val_val {
            // Store value in a temp slot to pass by pointer.
            let val_slot = self.builder.build_alloca(v.get_type(), "map_val")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_store(val_slot, v)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            self.builder.build_call(set_fn, &[map_ptr.into(), key_val.into(), val_slot.into()], "map_set")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        }
        Ok(None)
    }

    /// Emit Map.has(key) → bool
    fn emit_map_has(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.ctx.bool_type().const_int(0, false).into())); }
        let map_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let key_val = self.emit_expr(&args[1])?.unwrap();
        let has_fn = *self.fns.get("haki_map_has")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_has".into()))?;
        let r = self.builder.build_call(has_fn, &[map_ptr.into(), key_val.into()], "map_has")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit Map.delete(key)
    fn emit_map_delete(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(None); }
        let map_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let key_val = self.emit_expr(&args[1])?.unwrap();
        let del_fn = *self.fns.get("haki_map_delete")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_map_delete".into()))?;
        self.builder.build_call(del_fn, &[map_ptr.into(), key_val.into()], "map_del")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    /// Emit thread_spawn(fn) → calls haki_thread_spawn(fn_ptr) → Thread ptr.
    /// The argument is a Haki function reference — passed as a function pointer.
    fn emit_thread_spawn(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let spawn_fn = *self.fns.get("haki_thread_spawn")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_thread_spawn".into()))?;
        // The argument should be a function reference — load it as a pointer.
        let fn_ptr = match &args[0].kind {
            MonoExprKind::Var(name) => {
                // Look up the function value and get its pointer.
                if let Some(&fv) = self.fns.get(name.as_str()) {
                    fv.as_global_value().as_pointer_value().into()
                } else {
                    self.emit_expr(&args[0])?.unwrap()
                }
            }
            _ => self.emit_expr(&args[0])?.unwrap(),
        };
        let r = self.builder.build_call(spawn_fn, &[fn_ptr.into()], "thread_spawn")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit thread.join() → calls haki_thread_join(thread_ptr).
    fn emit_thread_join(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(None); }
        let thread_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let join_fn = *self.fns.get("haki_thread_join")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_thread_join".into()))?;
        self.builder.build_call(join_fn, &[thread_ptr.into()], "thread_join")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    /// Emit `async expr` — wraps a call in a Task dispatched to the thread pool.
    ///
    /// Strategy for v0.2:
    ///   1. The inner expression must be a call to a known function.
    ///   2. We generate a thunk: `void* __thunk_NAME(void* arg) { return (void*)NAME(); }`
    ///   3. Call `haki_task_spawn(thunk_ptr, null)` → returns HakiTask*.
    ///
    /// For calls with arguments, the args are captured in a heap-allocated
    /// struct and passed as the `arg` void* pointer to the thunk.
    /// For zero-argument functions (the common case in v0.2), arg is null.
    fn emit_async(&mut self, inner: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let spawn_fn = *self.fns.get("haki_task_spawn")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_task_spawn".into()))?;

        // Extract the callee name from the inner expression.
        let (callee_name, call_args) = match &inner.kind {
            MonoExprKind::Call(name, args) => (name.clone(), args.clone()),
            _ => {
                // Non-call async — not supported in v0.2, return null Task.
                return Ok(Some(self.tmap.ptr().const_null().into()));
            }
        };

        // Get or create a thunk for this function.
        let thunk_name = format!("__haki_thunk_{callee_name}");
        let target_fn = *self.fns.get(&callee_name)
            .ok_or_else(|| CodeGenError::UnknownFn(callee_name.clone()))?;

        // Create the thunk function if it doesn't exist yet.
        let ptr = self.tmap.ptr();
        let thunk_fn = if let Some(existing) = self.module.get_function(&thunk_name) {
            existing
        } else {
            // `void* thunk(void* arg)` — C-compatible calling convention
            let thunk_ty = ptr.fn_type(&[ptr.into()], false);
            let thunk = self.module.add_function(&thunk_name, thunk_ty, None);
            self.fns.insert(thunk_name.clone(), thunk);

            // Build the thunk body.
            let saved_fn   = self.current_fn;
            let saved_bb   = self.builder.get_insert_block();
            let saved_scopes = std::mem::take(&mut self.scopes);

            self.current_fn = Some(thunk);
            let entry = self.ctx.append_basic_block(thunk, "entry");
            self.builder.position_at_end(entry);

            // For zero-arg functions: just call and return result cast to ptr.
            if call_args.is_empty() {
                let call_result = self.builder.build_call(target_fn, &[], "thunk_call")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                let ret_val: BasicValueEnum = match call_result.try_as_basic_value().left() {
                    Some(v) => {
                        // Cast the return value to void* for the thunk return.
                        match v {
                            BasicValueEnum::IntValue(i) => {
                                self.builder.build_int_to_ptr(i, ptr, "ret_ptr")
                                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into()
                            }
                            BasicValueEnum::PointerValue(p) => p.into(),
                            _ => ptr.const_null().into(),
                        }
                    }
                    None => ptr.const_null().into(),
                };
                self.builder.build_return(Some(&ret_val))
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            } else {
                // Args present — heap-allocate a capture struct { arg0, arg1, ... },
                // pass it to the thunk as void*, unpack inside.
                let arg_thunk_param = thunk.get_nth_param(0).unwrap().into_pointer_value();
                let i64 = self.ctx.i64_type();

                // Build struct type from arg types in call_args.
                let mut arg_ll_tys: Vec<inkwell::types::BasicTypeEnum> = Vec::new();
                for a in &call_args {
                    match self.tmap.basic(&a.ty) {
                        Ok(t) => arg_ll_tys.push(t),
                        Err(_) => arg_ll_tys.push(ptr.into()),
                    }
                }
                let capture_ty = self.ctx.struct_type(&arg_ll_tys, false);

                // Unpack each captured arg from the struct pointer.
                let mut unpacked: Vec<BasicValueEnum> = Vec::new();
                for (i, _arg) in call_args.iter().enumerate() {
                    let field_ptr = self.builder.build_struct_gep(
                        capture_ty, arg_thunk_param, i as u32, &format!("cap_{i}")
                    ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    let val = self.builder.build_load(
                        arg_ll_tys[i], field_ptr, &format!("arg_{i}")
                    ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    unpacked.push(val);
                }

                // Call the target function with unpacked args.
                let meta_args: Vec<_> = unpacked.iter()
                    .map(|v| inkwell::values::BasicMetadataValueEnum::from(*v))
                    .collect();
                let call_result = self.builder.build_call(target_fn, &meta_args, "thunk_call")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let ret_val: BasicValueEnum = match call_result.try_as_basic_value().left() {
                    Some(BasicValueEnum::IntValue(i)) => {
                        self.builder.build_int_to_ptr(i, ptr, "ret_ptr")
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?.into()
                    }
                    Some(BasicValueEnum::PointerValue(p)) => p.into(),
                    _ => ptr.const_null().into(),
                };
                self.builder.build_return(Some(&ret_val))
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }

            // Restore builder state.
            self.current_fn = saved_fn;
            self.scopes = saved_scopes;
            if let Some(bb) = saved_bb {
                self.builder.position_at_end(bb);
            }
            thunk
        };

        // Spawn the task — pass args via capture struct when present.
        let thunk_ptr = thunk_fn.as_global_value().as_pointer_value();
        let arg_ptr: inkwell::values::BasicValueEnum = if call_args.is_empty() {
            ptr.const_null().into()
        } else {
            // Build struct type from arg types.
            let mut arg_ll_tys: Vec<inkwell::types::BasicTypeEnum> = Vec::new();
            for a in &call_args {
                match self.tmap.basic(&a.ty) {
                    Ok(t) => arg_ll_tys.push(t),
                    Err(_) => arg_ll_tys.push(ptr.into()),
                }
            }
            let capture_ty = self.ctx.struct_type(&arg_ll_tys, false);
            let sz = capture_ty.size_of()
                .ok_or_else(|| CodeGenError::BuildError("unsized async capture".into()))?;
            let malloc_fn = self.module.get_function("malloc")
                .ok_or_else(|| CodeGenError::UnknownFn("malloc".into()))?;
            let i64 = self.ctx.i64_type();
            let sz64 = self.builder.build_int_z_extend(sz, i64, "cap_sz")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let raw = self.builder.build_call(malloc_fn, &[sz64.into()], "cap_alloc")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                .try_as_basic_value().left().unwrap();
            // Store each arg into the capture struct.
            for (i, arg_expr) in call_args.iter().enumerate() {
                if let Some(v) = self.emit_expr(arg_expr)? {
                    let fp = self.builder.build_struct_gep(
                        capture_ty, raw.into_pointer_value(), i as u32, &format!("cs_{i}")
                    ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    self.builder.build_store(fp, v)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }
            }
            raw
        };

        let task_call = self.builder.build_call(
            spawn_fn,
            &[thunk_ptr.into(), arg_ptr.into()],
            "task_spawn",
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(task_call.try_as_basic_value().left())
    }

    /// Emit task.await() → haki_task_await(task_ptr) → void*
    /// Then load the actual return value from the result pointer.
    fn emit_task_await(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let task_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let await_fn = *self.fns.get("haki_task_await")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_task_await".into()))?;
        let r = self.builder.build_call(await_fn, &[task_ptr.into()], "task_await")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        // Result is a void* — return it as an opaque pointer.
        // The caller's type annotation determines how it's actually used.
        Ok(r.try_as_basic_value().left())
    }

    /// Emit Mutex(initial_value) → haki_mutex_new(value_ptr) → HakiMutex*
    fn emit_mutex_new(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let new_fn = *self.fns.get("haki_mutex_new")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_mutex_new".into()))?;
        // Initial value — store it on the heap and pass the pointer.
        let val_ptr = if let Some(first) = args.first() {
            if let Some(v) = self.emit_expr(first)? {
                // Allocate a slot and store the value.
                let slot = self.builder.build_alloca(v.get_type(), "mutex_val")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                self.builder.build_store(slot, v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                slot.into()
            } else {
                self.tmap.ptr().const_null().into()
            }
        } else {
            self.tmap.ptr().const_null().into()
        };
        let r = self.builder.build_call(new_fn, &[val_ptr], "mutex_new")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit mutex.lock() → haki_mutex_lock(mutex_ptr) → HakiMutexGuard*
    fn emit_mutex_lock(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let mutex_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let lock_fn = *self.fns.get("haki_mutex_lock")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_mutex_lock".into()))?;
        let r = self.builder.build_call(lock_fn, &[mutex_ptr.into()], "mutex_lock")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit guard.unlock() → haki_mutex_unlock(guard_ptr)
    fn emit_mutex_unlock(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(None); }
        let guard_ptr = self.emit_expr(&args[0])?.unwrap().into_pointer_value();
        let unlock_fn = *self.fns.get("haki_mutex_unlock")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_mutex_unlock".into()))?;
        self.builder.build_call(unlock_fn, &[guard_ptr.into()], "mutex_unlock")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    /// Dispatch string method calls to C runtime functions.
    /// args[0] is always `self` (the string pointer).
    fn emit_string_method(&mut self, name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // Map mangled method name → C function name
        let c_name = if name.ends_with("__split")      { "haki_string_split" }
            else if name.ends_with("__trim")            { "haki_string_trim" }
            else if name.ends_with("__trimStart")       { "haki_string_trim_start" }
            else if name.ends_with("__trimEnd")         { "haki_string_trim_end" }
            else if name.ends_with("__toUpper")         { "haki_string_to_upper" }
            else if name.ends_with("__toLower")         { "haki_string_to_lower" }
            else if name.ends_with("__contains")        { "haki_string_contains" }
            else if name.ends_with("__startsWith")      { "haki_string_starts_with" }
            else if name.ends_with("__endsWith")        { "haki_string_ends_with" }
            else if name.ends_with("__replace")         { "haki_string_replace" }
            else if name.ends_with("__indexOf")         { "haki_string_index_of" }
            else if name.ends_with("__substring")       { "haki_string_substring" }
            else if name.ends_with("__length")          {
                // string.length — call haki_string_length (already declared)
                return if args.is_empty() {
                    Ok(Some(self.ctx.i64_type().const_int(0, false).into()))
                } else {
                    let s = self.emit_expr(&args[0])?.unwrap();
                    let f = *self.fns.get("haki_string_length")
                        .ok_or_else(|| CodeGenError::UnknownFn("haki_string_length".into()))?;
                    let r = self.builder.build_call(f, &[s.into()], "str_len")
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                    Ok(r.try_as_basic_value().left())
                };
            }
            else {
                return Err(CodeGenError::UnknownFn(name.to_string()));
            };

        let fn_val = *self.fns.get(c_name)
            .ok_or_else(|| CodeGenError::UnknownFn(c_name.to_string()))?;

        let arg_vals: Vec<BasicMetadataValueEnum> = args.iter()
            .map(|a| self.emit_expr(a).map(|v|
                v.map(|bv| bv.into())
                 .unwrap_or_else(|| self.tmap.ptr().const_null().into())))
            .collect::<CodeGenResult<_>>()?;

        let r = self.builder.build_call(fn_val, &arg_vals, "str_method")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit readFile(path) → (content: string, err: Error?)
    /// Uses the C function: haki_file_read(path, &content_out, &error_out)
    /// Returns a pointer to a stack tuple { ptr content, ptr error }.
    fn emit_read_file(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let path_val = self.emit_expr(&args[0])?.unwrap();
        let read_fn = *self.fns.get("haki_file_read")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_file_read".into()))?;
        let ptr_ty = self.tmap.ptr();
        let ptr_ptr_ty = self.ctx.struct_type(&[ptr_ty.into(), ptr_ty.into()], false);

        // Allocate output slots.
        let content_slot = self.builder.build_alloca(ptr_ty, "rf_content")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let error_slot   = self.builder.build_alloca(ptr_ty, "rf_error")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Call haki_file_read(path, &content, &error).
        self.builder.build_call(read_fn, &[path_val.into(), content_slot.into(), error_slot.into()], "file_read")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Pack into a stack tuple { ptr content, ptr error } for the caller.
        // Wrap the raw char* error string in a HakiError struct so callers
        // can use .message, .cause etc. Uses haki_error_new which is null-safe.
        let tuple = self.builder.build_alloca(ptr_ptr_ty, "rf_tuple")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let content_val = self.builder.build_load(ptr_ty, content_slot, "rf_c")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let raw_error   = self.builder.build_load(ptr_ty, error_slot, "rf_e_raw")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Call haki_error_wrap(raw_char_ptr) → HakiError* | null
        let wrap_fn = *self.fns.get("haki_error_wrap")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_error_wrap".into()))?;
        let wrapped_err = self.builder.build_call(wrap_fn, &[raw_error.into()], "rf_err")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?
            .try_as_basic_value().left().unwrap();

        let c_ptr = self.builder.build_struct_gep(ptr_ptr_ty, tuple, 0, "t0")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(c_ptr, content_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let e_ptr = self.builder.build_struct_gep(ptr_ptr_ty, tuple, 1, "t1")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(e_ptr, wrapped_err)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(Some(tuple.into()))
    }

    /// Emit writeFile(path, content) → Error? (ptr to error string or null)
    fn emit_write_file(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let path    = self.emit_expr(&args[0])?.unwrap();
        let content = self.emit_expr(&args[1])?.unwrap();
        let write_fn = *self.fns.get("haki_file_write")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_file_write".into()))?;
        let r = self.builder.build_call(write_fn, &[path.into(), content.into()], "file_write")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit fileExists(path) → bool
    fn emit_file_exists(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.ctx.bool_type().const_int(0, false).into())); }
        let path = self.emit_expr(&args[0])?.unwrap();
        let exists_fn = *self.fns.get("haki_file_exists")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_file_exists".into()))?;
        let r = self.builder.build_call(exists_fn, &[path.into()], "file_exists")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    // ── HTTP Server (v0.4) ────────────────────────────────────────────────

    /// Emit HttpServer(port, handler) → haki_http_server_new(port, handler_ptr)
    fn emit_http_server_new(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let port = self.emit_expr(&args[0])?.unwrap();

        // If the handler arg is a Router, generate a dispatch thunk.
        let handler_ptr: BasicValueEnum = if args[1].ty == SemTy::Named("Router".into()) {
            let router_val = self.emit_expr(&args[1])?.unwrap();
            self.emit_router_thunk(router_val)?
        } else {
            match &args[1].kind {
                MonoExprKind::Var(name) => {
                    if let Some(&fv) = self.fns.get(name.as_str()) {
                        fv.as_global_value().as_pointer_value().into()
                    } else {
                        self.emit_expr(&args[1])?.unwrap()
                    }
                }
                _ => self.emit_expr(&args[1])?.unwrap(),
            }
        };
        let new_fn = *self.fns.get("haki_http_server_new")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_http_server_new".into()))?;
        let r = self.builder.build_call(new_fn, &[port.into(), handler_ptr.into()], "http_new")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Generate a thunk function that captures a Router pointer in a global
    /// and dispatches requests through it. Returns the thunk function pointer.
    fn emit_router_thunk(&mut self, router_val: BasicValueEnum<'ctx>) -> CodeGenResult<BasicValueEnum<'ctx>> {
        let ptr_ty = self.tmap.ptr();

        // Store the router in a module-level global so the thunk can access it.
        let global_name = "__haki_router_global";
        let global = self.module.get_global(global_name).unwrap_or_else(|| {
            let g = self.module.add_global(ptr_ty, None, global_name);
            g.set_initializer(&ptr_ty.const_null());
            g
        });
        self.builder.build_store(global.as_pointer_value(), router_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Generate the thunk once per module.
        let thunk_name = "__haki_router_thunk";
        let thunk_fn = if let Some(existing) = self.module.get_function(thunk_name) {
            existing
        } else {
            let thunk_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
            let thunk = self.module.add_function(thunk_name, thunk_ty, None);
            self.fns.insert(thunk_name.to_string(), thunk);

            let saved_fn     = self.current_fn;
            let saved_bb     = self.builder.get_insert_block();
            let saved_scopes = std::mem::take(&mut self.scopes);

            self.current_fn = Some(thunk);
            let entry = self.ctx.append_basic_block(thunk, "entry");
            self.builder.position_at_end(entry);

            let g = self.module.get_global(global_name).unwrap();
            let router_ptr = self.builder.build_load(ptr_ty, g.as_pointer_value(), "router")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let req_param = thunk.get_nth_param(0).unwrap();

            let dispatch_fn = *self.fns.get("haki_router_dispatch")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_router_dispatch".into()))?;
            let result = self.builder.build_call(
                dispatch_fn, &[router_ptr.into(), req_param.into()], "dispatch"
            ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            let ret_val: BasicValueEnum = result.try_as_basic_value().left()
                .unwrap_or(ptr_ty.const_null().into());
            self.builder.build_return(Some(&ret_val))
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

            self.current_fn = saved_fn;
            self.scopes = saved_scopes;
            if let Some(bb) = saved_bb { self.builder.position_at_end(bb); }
            thunk
        };
        Ok(thunk_fn.as_global_value().as_pointer_value().into())
    }

    /// Emit server.listen() → haki_http_server_listen(server_ptr)
    fn emit_http_server_listen(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(None); }
        let server = self.emit_expr(&args[0])?.unwrap();
        let listen_fn = *self.fns.get("haki_http_server_listen")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_http_server_listen".into()))?;
        self.builder.build_call(listen_fn, &[server.into()], "http_listen")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    /// Emit HttpResponse(status, body) → haki_http_response_new(status, body)
    fn emit_http_response_new(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let status = self.emit_expr(&args[0])?.unwrap();
        let body   = self.emit_expr(&args[1])?.unwrap();
        let resp_fn = *self.fns.get("haki_http_response_new")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_http_response_new".into()))?;
        let r = self.builder.build_call(resp_fn, &[status.into(), body.into()], "http_resp")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit HttpRequest field access via mangled method name
    fn emit_http_request_field(&mut self, name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let req_ptr = self.emit_expr(&args[0])?.unwrap();
        let c_fn = if name.ends_with("__path")   { "haki_http_request_path" }
            else if name.ends_with("__method")    { "haki_http_request_method" }
            else if name.ends_with("__body")      { "haki_http_request_body" }
            else { return Err(CodeGenError::UnknownFn(name.to_string())); };
        let f = *self.fns.get(c_fn).ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
        let r = self.builder.build_call(f, &[req_ptr.into()], "req_field")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit JSON free functions (jsonString, jsonInt, jsonBool)
    fn emit_json_call(&mut self, c_name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let f = *self.fns.get(c_name).ok_or_else(|| CodeGenError::UnknownFn(c_name.into()))?;
        let arg_vals: Vec<BasicMetadataValueEnum> = args.iter()
            .map(|a| self.emit_expr(a).map(|v| v.map(|bv| bv.into())
                .unwrap_or_else(|| self.tmap.ptr().const_null().into())))
            .collect::<CodeGenResult<_>>()?;
        let r = self.builder.build_call(f, &arg_vals, "json")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Generic helper — call a C function with all args emitted directly.
    fn emit_simple_call(&mut self, c_name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let f = *self.fns.get(c_name).ok_or_else(|| CodeGenError::UnknownFn(c_name.into()))?;
        let arg_vals: Vec<BasicMetadataValueEnum> = args.iter()
            .map(|a| self.emit_expr(a).map(|v| v.map(|bv| bv.into())
                .unwrap_or_else(|| self.tmap.ptr().const_null().into())))
            .collect::<CodeGenResult<_>>()?;
        let r = self.builder.build_call(f, &arg_vals, c_name)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    // ── Router (v0.4.5) ───────────────────────────────────────────────────

    /// Emit Router() → haki_router_new()
    fn emit_router_new(&mut self) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let f = *self.fns.get("haki_router_new")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_router_new".into()))?;
        let r = self.builder.build_call(f, &[], "router_new")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit router.get/post/put/delete/handle method calls.
    fn emit_router_method(&mut self, name: &str, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // router.handle(req) → haki_router_dispatch(router, req)
        if name.ends_with("__handle") {
            if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
            let router = self.emit_expr(&args[0])?.unwrap();
            let req    = self.emit_expr(&args[1])?.unwrap();
            let f = *self.fns.get("haki_router_dispatch")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_router_dispatch".into()))?;
            let r = self.builder.build_call(f, &[router.into(), req.into()], "dispatch")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // router.get/post/put/delete(pattern, handler) → haki_router_add(router, method, pattern, handler_ptr)
        let method_str = if name.ends_with("__get")    { "GET" }
            else if name.ends_with("__post")   { "POST" }
            else if name.ends_with("__put")    { "PUT" }
            else if name.ends_with("__delete") { "DELETE" }
            else { return Err(CodeGenError::UnknownFn(name.into())); };

        if args.len() < 3 { return Ok(None); }
        let router      = self.emit_expr(&args[0])?.unwrap();
        let pattern_val = self.emit_expr(&args[1])?.unwrap();

        // Handler: extract function pointer from Var or evaluate expression.
        let handler_ptr: BasicValueEnum = match &args[2].kind {
            MonoExprKind::Var(fn_name) => {
                // Try direct function lookup first (named function).
                if let Some(&fv) = self.fns.get(fn_name.as_str()) {
                    fv.as_global_value().as_pointer_value().into()
                } else {
                    // It's a local variable holding a fn pointer — load it.
                    self.emit_expr(&args[2])?.unwrap()
                }
            }
            _ => self.emit_expr(&args[2])?.unwrap(),
        };

        let method_global = self.builder.build_global_string_ptr(method_str, "http_method")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let method_ptr: BasicValueEnum = method_global.as_pointer_value().into();

        let add_fn = *self.fns.get("haki_router_add")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_router_add".into()))?;
        self.builder.build_call(add_fn, &[router.into(), method_ptr.into(), pattern_val.into(), handler_ptr.into()], "route_add")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    // ── Phase 4: Template rendering + static files ────────────────────────

    /// Emit renderTemplate(path, data) → (string, Error?)
    /// Matches the same two-ptr tuple pattern as readFile so the
    /// `const result, err = renderTemplate(...)` binding works correctly.
    fn emit_render_template(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.len() < 2 { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let path = self.emit_expr(&args[0])?.unwrap();
        let data = self.emit_expr(&args[1])?.unwrap();
        let ptr  = self.tmap.ptr();
        let tuple_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);

        let out_result = self.builder.build_alloca(ptr, "tmpl_result")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let out_error  = self.builder.build_alloca(ptr, "tmpl_error")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(out_result, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(out_error, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        let f = *self.fns.get("haki_render_template_map")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_render_template_map".into()))?;
        self.builder.build_call(f, &[path.into(), data.into(), out_result.into(), out_error.into()], "render")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Load values and pack into a { ptr, ptr } stack tuple — same pattern as readFile.
        let result_val = self.builder.build_load(ptr, out_result, "tmpl_str")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let error_val  = self.builder.build_load(ptr, out_error,  "tmpl_err")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        let tuple = self.builder.build_alloca(tuple_ty, "tmpl_tuple")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let r0 = self.builder.build_struct_gep(tuple_ty, tuple, 0, "t0")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(r0, result_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let r1 = self.builder.build_struct_gep(tuple_ty, tuple, 1, "t1")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(r1, error_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(Some(tuple.into()))
    }

    // ── Phase 5: JSON decode ─────────────────────────────────────────────

    /// Emit jsonDecode(s) → (Map<string,string>, Error?)
    /// Returns a packed { ptr, ptr } tuple matching the readFile pattern.
    fn emit_json_decode(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(Some(self.tmap.ptr().const_null().into())); }
        let s   = self.emit_expr(&args[0])?.unwrap();
        let ptr = self.tmap.ptr();
        let tuple_ty = self.ctx.struct_type(&[ptr.into(), ptr.into()], false);

        let out_map   = self.builder.build_alloca(ptr, "json_map")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let out_error = self.builder.build_alloca(ptr, "json_err")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(out_map,   ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(out_error, ptr.const_null())
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        let f = *self.fns.get("haki_json_decode")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_json_decode".into()))?;
        self.builder.build_call(f, &[s.into(), out_map.into(), out_error.into()], "jdecode")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        let map_val = self.builder.build_load(ptr, out_map,   "jmap")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let err_val = self.builder.build_load(ptr, out_error, "jerr")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        let tuple = self.builder.build_alloca(tuple_ty, "json_tuple")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let r0 = self.builder.build_struct_gep(tuple_ty, tuple, 0, "t0")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(r0, map_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let r1 = self.builder.build_struct_gep(tuple_ty, tuple, 1, "t1")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        self.builder.build_store(r1, err_val)
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(Some(tuple.into()))
    }

    // ── haki_ui v0.5 emitters ────────────────────────────────────────────

    /// Emit VStack/HStack — extract children from Array<View> literal and
    /// pass as a C array of GtkWidget* to haki_vstack_new / haki_hstack_new.
    fn emit_stack_construct(&mut self, type_name: &str, args: &[MonoNamedArg]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let c_fn = if type_name == "VStack" { "haki_vstack_new" } else { "haki_hstack_new" };
        let children_arg = args.iter().find(|a| a.name == "children");
        let ptr = self.tmap.ptr();
        let i64 = self.ctx.i64_type();

        // Try to extract children from an array literal in the mono AST.
        // If the children arg is an Array expression, emit each element
        // and pack them into a stack-allocated GtkWidget*[] array.
        if let Some(arg) = children_arg {
            if let MonoExprKind::Array(elems) = &arg.value.kind.clone() {
                let count = elems.len() as u64;
                // Allocate a C array of pointers on the stack.
                let arr_ty = ptr.array_type(count as u32);
                let arr_slot = self.builder.build_alloca(arr_ty, "child_arr")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                for (i, elem) in elems.iter().enumerate() {
                    let child_val = self.emit_expr(elem)?.unwrap_or(ptr.const_null().into());
                    let gep = unsafe {
                        self.builder.build_in_bounds_gep(
                            ptr,
                            arr_slot,
                            &[i64.const_int(i as u64, false)],
                            &format!("child_{i}"),
                        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?
                    };
                    self.builder.build_store(gep, child_val)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }

                let f = *self.fns.get(c_fn)
                    .ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
                let count_val = i64.const_int(count, false);
                let r = self.builder.build_call(f, &[arr_slot.into(), count_val.into()], "stack_new")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            }
        }

        // Fallback: empty stack
        let arr_slot = self.builder.build_alloca(ptr.array_type(0), "empty_arr")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let f = *self.fns.get(c_fn)
            .ok_or_else(|| CodeGenError::UnknownFn(c_fn.into()))?;
        let r = self.builder.build_call(f, &[arr_slot.into(), i64.const_int(0, false).into()], "stack_new")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(r.try_as_basic_value().left())
    }

    /// Emit app.run() → haki_app_run(title, root, body_fn)
    /// Reads all three from the module globals set at App construction time.
    fn emit_app_run(&mut self, args: &[MonoExpr]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        if args.is_empty() { return Ok(None); }
        let ptr = self.tmap.ptr();

        // Load the three globals written by emit_construct for App.
        let load_global = |this: &mut Self, name: &str| -> CodeGenResult<BasicValueEnum<'ctx>> {
            let g = this.module.get_global(name).unwrap_or_else(|| {
                let g = this.module.add_global(ptr, None, name);
                g.set_initializer(&ptr.const_null()); g
            });
            this.builder.build_load(ptr, g.as_pointer_value(), name)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))
        };

        let title_val = load_global(self, "__haki_app_title")?;
        let root_val  = load_global(self, "__haki_app_root")?;
        let body_val  = load_global(self, "__haki_app_body")?;

        let f = *self.fns.get("haki_app_run")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_app_run".into()))?;
        self.builder.build_call(f, &[title_val.into(), root_val.into(), body_val.into()], "app_run")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(None)
    }

    fn emit_construct(&mut self, type_name: &str, args: &[MonoNamedArg]) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        // ── haki_ui v0.5 widgets ─────────────────────────────────────────────

        // Spacer() — no args
        if type_name == "Spacer" {
            return self.emit_simple_call("haki_spacer_new", &[]);
        }

        // Text(content: string)
        if type_name == "Text" {
            let content = args.iter().find(|a| a.name == "content")
                .map(|a| self.emit_expr(&a.value))
                .transpose()?.flatten()
                .unwrap_or(self.tmap.ptr().const_null().into());
            let f = *self.fns.get("haki_text_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_text_new".into()))?;
            let r = self.builder.build_call(f, &[content.into()], "text_new")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // Button(label: string, onTap: fn() -> void)
        if type_name == "Button" {
            let label = args.iter().find(|a| a.name == "label")
                .map(|a| self.emit_expr(&a.value)).transpose()?.flatten()
                .unwrap_or(self.tmap.ptr().const_null().into());
            let on_tap: BasicValueEnum = if let Some(tap_arg) = args.iter().find(|a| a.name == "onTap") {
                match &tap_arg.value.kind {
                    MonoExprKind::Var(fn_name) => {
                        if let Some(&fv) = self.fns.get(fn_name.as_str()) {
                            fv.as_global_value().as_pointer_value().into()
                        } else {
                            self.emit_expr(&tap_arg.value)?.unwrap()
                        }
                    }
                    _ => self.emit_expr(&tap_arg.value)?.unwrap(),
                }
            } else {
                self.tmap.ptr().const_null().into()
            };
            let f = *self.fns.get("haki_button_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_button_new".into()))?;
            let r = self.builder.build_call(f, &[label.into(), on_tap.into()], "btn_new")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // TextField(value: string, onChange: fn(string) -> void)
        if type_name == "TextField" {
            let value = args.iter().find(|a| a.name == "value")
                .map(|a| self.emit_expr(&a.value)).transpose()?.flatten()
                .unwrap_or(self.tmap.ptr().const_null().into());
            let on_change: BasicValueEnum = if let Some(arg) = args.iter().find(|a| a.name == "onChange") {
                match &arg.value.kind {
                    MonoExprKind::Var(fn_name) => {
                        if let Some(&fv) = self.fns.get(fn_name.as_str()) {
                            fv.as_global_value().as_pointer_value().into()
                        } else { self.emit_expr(&arg.value)?.unwrap() }
                    }
                    _ => self.emit_expr(&arg.value)?.unwrap(),
                }
            } else { self.tmap.ptr().const_null().into() };
            let f = *self.fns.get("haki_textfield_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_textfield_new".into()))?;
            let r = self.builder.build_call(f, &[value.into(), on_change.into()], "tf_new")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // VStack(children: Array<View>) / HStack(children: Array<View>)
        if type_name == "VStack" || type_name == "HStack" {
            return self.emit_stack_construct(type_name, args);
        }

        // App(title: string, render: fn() -> View)
        if type_name == "App" {
            // App(title, root) — store title, root instance, and TypeName__body fn ptr.
            let ptr = self.tmap.ptr();
            let title_global = self.module.get_global("__haki_app_title")
                .unwrap_or_else(|| {
                    let g = self.module.add_global(ptr, None, "__haki_app_title");
                    g.set_initializer(&ptr.const_null()); g
                });
            let root_global = self.module.get_global("__haki_app_root")
                .unwrap_or_else(|| {
                    let g = self.module.add_global(ptr, None, "__haki_app_root");
                    g.set_initializer(&ptr.const_null()); g
                });
            let body_global = self.module.get_global("__haki_app_body")
                .unwrap_or_else(|| {
                    let g = self.module.add_global(ptr, None, "__haki_app_body");
                    g.set_initializer(&ptr.const_null()); g
                });

            if let Some(a) = args.iter().find(|a| a.name == "title") {
                let v = self.emit_expr(&a.value)?.unwrap();
                self.builder.build_store(title_global.as_pointer_value(), v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }

            if let Some(a) = args.iter().find(|a| a.name == "root") {
                // Emit the root instance and store it.
                let root_val = self.emit_expr(&a.value)?.unwrap();
                self.builder.build_store(root_global.as_pointer_value(), root_val)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                // Resolve TypeName__body from the concrete type on the expression.
                let concrete_name = match &a.value.ty {
                    SemTy::Named(n) => n.clone(),
                    _ => String::new(),
                };
                let body_fn_name = format!("{concrete_name}__body");
                if let Some(&body_fv) = self.fns.get(body_fn_name.as_str()) {
                    let body_ptr: BasicValueEnum =
                        body_fv.as_global_value().as_pointer_value().into();
                    self.builder.build_store(body_global.as_pointer_value(), body_ptr)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }
            }

            return Ok(Some(title_global.as_pointer_value().into()));
        }

        // Router() → haki_router_new()
        if type_name == "Router" {
            return self.emit_router_new();
        }

        // HttpServer(port: N, handler: fn | Router)
        if type_name == "HttpServer" {
            let port_arg = args.iter().find(|a| a.name == "port")
                .ok_or_else(|| CodeGenError::BuildError("HttpServer: missing port".into()))?;
            let handler_arg = args.iter().find(|a| a.name == "handler")
                .ok_or_else(|| CodeGenError::BuildError("HttpServer: missing handler".into()))?;

            let port_val = self.emit_expr(&port_arg.value)?.unwrap();

            // Determine handler: if it's a Router, generate a dispatch thunk
            // that captures the router in a global variable.
            let handler_ptr: BasicValueEnum = match &handler_arg.value.ty {
                SemTy::Named(n) if n == "Router" => {
                    // Emit the router value.
                    let router_val = self.emit_expr(&handler_arg.value)?.unwrap();

                    // Store router in a global so the thunk can access it.
                    let ptr_ty = self.tmap.ptr();
                    let global = self.module.add_global(ptr_ty, None, "__haki_router_global");
                    global.set_initializer(&ptr_ty.const_null());
                    self.builder.build_store(global.as_pointer_value(), router_val)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                    // Generate a thunk: void* __router_thunk(HakiHttpRequest* req)
                    let thunk_name = "__haki_router_thunk";
                    let thunk_fn = if let Some(existing) = self.module.get_function(thunk_name) {
                        existing
                    } else {
                        let thunk_ty = ptr_ty.fn_type(&[ptr_ty.into()], false);
                        let thunk = self.module.add_function(thunk_name, thunk_ty, None);

                        let saved_fn     = self.current_fn;
                        let saved_bb     = self.builder.get_insert_block();
                        let saved_scopes = std::mem::take(&mut self.scopes);

                        self.current_fn = Some(thunk);
                        let entry = self.ctx.append_basic_block(thunk, "entry");
                        self.builder.position_at_end(entry);

                        // Load router from global.
                        let router_ptr = self.builder.build_load(ptr_ty, global.as_pointer_value(), "router")
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                        // Get req param (first arg).
                        let req_param = thunk.get_nth_param(0).unwrap();

                        // Call haki_router_dispatch(router, req).
                        let dispatch_fn = *self.fns.get("haki_router_dispatch")
                            .ok_or_else(|| CodeGenError::UnknownFn("haki_router_dispatch".into()))?;
                        let result = self.builder.build_call(dispatch_fn, &[router_ptr.into(), req_param.into()], "dispatch")
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        let ret_val: BasicValueEnum = result.try_as_basic_value().left()
                            .unwrap_or(ptr_ty.const_null().into());
                        self.builder.build_return(Some(&ret_val))
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

                        self.current_fn = saved_fn;
                        self.scopes = saved_scopes;
                        if let Some(bb) = saved_bb { self.builder.position_at_end(bb); }
                        thunk
                    };
                    thunk_fn.as_global_value().as_pointer_value().into()
                }
                _ => {
                    // Regular function handler.
                    match &handler_arg.value.kind {
                        MonoExprKind::Var(name) => {
                            if let Some(&fv) = self.fns.get(name.as_str()) {
                                fv.as_global_value().as_pointer_value().into()
                            } else {
                                self.emit_expr(&handler_arg.value)?.unwrap()
                            }
                        }
                        _ => self.emit_expr(&handler_arg.value)?.unwrap(),
                    }
                }
            };

            let new_fn = *self.fns.get("haki_http_server_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_http_server_new".into()))?;
            let r = self.builder.build_call(new_fn, &[port_val.into(), handler_ptr.into()], "http_new")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }

        // HttpResponse(status: N, body: "...", contentType?: "...", location?: "...")
        if type_name == "HttpResponse" {
            let status_arg = args.iter().find(|a| a.name == "status")
                .ok_or_else(|| CodeGenError::BuildError("HttpResponse: missing status".into()))?;
            let status = self.emit_expr(&status_arg.value)?.unwrap();

            // Redirect shorthand: HttpResponse(status: 302, location: "/login")
            if let Some(loc_arg) = args.iter().find(|a| a.name == "location") {
                let location = self.emit_expr(&loc_arg.value)?.unwrap();
                // Build a redirect response with Location header body (MHD handles it via body)
                // For now: return plain response with location in body + 302
                // Phase 3 full header support deferred — produce a redirect body string
                let loc_str = format!("Redirecting...");
                let body_global = self.builder.build_global_string_ptr(&loc_str, "redirect_body")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                let ct_global = self.builder.build_global_string_ptr("text/html", "redirect_ct")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                // Use location value in Content-Location header via typed response
                let _ = location; // stored for future full header support
                let resp_fn = *self.fns.get("haki_http_response_typed")
                    .ok_or_else(|| CodeGenError::UnknownFn("haki_http_response_typed".into()))?;
                let r = self.builder.build_call(resp_fn, &[
                    status.into(),
                    body_global.as_pointer_value().into(),
                    ct_global.as_pointer_value().into(),
                ], "http_redirect")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            }

            let body_arg = args.iter().find(|a| a.name == "body")
                .ok_or_else(|| CodeGenError::BuildError("HttpResponse: missing body".into()))?;
            let body = self.emit_expr(&body_arg.value)?.unwrap();

            // Optional contentType field
            if let Some(ct_arg) = args.iter().find(|a| a.name == "contentType") {
                let ct = self.emit_expr(&ct_arg.value)?.unwrap();
                let resp_fn = *self.fns.get("haki_http_response_typed")
                    .ok_or_else(|| CodeGenError::UnknownFn("haki_http_response_typed".into()))?;
                let r = self.builder.build_call(resp_fn, &[status.into(), body.into(), ct.into()], "http_resp_typed")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            }

            let resp_fn = *self.fns.get("haki_http_response_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_http_response_new".into()))?;
            let r = self.builder.build_call(resp_fn, &[status.into(), body.into()], "http_resp")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }
        // Error(message: string) or Error(message: string, cause: Error?)
        if type_name == "Error" {
            let msg = if let Some(a) = args.iter().find(|a| a.name == "message") {
                self.emit_expr(&a.value)?.unwrap_or(self.tmap.ptr().const_null().into())
            } else {
                self.tmap.ptr().const_null().into()
            };
            let cause_arg = args.iter().find(|a| a.name == "cause");
            if let Some(cause) = cause_arg {
                let cause_val = self.emit_expr(&cause.value)?
                    .unwrap_or(self.tmap.ptr().const_null().into());
                let f = *self.fns.get("haki_error_new_with_cause")
                    .ok_or_else(|| CodeGenError::UnknownFn("haki_error_new_with_cause".into()))?;
                let r = self.builder.build_call(f, &[msg.into(), cause_val.into()], "error_new")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            } else {
                let f = *self.fns.get("haki_error_new")
                    .ok_or_else(|| CodeGenError::UnknownFn("haki_error_new".into()))?;
                let r = self.builder.build_call(f, &[msg.into()], "error_new")
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                return Ok(r.try_as_basic_value().left());
            }
        }

        // Map<K,V>() constructor → haki_map_new(val_size)
        // val_size for v0.1 is always 8 (pointer-sized or i64).
        if type_name == "Map" || type_name.starts_with("Map__") {            let map_new = *self.fns.get("haki_map_new")
                .ok_or_else(|| CodeGenError::UnknownFn("haki_map_new".into()))?;
            let val_size = self.ctx.i64_type().const_int(8, false);
            let r = self.builder.build_call(map_new, &[val_size.into()], "map_new")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            return Ok(r.try_as_basic_value().left());
        }
        let is_class = self.layout.is_class.get(type_name).copied().unwrap_or(false);
        let Some(llstruct) = self.layout.get(type_name) else {
            return Ok(Some(self.tmap.ptr().const_null().into()));
        };

        // Both structs and classes are heap-allocated when constructed.
        // - Classes: use haki_alloc (ARC header + user data, refcount = 1)
        // - Structs: use malloc (no ARC header, just raw data)
        //   Using alloca would produce a dangling pointer if the struct escapes
        //   the current stack frame (e.g. returned from a constructor function).
        let obj_ptr: PointerValue = if is_class {
            let sz = llstruct.size_of()
                .ok_or_else(|| CodeGenError::BuildError(format!("size_of {type_name}")))?;
            let r = self.builder.build_call(self.arc.alloc, &[sz.into()], "alloc")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            r.try_as_basic_value().left().unwrap().into_pointer_value()
        } else {
            // Declare malloc if not already present.
            let malloc_fn = self.module.get_function("malloc").unwrap_or_else(|| {
                let ptr  = self.tmap.ptr();
                let i64  = self.ctx.i64_type();
                let ft   = ptr.fn_type(&[i64.into()], false);
                self.module.add_function("malloc", ft, None)
            });
            let sz = llstruct.size_of()
                .ok_or_else(|| CodeGenError::BuildError(format!("size_of {type_name}")))?;
            let r = self.builder.build_call(malloc_fn, &[sz.into()], "struct_alloc")
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            r.try_as_basic_value().left().unwrap().into_pointer_value()
        };

        for arg in args {
            let pos = self.find_field_pos(type_name, &arg.name).unwrap_or(0);
            let idx = (pos + if is_class { 1 } else { 0 }) as u32;
            let struct_ty2 = self.layout.get(type_name)
                .ok_or_else(|| CodeGenError::BuildError(format!("no layout for {type_name}")))?;
            let fp = self.builder.build_struct_gep(struct_ty2, obj_ptr, idx, &arg.name)
                .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            if let Some(v) = self.emit_expr(&arg.value)? {
                self.builder.build_store(fp, v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }
        Ok(Some(obj_ptr.into()))
    }

    fn emit_index(&mut self, recv: &MonoExpr, idx: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let arr_ptr = self.emit_expr(recv)?.unwrap().into_pointer_value();
        let idx_val = self.emit_expr(idx)?.unwrap().into_int_value();

        // Determine element type from receiver's generic argument.
        let elem_sem = match &recv.ty {
            SemTy::Generic(_, args) if !args.is_empty() => args[0].clone(),
            _ => SemTy::Int,
        };
        let elem_ll = self.tmap.basic(&elem_sem)?;

        // Call haki_array_get(ptr arr, i64 idx) → ptr (pointer to element)
        let get_fn = *self.fns.get("haki_array_get")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_get".into()))?;
        let elem_ptr_result = self.builder.build_call(
            get_fn,
            &[arr_ptr.into(), idx_val.into()],
            "arr_get",
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        let elem_ptr = elem_ptr_result.try_as_basic_value()
            .left().unwrap().into_pointer_value();

        // Load the element value from the returned pointer.
        let val = self.builder.build_load(elem_ll, elem_ptr, "arr_elem")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
        Ok(Some(val))
    }

    fn emit_block_expr(&mut self, block: &MonoBlock) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        self.scopes.push(Scope::new());
        let mut yv: Option<BasicValueEnum> = None;
        for stmt in &block.stmts {
            if let MonoStmtKind::Yield(e) = &stmt.kind { yv = self.emit_expr(e)?; }
            else { self.emit_stmt(stmt)?; }
        }
        self.emit_scope_release();
        self.scopes.pop();
        Ok(yv)
    }

    fn emit_array(&mut self, elems: &[MonoExpr], ty: &SemTy) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let i64_ty  = self.ctx.i64_type();
        let elem_sem = match ty {
            SemTy::Generic(_, args) if !args.is_empty() => {
                // Empty array literal infers Array<void>. Treat as Array<int>
                // since the actual element type comes from the binding annotation.
                if args[0] == SemTy::Void { SemTy::Int } else { args[0].clone() }
            }
            _ => SemTy::Int,
        };
        let elem_ll = self.tmap.basic(&elem_sem)?;

        // Get the sizeof the element type. For LLVM, use target data or
        // a compile-time constant for well-known types.
        let elem_size: u64 = match &elem_sem {
            SemTy::Bool  => 1,
            SemTy::Int   => 8,
            SemTy::Float => 8,
            _            => 8, // pointers / everything else is ptr-sized
        };
        let elem_size_val = i64_ty.const_int(elem_size, false);
        let count_val     = i64_ty.const_int(elems.len() as u64, false);

        // Stack-allocate a temporary data buffer to hold the literal elements.
        // This is just used for the haki_array_from call — the C function
        // copies the data onto the heap immediately.
        let temp_buf = self.builder.build_array_alloca(elem_ll, count_val, "arr_literal")
            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        // Store each element into the temp buffer.
        for (i, elem) in elems.iter().enumerate() {
            if let Some(v) = self.emit_expr(elem)? {
                let idx = i64_ty.const_int(i as u64, false);
                let slot = unsafe {
                    self.builder.build_gep(elem_ll, temp_buf, &[idx], &format!("lit_{i}"))
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?
                };
                self.builder.build_store(slot, v)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
        }

        // Call haki_array_from(ptr src, i64 count, i64 elem_size) → ptr
        let from_fn = *self.fns.get("haki_array_from")
            .ok_or_else(|| CodeGenError::UnknownFn("haki_array_from".into()))?;
        let result = self.builder.build_call(
            from_fn,
            &[temp_buf.into(), count_val.into(), elem_size_val.into()],
            "arr",
        ).map_err(|e| CodeGenError::BuildError(e.to_string()))?;

        Ok(result.try_as_basic_value().left())
    }

    fn emit_assign(&mut self, target: &MonoExpr, value: &MonoExpr) -> CodeGenResult<Option<BasicValueEnum<'ctx>>> {
        let new_val = self.emit_expr(value)?.unwrap();
        match &target.kind {
            MonoExprKind::Var(name) => {
                let (slot_ptr, arc, ty, field_gep) = {
                    let mut found = None;
                    for scope in self.scopes.iter().rev() {
                        if let Some(v) = scope.vars.get(name.as_str()) {
                            found = Some((v.ptr, TypeMap::is_arc_type(&v.ty, &self.layout), v.ty.clone(), v.field_gep));
                            break;
                        }
                    }
                    found.ok_or_else(|| CodeGenError::UnknownVar(name.clone()))?
                };
                if arc {
                    if let Ok(ll_ty) = self.tmap.basic(&ty) {
                        let old = self.builder.build_load(ll_ty, slot_ptr, "old")
                            .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                        let _ = self.builder.build_call(self.arc.release, &[old.into()], "rel");
                    }
                    let _ = self.builder.build_call(self.arc.retain, &[new_val.into()], "ret");
                }
                // Write to the local slot.
                self.builder.build_store(slot_ptr, new_val)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                // If this local backs a struct field, also write back through the GEP
                // so the mutation persists on the object (not just the local copy).
                if let Some(gep_ptr) = field_gep {
                    self.builder.build_store(gep_ptr, new_val)
                        .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                }
            }
            MonoExprKind::Field(recv_expr, fname) => {
                let recv_val = self.emit_expr(recv_expr)?.unwrap().into_pointer_value();
                let type_name = match &recv_expr.ty {
                    SemTy::Named(n) | SemTy::Generic(n, _) => n.clone(),
                    _ => return Ok(None),
                };
                let pos = self.find_field_pos(&type_name, fname)?;
                let is_class = self.layout.is_class.get(&type_name).copied().unwrap_or(false);
                let idx = (pos + if is_class { 1 } else { 0 }) as u32;
                let struct_ty3 = self.layout.get(&type_name)
                    .ok_or_else(|| CodeGenError::BuildError(format!("no layout for {type_name}")))?;
                let fp = self.builder.build_struct_gep(struct_ty3, recv_val, idx, fname)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
                self.builder.build_store(fp, new_val)
                    .map_err(|e| CodeGenError::BuildError(e.to_string()))?;
            }
            _ => {}
        }
        Ok(None)
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn find_field_pos(&self, type_name: &str, fname: &str) -> CodeGenResult<usize> {
        for s in &self.program.structs {
            if s.name == type_name {
                if let Some(p) = s.fields.iter().position(|f| f.name == fname) { return Ok(p); }
            }
        }
        for c in &self.program.classes {
            if c.name == type_name {
                if let Some(p) = c.fields.iter().position(|f| f.name == fname) { return Ok(p); }
            }
        }
        Err(CodeGenError::BuildError(format!("no field {fname} on {type_name}")))
    }

    fn get_field_ty(&self, type_name: &str, fname: &str) -> Option<SemTy> {
        for s in &self.program.structs {
            if s.name == type_name {
                if let Some(f) = s.fields.iter().find(|f| f.name == fname) { return Some(f.ty.clone()); }
            }
        }
        for c in &self.program.classes {
            if c.name == type_name {
                if let Some(f) = c.fields.iter().find(|f| f.name == fname) { return Some(f.ty.clone()); }
            }
        }
        None
    }
}
