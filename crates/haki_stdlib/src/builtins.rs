/// builtins.rs — Injects built-in function signatures into the type-checker.
///
/// Every function here is declared `extern` in the IR and implemented in
/// `haki_runtime.c`. The typechecker only needs the signature; the codegen
/// emits `declare` stubs that the linker resolves.

use haki_ast::{Ident, Param, ReturnTy, Span, Ty, TyKind};
use haki_typeck::collector::{FnInfo, SymbolTable};

/// Register all v0.1 built-in functions into `sym`.
/// Called once at the start of every compilation, before user code is collected.
pub fn register_builtins(sym: &mut SymbolTable) {
    // ── print(msg: string) ────────────────────────────────────────────────
    // Writes `msg` to stdout followed by a newline.
    // Unified print(any) — accepts string, int, float, bool.
    // The codegen dispatches to the right C function based on the argument type.
    // The `T` parameter is a single-uppercase-letter wildcard that the typechecker
    // accepts for any concrete type (see infer_call).
    // Writes to stdout followed by a newline.
    // Unified print(any): the typechecker accepts string; the codegen
    // dispatches int/float/bool to the right C variant at emit time.
    sym.functions.insert("print".into(), FnInfo {
        name: "print".into(),
        type_params: vec![],
        params: vec![str_param("msg")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── print_int(n: int) ─────────────────────────────────────────────────
    sym.functions.insert("print_int".into(), FnInfo {
        name: "print_int".into(),
        type_params: vec![],
        params: vec![int_param("n")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── print_float(f: f64) ───────────────────────────────────────────────
    sym.functions.insert("print_float".into(), FnInfo {
        name: "print_float".into(),
        type_params: vec![],
        params: vec![float_param("f")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── print_bool(b: bool) ───────────────────────────────────────────────
    sym.functions.insert("print_bool".into(), FnInfo {
        name: "print_bool".into(),
        type_params: vec![],
        params: vec![bool_param("b")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── int_to_string(n: int) -> string ───────────────────────────────────
    sym.functions.insert("int_to_string".into(), FnInfo {
        name: "int_to_string".into(),
        type_params: vec![],
        params: vec![int_param("n")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── float_to_string(f: f64) -> string ────────────────────────────────
    sym.functions.insert("float_to_string".into(), FnInfo {
        name: "float_to_string".into(),
        type_params: vec![],
        params: vec![float_param("f")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── bool_to_string(b: bool) -> string ────────────────────────────────
    sym.functions.insert("bool_to_string".into(), FnInfo {
        name: "bool_to_string".into(),
        type_params: vec![],
        params: vec![bool_param("b")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── string_length(s: string) -> int ──────────────────────────────────
    sym.functions.insert("string_length".into(), FnInfo {
        name: "string_length".into(),
        type_params: vec![],
        params: vec![str_param("s")],
        return_ty: Some(ReturnTy::Single(int_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── argv() -> Array<string> ───────────────────────────────────────────
    // Returns the command-line arguments passed to the program.
    // argv()[0] is the program name; argv()[1] is the first user argument.
    sym.functions.insert("argv".into(), FnInfo {
        name: "argv".into(),
        type_params: vec![],
        params: vec![],
        return_ty: Some(ReturnTy::Single(array_ty("string"))),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── readFile(path: string) -> (string, Error?) ────────────────────────
    sym.functions.insert("readFile".into(), FnInfo {
        name: "readFile".into(),
        type_params: vec![],
        params: vec![str_param("path")],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(Ty {
                kind: TyKind::Named(Ident::new("Error", Span::dummy())),
                span: Span::dummy(),
            })), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });

    // ── writeFile(path: string, content: string) -> Error? ────────────────
    sym.functions.insert("writeFile".into(), FnInfo {
        name: "writeFile".into(),
        type_params: vec![],
        params: vec![str_param("path"), str_param("content")],
        return_ty: Some(ReturnTy::Single(Ty {
            kind: TyKind::Optional(Box::new(Ty {
                kind: TyKind::Named(Ident::new("Error", Span::dummy())),
                span: Span::dummy(),
            })),
            span: Span::dummy(),
        })),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });

    // ── fileExists(path: string) -> bool ──────────────────────────────────
    sym.functions.insert("fileExists".into(), FnInfo {
        name: "fileExists".into(),
        type_params: vec![],
        params: vec![str_param("path")],
        return_ty: Some(ReturnTy::Single(bool_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── std/env builtins ──────────────────────────────────────────────────
    sym.functions.insert("haki_env_get".into(), FnInfo {
        name: "haki_env_get".into(), type_params: vec![],
        params: vec![str_param("name")],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_env_set".into(), FnInfo {
        name: "haki_env_set".into(), type_params: vec![],
        params: vec![str_param("name"), str_param("value")],
        return_ty: Some(ReturnTy::Single(Ty {
            kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy()
        })),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_env_unset".into(), FnInfo {
        name: "haki_env_unset".into(), type_params: vec![],
        params: vec![str_param("name")],
        return_ty: Some(ReturnTy::Single(Ty {
            kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy()
        })),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_env_cwd".into(), FnInfo {
        name: "haki_env_cwd".into(), type_params: vec![], params: vec![],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_env_chdir".into(), FnInfo {
        name: "haki_env_chdir".into(), type_params: vec![],
        params: vec![str_param("path")],
        return_ty: Some(ReturnTy::Single(Ty {
            kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy()
        })),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });

    // ── std/time builtins ─────────────────────────────────────────────────
    sym.functions.insert("haki_time_now_ms".into(), FnInfo {
        name: "haki_time_now_ms".into(), type_params: vec![], params: vec![],
        return_ty: Some(ReturnTy::Single(int_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });
    sym.functions.insert("haki_time_sleep_ms".into(), FnInfo {
        name: "haki_time_sleep_ms".into(), type_params: vec![],
        params: vec![int_param("ms")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });
    sym.functions.insert("haki_time_format".into(), FnInfo {
        name: "haki_time_format".into(), type_params: vec![],
        params: vec![int_param("unix_sec")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── std/process builtins ──────────────────────────────────────────────
    sym.functions.insert("haki_process_run".into(), FnInfo {
        name: "haki_process_run".into(), type_params: vec![],
        params: vec![str_param("cmd"), param("args", array_ty("string"))],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_process_exec".into(), FnInfo {
        name: "haki_process_exec".into(), type_params: vec![],
        params: vec![str_param("cmd"), param("args", array_ty("string"))],
        return_ty: Some(ReturnTy::Tuple(vec![
            int_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_process_shell".into(), FnInfo {
        name: "haki_process_shell".into(), type_params: vec![],
        params: vec![str_param("cmd")],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_process_exit".into(), FnInfo {
        name: "haki_process_exit".into(), type_params: vec![],
        params: vec![int_param("code")],
        return_ty: None,
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── std/regex builtins ────────────────────────────────────────────────
    sym.functions.insert("haki_regex_matches".into(), FnInfo {
        name: "haki_regex_matches".into(), type_params: vec![],
        params: vec![str_param("s"), str_param("pattern")],
        return_ty: Some(ReturnTy::Single(bool_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });
    sym.functions.insert("haki_regex_find".into(), FnInfo {
        name: "haki_regex_find".into(), type_params: vec![],
        params: vec![str_param("s"), str_param("pattern")],
        return_ty: Some(ReturnTy::Tuple(vec![
            str_ty(),
            Ty { kind: TyKind::Optional(Box::new(named_ty("Error"))), span: Span::dummy() },
        ])),
        span: Span::dummy(),
        is_extern: false,
        extern_abi: None,
    });
    sym.functions.insert("haki_regex_replace_all".into(), FnInfo {
        name: "haki_regex_replace_all".into(), type_params: vec![],
        params: vec![str_param("s"), str_param("pattern"), str_param("replacement")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });
    sym.functions.insert("haki_regex_split".into(), FnInfo {
        name: "haki_regex_split".into(), type_params: vec![],
        params: vec![str_param("s"), str_param("pattern")],
        return_ty: Some(ReturnTy::Single(array_ty("string"))),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // ── std/json additional builtins ───────────────────────────────────────
    sym.functions.insert("jsonEncodeObject".into(), FnInfo {
        name: "jsonEncodeObject".into(), type_params: vec![],
        params: vec![param("fields", map_str_str_ty())],
        // Map<string,string> param — matches json.haki fn object signature
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });
    sym.functions.insert("jsonEncodeArray".into(), FnInfo {
        name: "jsonEncodeArray".into(), type_params: vec![],
        params: vec![param("items", array_ty("string"))],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(), is_extern: false, extern_abi: None });

    // haki_ui platform functions — provided by haki_ui_gtk.c / haki_ui_dom.js at link time
    sym.functions.insert("haki_app_run".into(), FnInfo {
        name: "haki_app_run".into(), type_params: vec![],
        params: vec![str_param("json"), str_param("title"),
                     int_param("width"), int_param("height")],
        return_ty: None,
        span: Span::dummy(), is_extern: true,
        extern_abi: Some("c".into()) });

    // ── Chan<T> runtime ──────────────────────────────────────────────────────
    // Chan<T> is a builtin generic class handled specially in the typechecker.
    // Method signatures are registered here for type inference.
    sym.functions.insert("haki_chan_new".into(), FnInfo {
        name: "haki_chan_new".into(), type_params: vec![],
        params: vec![int_param("capacity"), int_param("elem_size")],
        return_ty: Some(ReturnTy::Single(named_ty("Chan"))),
        span: Span::dummy(), is_extern: true, extern_abi: Some("c".into()) });

    sym.functions.insert("haki_chan_close".into(), FnInfo {
        name: "haki_chan_close".into(), type_params: vec![],
        params: vec![param("ch", named_ty("Chan"))],
        return_ty: None,
        span: Span::dummy(), is_extern: true, extern_abi: Some("c".into()) });

    // ── TaskGroup<T> runtime ─────────────────────────────────────────────────
    sym.functions.insert("haki_taskgroup_new".into(), FnInfo {
        name: "haki_taskgroup_new".into(), type_params: vec![],
        params: vec![],
        return_ty: Some(ReturnTy::Single(named_ty("TaskGroup"))),
        span: Span::dummy(), is_extern: true, extern_abi: Some("c".into()) });

    // haki_ui GTK platform functions — provided by haki_ui_gtk.c at link time
    let gtk_fns: &[(&str, bool)] = &[
        ("haki_gtk_create_window",          true),   // → int
        ("haki_gtk_create_label",           true),   // → int
        ("haki_gtk_create_button",          true),   // → int
        ("haki_gtk_create_box",             true),   // → int
        ("haki_gtk_set_text",               false),  // → void
        ("haki_gtk_set_visible",            false),  // → void
        ("haki_gtk_insert_child",           false),  // → void
        ("haki_gtk_remove_child",           false),  // → void
        ("haki_platform_run",               false),  // → void
        ("haki_set_callback_dispatcher",    false),  // → void
        ("haki_gtk_alloc_node_id",           true),   // → int
        ("haki_register_callback",           false),  // → void
        ("haki_fire_callback",               false),  // → void
        ("haki_make_closure",                true),   // → void* fat pointer
        ("haki_set_rerender_callback",        false),  // → void
        ("haki_gtk_peek_next_id",            true),   // → int
        ("haki_gtk_mark_label",              false),  // → void
        ("haki_gtk_get_label_id",            true),   // → int
        // v3.4 new functions
        ("haki_gtk_register_node",           false),  // → void
        ("haki_set_rerender_fn",             false),  // → void
        ("haki_trigger_rerender",            false),  // → void
        ("haki_gtk_set_callback",            false),  // → void
        ("haki_gtk_create_text_field",       true),   // → int
        ("haki_gtk_create_checkbox",         true),   // → int
        ("haki_gtk_create_dropdown",         true),   // → int
        ("haki_gtk_create_image",            true),   // → int
        ("haki_gtk_set_padding",             false),  // → void
        ("haki_gtk_set_spacing",             false),  // → void
        ("haki_gtk_set_alignment",           false),  // → void
        ("haki_get_callback",                true),   // → void* closure
    ];
    for (name, returns_int) in gtk_fns {
        sym.functions.insert((*name).into(), FnInfo {
            name: (*name).into(), type_params: vec![],
            params: vec![],
            return_ty: if *returns_int {
                Some(ReturnTy::Single(int_ty()))
            } else {
                None
            },
            span: Span::dummy(), is_extern: true,
            extern_abi: Some("c".into()) });
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn named_ty(name: &str) -> Ty {
    Ty { kind: TyKind::Named(Ident::new(name, Span::dummy())), span: Span::dummy() }
}

fn array_ty(elem: &str) -> Ty {
    Ty { kind: TyKind::Generic(
        Ident::new("Array", Span::dummy()),
        vec![named_ty(elem)]
    ), span: Span::dummy() }
}

fn str_ty()   -> Ty { named_ty("string") }
fn int_ty()   -> Ty { named_ty("int") }
fn float_ty() -> Ty { named_ty("f64") }
fn bool_ty()  -> Ty { named_ty("bool") }
fn map_str_str_ty() -> Ty {
    Ty { kind: TyKind::Generic(
        Ident::new("Map", Span::dummy()),
        vec![named_ty("string"), named_ty("string")]
    ), span: Span::dummy() }
}

fn param(name: &str, ty: Ty) -> Param {
    Param { name: Ident::new(name, Span::dummy()), ty, span: Span::dummy() }
}
fn str_param(name: &str)   -> Param { param(name, str_ty()) }
fn int_param(name: &str)   -> Param { param(name, int_ty()) }
fn float_param(name: &str) -> Param { param(name, float_ty()) }
fn bool_param(name: &str)  -> Param { param(name, bool_ty()) }
