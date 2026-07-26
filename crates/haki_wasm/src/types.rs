/// types.rs — Maps Haki SemTy to Wasm ValType.
///
/// Mapping:
///   int    → i64
///   f64    → f64
///   bool   → i32   (Wasm has no i1; use 0/1)
///   string → i32   (pointer into linear memory, v0.1 simplified)
///   Named/Generic/Optional → i32  (opaque ptr for now)
///   void   → (no value type — function returns nothing)

use wasm_encoder::ValType;
use haki_typeck::typed_ast::SemTy;
use crate::error::{WasmError, WasmResult};

pub fn sem_to_val(ty: &SemTy) -> WasmResult<ValType> {
    match ty {
        SemTy::Int              => Ok(ValType::I64),
        SemTy::Float            => Ok(ValType::F64),
        SemTy::Bool             => Ok(ValType::I32),
        SemTy::String           => Ok(ValType::I32), // ptr into linear mem
        SemTy::Named(_)
        | SemTy::Generic(_, _)
        | SemTy::Optional(_)
        | SemTy::Tuple(_)
        | SemTy::Fn(_, _)
        | SemTy::Closure(_, _)  => Ok(ValType::I32), // fat ptr
        SemTy::Void | SemTy::Never => Err(WasmError::UnsupportedType("void".into())),
        SemTy::Var(n)           => Err(WasmError::UnsupportedType(format!("?{n}"))),
    }
}

/// Returns `None` for void/never (function returns nothing).
pub fn sem_to_result(ty: &SemTy) -> Option<ValType> {
    match ty {
        SemTy::Void | SemTy::Never => None,
        other => sem_to_val(other).ok(),
    }
}

/// Map an AST TyKind to a Wasm ValType for extern fn declarations.
/// Same mapping as sem_to_val: int→i64, float→f64, bool→i32, everything else→i32 (ptr).
pub fn ast_ty_to_val(ty: &haki_ast::Ty) -> ValType {
    match &ty.kind {
        haki_ast::TyKind::Named(id) => match id.name.as_str() {
            "int"   => ValType::I64,
            "float" => ValType::F64,
            "bool"  => ValType::I32,
            _       => ValType::I32,  // string, Named types → ptr
        },
        haki_ast::TyKind::Generic(_, _)
        | haki_ast::TyKind::Optional(_)
        | haki_ast::TyKind::Tuple(_)
        | haki_ast::TyKind::Fn(_, _) => ValType::I32,
    }
}

/// Map an AST ReturnTy to an optional Wasm ValType.
/// Returns None for void (no return value).
pub fn ast_return_to_val(ret: &Option<haki_ast::ReturnTy>) -> Option<ValType> {
    match ret {
        None => None,
        Some(haki_ast::ReturnTy::Single(ty)) => {
            let v = ast_ty_to_val(ty);
            // void-equivalent Named("void") returns None
            if matches!(&ty.kind, haki_ast::TyKind::Named(id) if id.name == "void") {
                None
            } else {
                Some(v)
            }
        }
        Some(haki_ast::ReturnTy::Tuple(_)) => Some(ValType::I32), // tuple → ptr
    }
}
