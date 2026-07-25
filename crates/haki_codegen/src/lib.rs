/// haki_codegen — LLVM IR emission for the Haki programming language v0.1.
///
/// Takes a `MonoProgram` (fully monomorphized, no type variables) and
/// produces LLVM IR text that can be compiled to a native binary via clang.
///
/// Architecture:
///   - `types`   — maps `ConcrTy` to `inkwell::types::BasicTypeEnum`
///   - `layout`  — pre-registers all struct/class LLVM struct types
///   - `arc`     — declares ARC runtime functions (retain/release)
///   - `codegen` — the main code generator; lowers fns/stmts/exprs
///
/// ARC model:
///   - Every class (reference type) value has a `%haki_obj` header:
///       { i64 refcount, i8* destructor_ptr }
///   - `haki_retain(ptr)` increments the refcount atomically.
///   - `haki_release(ptr)` decrements; if it hits zero, calls destructor.
///   - The codegen injects retain at every assignment of a class value,
///     and release at the end of every scope that owns a class binding.
///
/// Output: LLVM IR as a String. The caller (hakic) writes it to a .ll file.

pub mod error;
pub mod types;
pub mod layout;
pub mod arc;
pub mod codegen;

pub use codegen::CodeGen;
pub use error::{CodeGenError, CodeGenResult};

use haki_mono::mono_ast::MonoProgram;

/// Entry point: lower a monomorphized program to LLVM IR text.
pub fn emit_ir(program: &MonoProgram, module_name: &str) -> CodeGenResult<String> {
    let ctx = inkwell::context::Context::create();
    let mut cg = CodeGen::new(&ctx, module_name, program);
    cg.emit()?;
    Ok(cg.ir_string())
}
