/// haki_typeck — Type checker for the Haki programming language v0.1.

pub mod error;
pub mod typed_ast;
pub mod collector;
pub mod infer;
pub mod check;

pub use error::{TypeError, TypeResult};
pub use typed_ast::TypedSourceFile;
pub use collector::{SymbolTable, ModuleSymbols};

use haki_ast::SourceFile;

/// Type-check a source file using a pre-populated symbol table.
///
/// The caller is responsible for populating `sym` with any built-in
/// functions (stdlib) before calling this. Typically:
///
///   let mut sym = SymbolTable::new();
///   haki_stdlib::register_builtins(&mut sym);
///   let typed = typecheck_with_sym(file, sym)?;
pub fn typecheck_with_sym(file: &SourceFile, mut sym: SymbolTable) -> TypeResult<TypedSourceFile> {
    sym.collect(file)?;
    let mut inferer = infer::Inferer::new(sym);
    let typed = inferer.infer_file(file)?;
    check::check(&typed)?;
    Ok(typed)
}

/// Type-check a source file with only the built-in types pre-registered
/// (no stdlib functions). Useful for tests that don't need `print` etc.
pub fn typecheck(file: &SourceFile) -> TypeResult<TypedSourceFile> {
    typecheck_with_sym(file, SymbolTable::new())
}

/// Collect the exported symbols from a module source file.
/// Used by the compiler driver to build the module registry.
/// Does NOT run inference or checking — only the collection pass.
pub fn collect_module(file: &SourceFile) -> TypeResult<ModuleSymbols> {
    // Run collection into a fresh (no-builtins) symbol table.
    let mut sym = SymbolTable::default();
    sym.collect(file)?;

    // Extract only user-defined symbols (not builtins injected by new()).
    let mut mod_syms = ModuleSymbols::default();
    for (name, fi) in &sym.functions {
        mod_syms.functions.insert(name.clone(), fi.clone());
    }
    for (name, td) in &sym.types {
        mod_syms.types.insert(name.clone(), td.clone());
    }
    for (name, ed) in &sym.enum_defs {
        mod_syms.enum_defs.insert(name.clone(), ed.clone());
    }
    Ok(mod_syms)
}
