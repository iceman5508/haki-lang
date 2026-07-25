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
        span: Span::dummy(),
    });

    // ── print_int(n: int) ─────────────────────────────────────────────────
    sym.functions.insert("print_int".into(), FnInfo {
        name: "print_int".into(),
        type_params: vec![],
        params: vec![int_param("n")],
        return_ty: None,
        span: Span::dummy(),
    });

    // ── print_float(f: f64) ───────────────────────────────────────────────
    sym.functions.insert("print_float".into(), FnInfo {
        name: "print_float".into(),
        type_params: vec![],
        params: vec![float_param("f")],
        return_ty: None,
        span: Span::dummy(),
    });

    // ── print_bool(b: bool) ───────────────────────────────────────────────
    sym.functions.insert("print_bool".into(), FnInfo {
        name: "print_bool".into(),
        type_params: vec![],
        params: vec![bool_param("b")],
        return_ty: None,
        span: Span::dummy(),
    });

    // ── int_to_string(n: int) -> string ───────────────────────────────────
    sym.functions.insert("int_to_string".into(), FnInfo {
        name: "int_to_string".into(),
        type_params: vec![],
        params: vec![int_param("n")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(),
    });

    // ── float_to_string(f: f64) -> string ────────────────────────────────
    sym.functions.insert("float_to_string".into(), FnInfo {
        name: "float_to_string".into(),
        type_params: vec![],
        params: vec![float_param("f")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(),
    });

    // ── bool_to_string(b: bool) -> string ────────────────────────────────
    sym.functions.insert("bool_to_string".into(), FnInfo {
        name: "bool_to_string".into(),
        type_params: vec![],
        params: vec![bool_param("b")],
        return_ty: Some(ReturnTy::Single(str_ty())),
        span: Span::dummy(),
    });

    // ── string_length(s: string) -> int ──────────────────────────────────
    sym.functions.insert("string_length".into(), FnInfo {
        name: "string_length".into(),
        type_params: vec![],
        params: vec![str_param("s")],
        return_ty: Some(ReturnTy::Single(int_ty())),
        span: Span::dummy(),
    });

    // ── argv() -> Array<string> ───────────────────────────────────────────
    // Returns the command-line arguments passed to the program.
    // argv()[0] is the program name; argv()[1] is the first user argument.
    sym.functions.insert("argv".into(), FnInfo {
        name: "argv".into(),
        type_params: vec![],
        params: vec![],
        return_ty: Some(ReturnTy::Single(array_ty("string"))),
        span: Span::dummy(),
    });

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
    });

    // ── fileExists(path: string) -> bool ──────────────────────────────────
    sym.functions.insert("fileExists".into(), FnInfo {
        name: "fileExists".into(),
        type_params: vec![],
        params: vec![str_param("path")],
        return_ty: Some(ReturnTy::Single(bool_ty())),
        span: Span::dummy(),
    });
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

fn param(name: &str, ty: Ty) -> Param {
    Param { name: Ident::new(name, Span::dummy()), ty, span: Span::dummy() }
}
fn str_param(name: &str)   -> Param { param(name, str_ty()) }
fn int_param(name: &str)   -> Param { param(name, int_ty()) }
fn float_param(name: &str) -> Param { param(name, float_ty()) }
fn bool_param(name: &str)  -> Param { param(name, bool_ty()) }
