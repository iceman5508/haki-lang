/// haki_stdlib — Standard library for Haki v0.1.
///
/// Responsibilities:
///   1. Register built-in functions (`print`, `panic`) in the type-checker's
///      symbol table so Haki source can call them.
///   2. Provide the `haki_runtime.c` source that implements those functions at
///      the C level — emitted alongside the IR by `hakic`.
///   3. Declare extern LLVM function signatures in `haki_codegen` so the IR
///      contains proper `declare` stubs.
///
/// Built-in functions (v0.1):
///   print(msg: string)        — writes to stdout, newline appended
///   print_int(n: int)         — prints an integer
///   print_float(f: f64)       — prints a float
///   panic(msg: string)        — terminates the process (re-exported from runtime)

pub mod builtins;
pub mod runtime;

pub use builtins::register_builtins;
pub use runtime::RUNTIME_C_SOURCE;
pub use runtime::CORE_RUNTIME_C_SOURCE;
pub use runtime::UI_RUNTIME_C_SOURCE;
pub use runtime::UI_RUNTIME_WIN32_C_SOURCE;
pub use runtime::SYS_RUNTIME_C_SOURCE;
