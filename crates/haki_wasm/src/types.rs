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
