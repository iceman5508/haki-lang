/// haki_wasm — WebAssembly backend for Haki v0.1.
///
/// Takes a `MonoProgram` and emits a valid `.wasm` binary module.
///
/// v0.1 scope:
///   - Core value types: i64 (int), f64, i32 (bool), externref (string/ptr)
///   - Function definitions with parameters and return values
///   - Basic arithmetic, comparisons, logical ops
///   - if/else (using Wasm `if`/`else`/`end`)
///   - while loops (using Wasm `block`/`loop`/`br_if`)
///   - Local variable binding (via Wasm locals)
///   - Function calls (direct)
///   - Imports: `env.print`, `env.print_int`, `env.print_float`
///     (host provides these; browser/Node wires them to console.log)
///
/// NOT in v0.1 Wasm scope:
///   - ARC / garbage collection (host memory model)
///   - Structs/classes (no GC proposal yet)
///   - Array/Map (future: use Wasm GC proposal or linear memory)
///   - Thread spawning (Wasm threads need SharedArrayBuffer)

pub mod error;
pub mod types;
pub mod emitter;

pub use error::{WasmError, WasmResult};
pub use emitter::WasmEmitter;

use haki_mono::mono_ast::MonoProgram;

/// Entry point: lower a monomorphized program to a Wasm binary.
pub fn emit_wasm(program: &MonoProgram, module_name: &str) -> WasmResult<Vec<u8>> {
    let mut emitter = WasmEmitter::new(module_name);
    emitter.emit(program)?;
    Ok(emitter.finish())
}
